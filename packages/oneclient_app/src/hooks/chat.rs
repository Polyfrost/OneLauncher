use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use freya::radio::RadioStation;
use uuid::Uuid;

use oneclient_polyplus::{
    BlockedPlayer, Friend, FriendRequest, MAX_MESSAGE_LENGTH, MAX_PAGE_SIZE, PlayerStatus,
    PlusClient, PlusError,
};

use crate::chat::{ChatRoster, ChatToast, FriendRequestStatus, FriendRequests};
use crate::components::IconType;
use crate::hooks::{Actions, PumpSignal};
use crate::i18n::{t, tf};
use crate::state::{AppChannel, AppState, AsyncStatus};

const PAGE_SIZE: u64 = 50;

const RESYNC_FLOOR: Duration = Duration::from_secs(30);

const ROSTER_FLOOR: Duration = Duration::from_secs(60);

static RESYNC_RUNNING: AtomicBool = AtomicBool::new(false);
static RESYNC_QUEUED: AtomicBool = AtomicBool::new(false);
static LAST_RESYNC: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_ROSTER: Mutex<Option<Instant>> = Mutex::new(None);

type Station = RadioStation<AppState, AppChannel>;

fn client() -> Option<std::sync::Arc<PlusClient>> {
    oneclient_polyplus::client()
}

fn owner_of(station: Station) -> Option<Uuid> {
    station.peek().chat.owner
}

fn report(station: Station, error: &PlusError) {
    tracing::error!("chat request failed: {error:#}");
    fail(station, error.to_string());
}

fn fail(station: Station, message: String) {
    let mut guard = station.clone().write_channel(AppChannel::Chat);
    guard.chat.status = AsyncStatus::Error;
    guard.chat.error = Some(message);
}

fn clear_error(station: Station) {
    if station.peek().chat.error.is_none() {
        return;
    }

    let mut guard = station.clone().write_channel(AppChannel::Chat);
    guard.chat.status = AsyncStatus::Ready;
    guard.chat.error = None;
}

fn settle_friend_request(station: Station, status: FriendRequestStatus) {
    station
        .clone()
        .write_channel(AppChannel::Chat)
        .chat
        .friend_request = status;
}

fn select_group(station: Station, group_id: i32) {
    let mut guard = station.clone().write_channel(AppChannel::Chat);
    guard.chat.active = Some(group_id);
    guard.chat.mark_read(group_id);
    guard.chat.begin_thread_load(group_id);
}

async fn load_group(station: Station, group_id: i32) {
    load_messages(station, group_id).await;
    mark_read(station, group_id).await;
}

async fn open_group(station: Station, group_id: i32) {
    select_group(station, group_id);
    load_group(station, group_id).await;
}

async fn roster_change(station: Station, result: Result<(), PlusError>) {
    match result {
        Ok(()) => {
            clear_error(station);
            load_roster(station).await;
        }
        Err(err) => report(station, &err),
    }
}

fn collect_roster(
    friends: Result<Vec<Friend>, PlusError>,
    incoming: Result<Vec<FriendRequest>, PlusError>,
    outgoing: Result<Vec<FriendRequest>, PlusError>,
    blocked: Result<Vec<BlockedPlayer>, PlusError>,
) -> Result<ChatRoster, PlusError> {
    Ok(ChatRoster {
        friends: friends?,
        requests: FriendRequests {
            incoming: incoming?,
            outgoing: outgoing?,
        },
        blocked: blocked?,
    })
}

pub async fn load_roster(station: Station) {
    let Some(client) = client() else { return };
    let owner = owner_of(station);

    let (friends, incoming, outgoing, blocked) = tokio::join!(
        client.friends(),
        client.incoming_requests(),
        client.outgoing_requests(),
        client.blocked(),
    );

    if owner_of(station) != owner {
        return;
    }

    match collect_roster(friends, incoming, outgoing, blocked) {
        Ok(roster) => {
            if let Ok(mut last) = LAST_ROSTER.lock() {
                *last = Some(Instant::now());
            }

            oneclient_polyplus::watch_players(
                roster.friends.iter().map(|friend| friend.player).collect(),
            );

            station
                .clone()
                .write_channel(AppChannel::ChatRoster)
                .chat
                .set_roster(roster);
        }
        Err(err) => report(station, &err),
    }
}

