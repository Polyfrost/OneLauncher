use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use freya::prelude::spawn_forever;
use freya::radio::RadioStation;
use uuid::Uuid;

use oneclient_polyplus::{MAX_MESSAGE_LENGTH, MAX_PAGE_SIZE, PlusClient, PlusError};

use crate::hooks::Actions;
use crate::state::{AppChannel, AppState, AsyncStatus};

const PAGE_SIZE: u64 = 50;

const RESYNC_FLOOR: Duration = Duration::from_secs(30);

static RESYNC_RUNNING: AtomicBool = AtomicBool::new(false);
static LAST_RESYNC: Mutex<Option<Instant>> = Mutex::new(None);

type Station = RadioStation<AppState, AppChannel>;

fn client() -> Option<std::sync::Arc<PlusClient>> {
    oneclient_polyplus::client()
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

fn select_group(station: Station, group_id: i32) {
    let mut guard = station.clone().write_channel(AppChannel::Chat);
    guard.chat.active = Some(group_id);
    guard.chat.mark_read(group_id);
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
            crate::hooks::invalidate_chat_queries().await;
        }
        Err(err) => report(station, &err),
    }
}

pub async fn load_conversations(station: Station) {
    let Some(client) = client() else { return };

    {
        let mut guard = station.clone().write_channel(AppChannel::Chat);
        if guard.chat.conversations().is_empty() {
            guard.chat.status = AsyncStatus::Loading;
        }
    }

    match client.groups().await {
        Ok(groups) => station
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .set_conversations(groups),
        Err(err) => report(station, &err),
    }
}

pub async fn load_messages(station: Station, group_id: i32) {
    let Some(client) = client() else { return };

    match client.messages(group_id, None, Some(PAGE_SIZE)).await {
        Ok(page) => station
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .set_messages(group_id, page),
        Err(err) => report(station, &err),
    }
}

pub async fn load_older_messages(station: Station, group_id: i32) {
    let Some(client) = client() else { return };

    let before = station.peek().chat.oldest_message(group_id);
    if before.is_none() {
        return;
    }

    match client
        .messages(group_id, before, Some(PAGE_SIZE.min(MAX_PAGE_SIZE)))
        .await
    {
        Ok(page) if page.is_empty() => {}
        Ok(page) => station
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .prepend_messages(group_id, page),
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
        if let Ok(mut last) = LAST_RESYNC.lock() {
            *last = Some(Instant::now());
        }

        RESYNC_RUNNING.store(false, Ordering::Release);
    }
}

fn claim_resync(floor: Option<Duration>) -> Option<ResyncGuard> {
    if let Some(floor) = floor
        && let Ok(last) = LAST_RESYNC.lock()
        && last.is_some_and(|at| at.elapsed() < floor)
    {
        return None;
    }

    RESYNC_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| ResyncGuard)
}