fn roster_is_fresh() -> bool {
    LAST_ROSTER
        .lock()
        .ok()
        .and_then(|last| *last)
        .is_some_and(|at| at.elapsed() < ROSTER_FLOOR)
}

pub async fn load_conversations(station: Station) {
    let Some(client) = client() else { return };
    let owner = owner_of(station);

    {
        let mut guard = station.clone().write_channel(AppChannel::Chat);
        if guard.chat.conversations().is_empty() {
            guard.chat.status = AsyncStatus::Loading;
        }
    }

    let groups = client.groups().await;
    if owner_of(station) != owner {
        return;
    }

    match groups {
        Ok(groups) => station
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .set_conversations(groups),
        Err(err) => {
            report(station, &err);
            return;
        }
    }

    let unread: Vec<i32> = {
        let state = station.peek();
        state
            .chat
            .conversations()
            .iter()
            .filter(|conversation| conversation.unread && !state.chat.has_messages(conversation.id))
            .map(|conversation| conversation.id)
            .collect()
    };

    for group_id in unread {
        load_messages(station, group_id).await;
    }
}

pub async fn load_messages(station: Station, group_id: i32) {
    let Some(client) = client() else {
        station
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .fail_thread_load(group_id, t("oneclient.chat.errors.unavailable").to_string());
        return;
    };

    station
        .clone()
        .write_channel(AppChannel::Chat)
        .chat
        .begin_thread_load(group_id);

    let owner = owner_of(station);
    let limit = PAGE_SIZE.min(MAX_PAGE_SIZE);
    let page = client.messages(group_id, None, Some(limit)).await;
    if owner_of(station) != owner {
        return;
    }

    match page {
        Ok(page) => {
            let complete = (page.len() as u64) < limit;
            let mut guard = station.clone().write_channel(AppChannel::Chat);
            guard.chat.set_messages(group_id, page);
            guard.chat.set_history_complete(group_id, complete);
            guard.chat.settle_thread_load(group_id);
        }
        Err(err) => {
            tracing::error!("could not load conversation {group_id}: {err:#}");
            station
                .clone()
                .write_channel(AppChannel::Chat)
                .chat
                .fail_thread_load(group_id, err.to_string());
        }
    }
}

pub async fn load_older_messages(station: Station, group_id: i32) {
    let Some(client) = client() else { return };

    let before = station.peek().chat.oldest_message(group_id);
    if before.is_none() {
        return;
    }

    let owner = owner_of(station);
    let limit = PAGE_SIZE.min(MAX_PAGE_SIZE);
    let page = client.messages(group_id, before, Some(limit)).await;
    if owner_of(station) != owner {
        return;
    }

    match page {
        Ok(page) => {
            let complete = (page.len() as u64) < limit;
            let mut guard = station.clone().write_channel(AppChannel::Chat);
            if !page.is_empty() {
                guard.chat.prepend_messages(group_id, page);
            }
            guard.chat.set_history_complete(group_id, complete);
        }
        Err(err) => report(station, &err),
    }
}

pub async fn mark_read(station: Station, group_id: i32) {
    let Some(client) = client() else { return };

    let Some(message_id) = station.peek().chat.newest_message(group_id) else {
        return;
    };

    if let Err(err) = client.mark_read(group_id, message_id).await {
        tracing::warn!("could not mark conversation {group_id} read: {err:#}");
        return;
    }

    station
        .clone()
        .write_channel(AppChannel::Chat)
        .chat
        .mark_read(group_id);
}

struct ResyncGuard;

impl Drop for ResyncGuard {
    fn drop(&mut self) {
        RESYNC_RUNNING.store(false, Ordering::Release);
    }
}

fn resynced_within(floor: Duration) -> bool {
    LAST_RESYNC
        .lock()
        .ok()
        .and_then(|last| *last)
        .is_some_and(|at| at.elapsed() < floor)
}

fn stamp_resync() {
    if let Ok(mut last) = LAST_RESYNC.lock() {
        *last = Some(Instant::now());
    }
}

async fn resync_once(station: Station) {
    load_conversations(station).await;

    let active = station.peek().chat.active;
    if let Some(group_id) = active {
        load_messages(station, group_id).await;
    }
}

async fn resync(station: Station, floor: Option<Duration>) {
    if floor.is_some_and(resynced_within) {
        return;
    }

    if RESYNC_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        RESYNC_QUEUED.store(true, Ordering::Release);
        return;
    }

    let _guard = ResyncGuard;

    loop {
        RESYNC_QUEUED.store(false, Ordering::Release);
        resync_once(station).await;
        stamp_resync();

        if !RESYNC_QUEUED.load(Ordering::Acquire) {
            break;
        }
    }
}

pub async fn resync_chat(station: Station) {
    resync(station, None).await;
}

async fn deliver(station: Station, group_id: i32, key: Uuid, content: String) {
    let Some(client) = client() else {
        station
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .fail_send(group_id, key);
        return;
    };

    let owner = owner_of(station);
    let sent = client.send_message(group_id, &content, key).await;
    if owner_of(station) != owner {
        return;
    }

    match sent {
        Ok(message) => {
            let sender = message.sender;
            let mut guard = station.clone().write_channel(AppChannel::Chat);
            guard.chat.finish_send(group_id, key, message.into());
            guard.chat.note_activity(group_id, sender, content, false);
        }
        Err(err) => {
            tracing::error!("could not send a chat message: {err:#}");
            station
                .clone()
                .write_channel(AppChannel::Chat)
                .chat
                .fail_send(group_id, key);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChatJob {
    Resync {
        floored: bool,
    },
    Roster {
        floored: bool,
    },
    Conversations,
    OpenGroup(i32),
    OlderMessages(i32),
    Deliver {
        group_id: i32,
        key: Uuid,
        content: String,
    },
    EditMessage {
        group_id: i32,
        message_id: i64,
        content: String,
    },
    DeleteMessage {
        group_id: i32,
        message_id: i64,
    },
    StartDirectMessage(Uuid),
    CreateGroup {
        name: String,
        members: Vec<Uuid>,
    },
    AddFriend(String),
    AcceptRequest(i32),
    DeclineRequest(i32),
    CancelRequest(i32),
    RemoveFriend(Uuid),
    Block(Uuid),
    Unblock(Uuid),
    SetStatus {
        status: PlayerStatus,
        previous: PlayerStatus,
    },
    AddMembers {
        group_id: i32,
        members: Vec<Uuid>,
    },
    LeaveGroup(i32),
    SwitchOwner {
        signed_in: bool,
    },
}

impl ChatJob {
    pub async fn run(self, station: Station) {
        match self {
            Self::Resync { floored } => {
                resync(station, floored.then_some(RESYNC_FLOOR)).await;
            }
            Self::Roster { floored } => {
                if floored && roster_is_fresh() {
                    return;
                }
                load_roster(station).await;
            }
            Self::Conversations => load_conversations(station).await,
            Self::OpenGroup(group_id) => load_group(station, group_id).await,
            Self::OlderMessages(group_id) => load_older_messages(station, group_id).await,
            Self::Deliver {
                group_id,
                key,
                content,
            } => deliver(station, group_id, key, content).await,
            Self::EditMessage {
                group_id,
                message_id,
                content,
            } => {
                let Some(client) = client() else { return };
                let owner = owner_of(station);
                let edited = client.edit_message(group_id, message_id, &content).await;
                if owner_of(station) != owner {
                    return;
                }

                match edited {
                    Ok(message) => station
                        .clone()
                        .write_channel(AppChannel::Chat)
                        .chat
                        .upsert(group_id, message.into()),
                    Err(err) => report(station, &err),
                }
            }
            Self::DeleteMessage {
                group_id,
                message_id,
            } => {
                let Some(client) = client() else { return };
                let owner = owner_of(station);
                let deleted = client.delete_message(group_id, message_id).await;
                if owner_of(station) != owner {
                    return;
                }

                match deleted {
                    Ok(()) => station
                        .clone()
                        .write_channel(AppChannel::Chat)
                        .chat
                        .remove(group_id, message_id),
                    Err(err) => report(station, &err),
                }
            }
            Self::StartDirectMessage(player) => {
                let Some(client) = client() else { return };
                let owner = owner_of(station);
                let opened = client.open_direct_message(player).await;
                if owner_of(station) != owner {
                    return;
                }

                match opened {
                    Ok(summary) => {
                        load_conversations(station).await;
                        open_group(station, summary.id).await;
                    }
                    Err(err) => report(station, &err),
                }
            }
            Self::CreateGroup { name, members } => {
                let Some(client) = client() else { return };
                let owner = owner_of(station);
                let created = client.create_group(&name, &members).await;
                if owner_of(station) != owner {
                    return;
                }

                match created {
                    Ok(summary) => {
                        load_conversations(station).await;
                        open_group(station, summary.id).await;
                    }
                    Err(err) => report(station, &err),
                }
            }
            Self::AddFriend(username) => {
                let Some(client) = client() else {
                    settle_friend_request(
                        station,
                        FriendRequestStatus::Failed(t("oneclient.chat.errors.unavailable").into()),
                    );
                    return;
                };

                let outcome = match client.lookup_username(&username).await {
                    Ok(Some(player)) => client.send_friend_request(player).await.map(drop),
                    Ok(None) => {
                        settle_friend_request(
                            station,
                            FriendRequestStatus::Failed(tf(
                                "oneclient.chat.errors.no_player",
                                &[&username],
                            )),
                        );
                        return;
                    }
                    Err(err) => Err(err),
                };

                match outcome {
                    Ok(()) => {
                        settle_friend_request(station, FriendRequestStatus::Sent(username));
                        load_roster(station).await;
                    }
                    Err(err) => {
                        tracing::error!("could not send a friend request: {err:#}");
                        settle_friend_request(
                            station,
                            FriendRequestStatus::Failed(err.to_string()),
                        );
                    }
                }
            }
            Self::AcceptRequest(request_id) => {
                let Some(client) = client() else { return };
                roster_change(station, client.accept_request(request_id).await).await;
                load_conversations(station).await;
            }
            Self::DeclineRequest(request_id) => {
                let Some(client) = client() else { return };
                roster_change(station, client.decline_request(request_id).await).await;
            }
            Self::CancelRequest(request_id) => {
                let Some(client) = client() else { return };
                roster_change(station, client.cancel_request(request_id).await).await;
            }
            Self::RemoveFriend(player) => {
                let Some(client) = client() else { return };
                roster_change(station, client.remove_friend(player).await).await;
            }
            Self::Block(player) => {
                let Some(client) = client() else { return };
                roster_change(station, client.block(player).await).await;
                load_conversations(station).await;
            }
            Self::Unblock(player) => {
                let Some(client) = client() else { return };
                roster_change(station, client.unblock(player).await).await;
            }
            Self::SwitchOwner { signed_in } => {
                oneclient_polyplus::watch_players(Vec::new());
                oneclient_polyplus::restart_session().await;

                if signed_in {
                    load_self_status(station).await;
                    load_roster(station).await;
                    resync_chat(station).await;
                }
            }
            Self::AddMembers { group_id, members } => {
                let Some(client) = client() else { return };
                let owner = owner_of(station);

                let mut failure = None;
                for member in members {
                    if let Err(err) = client.add_member(group_id, member).await {
                        failure = Some(err);
                        break;
                    }
                }

                if owner_of(station) != owner {
                    return;
                }
                if let Some(err) = failure {
                    report(station, &err);
                }
                load_conversations(station).await;
            }
            Self::LeaveGroup(group_id) => {
                let Some(client) = client() else { return };
                let Some(owner) = owner_of(station) else {
                    return;
                };

                let left = client.remove_member(group_id, owner).await;
                if owner_of(station) != Some(owner) {
                    return;
                }

                match left {
                    Ok(()) => station
                        .clone()
                        .write_channel(AppChannel::Chat)
                        .chat
                        .forget_conversation(group_id),
                    Err(err) => report(station, &err),
                }
                load_conversations(station).await;
            }
            Self::SetStatus { status, previous } => {
                let Some(client) = client() else { return };
                let owner = owner_of(station);
                let result = client.set_status(status).await;
                if owner_of(station) != owner {
                    return;
                }

                if let Err(err) = result {
                    {
                        let mut guard = station.clone().write_channel(AppChannel::Chat);
                        if guard.chat.self_status == status {
                            guard.chat.self_status = previous;
                        }
                    }
                    report(station, &err);
                }
            }
        }
    }
}

const TOAST_PREVIEW_CHARS: usize = 120;

pub async fn notify_chat(actions: Actions, toast: ChatToast) {
    let name = player_name(toast.sender()).await;
    let group_id = toast.group_id();

    let (title, body, toast_only) = match toast {
        ChatToast::Message {
            group_name,
            content,
            ..
        } => (
            match group_name {
                Some(group) => tf("oneclient.chat.notify.group_message", &[&name, &group]),
                None => name,
            },
            preview(&content),
            true,
        ),
        ChatToast::Invite { .. } => (
            tf("oneclient.chat.notify.invite", &[&name]),
            String::new(),
            false,
        ),
        ChatToast::FriendRequest { .. } => (
            tf("oneclient.chat.notify.friend_request", &[&name]),
            String::new(),
            false,
        ),
    };

    let notice = actions
        .notify(title)
        .body(body)
        .icon(IconType::MessageTextSquare01)
        .action(crate::notifications::NotificationAction {
            label: t("oneclient.chat.notify.open").to_string(),
            kind: crate::notifications::NotificationActionKind::OpenChat(group_id),
        });

    if toast_only {
        notice.toast_only().send();
    } else {
        notice.send();
    }
}

fn preview(content: &str) -> String {
    let mut chars = content.chars();
    let mut cut: String = chars.by_ref().take(TOAST_PREVIEW_CHARS).collect();
    if chars.next().is_some() {
        cut.push('…');
    }
    cut
}

async fn player_name(player: Uuid) -> String {
    let fallback = player.simple().to_string().chars().take(8).collect();
    let Ok(state) = crate::launcher::state() else {
        return fallback;
    };

    oneclient_mc::fetch_player_profile_view(&state.services.requester, &player.to_string(), None)
        .await
        .map(|view| view.username)
        .unwrap_or(fallback)
}

pub async fn load_self_status(station: Station) {
    let Some(client) = client() else { return };
    let owner = owner_of(station);

    let status = client.status().await;
    if owner_of(station) != owner {
        return;
    }

    match status {
        Ok(status) => {
            station
                .clone()
                .write_channel(AppChannel::Chat)
                .chat
                .self_status = status;
        }
        Err(err) => tracing::warn!("could not load the chat status: {err:#}"),
    }
}

impl Actions {
    fn chat_job(&self, job: ChatJob) {
        self.nudge(PumpSignal::Chat(job));
    }

    pub fn refresh_chat(&self) {
        self.chat_job(ChatJob::Resync { floored: false });
        self.chat_job(ChatJob::Roster { floored: true });
    }

    pub fn sync_chat(&self) {
        self.chat_job(ChatJob::Resync { floored: true });
    }

    pub fn reload_roster(&self) {
        self.chat_job(ChatJob::Roster { floored: false });
    }

    pub fn open_conversation(&self, group_id: i32) {
        select_group(self.station(), group_id);
        self.chat_job(ChatJob::OpenGroup(group_id));
    }

    pub fn reload_conversation(&self, group_id: i32) {
        self.station()
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .begin_thread_load(group_id);

        self.chat_job(ChatJob::OpenGroup(group_id));
    }

    pub fn load_older_messages(&self, group_id: i32) {
        self.chat_job(ChatJob::OlderMessages(group_id));
    }

    pub fn send_chat_message(&self, group_id: i32, content: impl Into<String>) {
        let content = content.into().trim().to_string();
        if content.is_empty() || content.len() > MAX_MESSAGE_LENGTH {
            return;
        }

        let key = Uuid::new_v4();
        self.station()
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .begin_send(group_id, key, content.clone());

        self.chat_job(ChatJob::Deliver {
            group_id,
            key,
            content,
        });
    }

    pub fn retry_chat_message(&self, group_id: i32, key: Uuid) {
        let content = self
            .station()
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .begin_retry(group_id, key);

        let Some(content) = content else { return };

        self.chat_job(ChatJob::Deliver {
            group_id,
            key,
            content,
        });
    }

    pub fn discard_chat_message(&self, group_id: i32, key: Uuid) {
        self.station()
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .discard_pending(group_id, key);
    }

    pub fn edit_chat_message(&self, group_id: i32, message_id: i64, content: impl Into<String>) {
        let content = content.into().trim().to_string();
        if content.is_empty() || content.len() > MAX_MESSAGE_LENGTH {
            return;
        }

        self.chat_job(ChatJob::EditMessage {
            group_id,
            message_id,
            content,
        });
    }

    pub fn delete_chat_message(&self, group_id: i32, message_id: i64) {
        self.chat_job(ChatJob::DeleteMessage {
            group_id,
            message_id,
        });
    }

    pub fn start_direct_message(&self, player: Uuid) {
        clear_error(self.station());

        let existing = {
            let station = self.station();
            let state = station.peek();
            let owner = state.chat.owner.unwrap_or_else(Uuid::nil);
            state
                .chat
                .conversations()
                .iter()
                .find(|conversation| conversation.counterpart(owner) == Some(player))
                .map(|conversation| conversation.id)
        };

        if let Some(group_id) = existing {
            self.open_conversation(group_id);
            return;
        }

        self.station()
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .active = None;
        self.chat_job(ChatJob::StartDirectMessage(player));
    }

    pub fn create_chat_group(&self, name: impl Into<String>, members: Vec<Uuid>) {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return;
        }

        self.chat_job(ChatJob::CreateGroup { name, members });
    }

    pub fn add_friend(&self, username: impl Into<String>) {
        let username = username.into().trim().to_string();
        if username.is_empty() {
            return;
        }

        settle_friend_request(
            self.station(),
            FriendRequestStatus::Sending(username.clone()),
        );
        self.chat_job(ChatJob::AddFriend(username));
    }

    pub fn reset_friend_request(&self) {
        settle_friend_request(self.station(), FriendRequestStatus::Idle);
    }

    pub fn clear_chat_error(&self) {
        clear_error(self.station());
    }

    pub fn retry_chat(&self) {
        self.clear_chat_error();
        self.chat_job(ChatJob::Resync { floored: false });
        self.chat_job(ChatJob::Roster { floored: false });
    }

    pub fn accept_friend_request(&self, request_id: i32) {
        self.chat_job(ChatJob::AcceptRequest(request_id));
    }

    pub fn decline_friend_request(&self, request_id: i32) {
        self.chat_job(ChatJob::DeclineRequest(request_id));
    }

    pub fn cancel_friend_request(&self, request_id: i32) {
        self.chat_job(ChatJob::CancelRequest(request_id));
    }

    pub fn remove_friend(&self, player: Uuid) {
        self.chat_job(ChatJob::RemoveFriend(player));
    }

    pub fn block_player(&self, player: Uuid) {
        self.chat_job(ChatJob::Block(player));
    }

    pub fn unblock_player(&self, player: Uuid) {
        self.chat_job(ChatJob::Unblock(player));
    }

    pub fn add_group_members(&self, group_id: i32, members: Vec<Uuid>) {
        if members.is_empty() {
            return;
        }

        clear_error(self.station());
        self.chat_job(ChatJob::AddMembers { group_id, members });
    }

    pub fn leave_group(&self, group_id: i32) {
        clear_error(self.station());
        self.chat_job(ChatJob::LeaveGroup(group_id));
    }

    pub fn show_chat(&self, group_id: Option<i32>) {
        if let Some(group_id) = group_id {
            self.open_conversation(group_id);
        }

        if !crate::view::chat::is_chat_window_open() {
            let mut guard = self.station().clone().write_channel(AppChannel::Social);
            guard.social_open = true;
            guard.social_popped_notice = false;
        }
    }

    pub fn set_self_status(&self, status: PlayerStatus) {
        let previous = {
            let station = self.station();
            let mut guard = station.clone().write_channel(AppChannel::Chat);
            let previous = guard.chat.self_status;
            guard.chat.self_status = status;
            previous
        };

        if previous != status {
            self.chat_job(ChatJob::SetStatus { status, previous });
        }
    }

    pub fn decline_chat_invite(&self, invite_id: i32) {
        self.station()
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .dismiss_invite(invite_id);
    }

    pub fn sync_chat_owner(&self, owner: Option<Uuid>) {
        let station = self.station();
        if station.peek().chat.owner == owner {
            return;
        }

        let changed = station
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .set_owner(owner);

        if !changed {
            return;
        }

        if owner.is_none() {
            crate::view::chat::close_chat_window();
            self.close_social();
        }

        self.chat_job(ChatJob::SwitchOwner {
            signed_in: owner.is_some(),
        });
    }
}