async fn resync(station: Station, floor: Option<Duration>) {
    let Some(_guard) = claim_resync(floor) else {
        return;
    };

    load_conversations(station).await;

    let active = station.peek().chat.active;
    if let Some(group_id) = active {
        load_messages(station, group_id).await;
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

    match client.send_message(group_id, &content, key).await {
        Ok(message) => {
            let mut guard = station.clone().write_channel(AppChannel::Chat);
            guard.chat.finish_send(group_id, key, message.into());
            guard.chat.note_activity(group_id, content, false);
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

impl Actions {
    pub fn refresh_chat(&self) {
        let station = self.station();
        spawn_forever(async move { resync(station, None).await });
    }

    pub fn sync_chat(&self) {
        let station = self.station();
        spawn_forever(async move { resync(station, Some(RESYNC_FLOOR)).await });
    }

    pub fn open_conversation(&self, group_id: i32) {
        let station = self.station();
        select_group(station, group_id);
        spawn_forever(async move { load_group(station, group_id).await });
    }

    pub fn load_older_messages(&self, group_id: i32) {
        let station = self.station();
        spawn_forever(async move { load_older_messages(station, group_id).await });
    }

    pub fn send_chat_message(&self, group_id: i32, content: impl Into<String>) {
        let content = content.into().trim().to_string();
        if content.is_empty() || content.len() > MAX_MESSAGE_LENGTH {
            return;
        }

        let key = Uuid::new_v4();
        let station = self.station();
        {
            let mut guard = station.clone().write_channel(AppChannel::Chat);
            guard.chat.begin_send(group_id, key, content.clone());
        }

        spawn_forever(async move { deliver(station, group_id, key, content).await });
    }

    pub fn retry_chat_message(&self, group_id: i32, key: Uuid) {
        let station = self.station();
        let Some(content) = station
            .peek()
            .chat
            .pending_for(group_id)
            .iter()
            .find(|pending| pending.key == key)
            .map(|pending| pending.content.clone())
        else {
            return;
        };

        spawn_forever(async move { deliver(station, group_id, key, content).await });
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

        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            match client.edit_message(group_id, message_id, &content).await {
                Ok(message) => station
                    .clone()
                    .write_channel(AppChannel::Chat)
                    .chat
                    .upsert(group_id, message.into()),
                Err(err) => report(station, &err),
            }
        });
    }

    pub fn delete_chat_message(&self, group_id: i32, message_id: i64) {
        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            match client.delete_message(group_id, message_id).await {
                Ok(()) => station
                    .clone()
                    .write_channel(AppChannel::Chat)
                    .chat
                    .remove(group_id, message_id),
                Err(err) => report(station, &err),
            }
        });
    }

    pub fn start_direct_message(&self, player: Uuid) {
        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            match client.open_direct_message(player).await {
                Ok(summary) => {
                    load_conversations(station).await;
                    open_group(station, summary.id).await;
                }
                Err(err) => report(station, &err),
            }
        });
    }

    pub fn create_chat_group(&self, name: impl Into<String>, members: Vec<Uuid>) {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return;
        }

        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            match client.create_group(&name, &members).await {
                Ok(summary) => {
                    load_conversations(station).await;
                    open_group(station, summary.id).await;
                }
                Err(err) => report(station, &err),
            }
        });
    }

    pub fn add_friend(&self, username: impl Into<String>) {
        let username = username.into().trim().to_string();
        if username.is_empty() {
            return;
        }

        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };

            match client.lookup_username(&username).await {
                Ok(Some(player)) => {
                    roster_change(station, client.send_friend_request(player).await.map(drop))
                        .await;
                }
                Ok(None) => fail(station, format!("No player named {username}.")),
                Err(err) => report(station, &err),
            }
        });
    }

    pub fn accept_friend_request(&self, request_id: i32) {
        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            roster_change(station, client.accept_request(request_id).await).await;
            load_conversations(station).await;
        });
    }

    pub fn decline_friend_request(&self, request_id: i32) {
        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            roster_change(station, client.decline_request(request_id).await).await;
        });
    }

    pub fn cancel_friend_request(&self, request_id: i32) {
        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            roster_change(station, client.cancel_request(request_id).await).await;
        });
    }

    pub fn remove_friend(&self, player: Uuid) {
        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            roster_change(station, client.remove_friend(player).await).await;
        });
    }

    pub fn block_player(&self, player: Uuid) {
        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            roster_change(station, client.block(player).await).await;
            load_conversations(station).await;
        });
    }

    pub fn unblock_player(&self, player: Uuid) {
        let station = self.station();
        spawn_forever(async move {
            let Some(client) = client() else { return };
            roster_change(station, client.unblock(player).await).await;
        });
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
        }

        spawn_forever(async move {
            oneclient_polyplus::forget_token().await;

            if owner.is_some() {
                crate::hooks::invalidate_chat_queries().await;
                resync_chat(station).await;
            }
        });
    }
}
