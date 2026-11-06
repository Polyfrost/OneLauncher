use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use freya::prelude::spawn_in_window;
use freya::query::{MutationStateData, UseMutation};
use freya::radio::RadioStation;
use oneclient_auth::MinecraftAccount;
use oneclient_common::GameLoader;
use oneclient_content::packages::{LinkedArtifactInfo, PackageStore};
use uuid::Uuid;

use oneclient_polyplus::{
    BlockedPlayer, Friend, FriendRequest, GroupSummary, MAX_MESSAGE_LENGTH, MAX_PAGE_SIZE,
    PlayerStatus, PlusClient, PlusError,
};

use crate::chat::{ChatRoster, ChatToast, FriendRequestStatus, FriendRequests, InviteGame};
use crate::components::IconType;
use crate::hooks::{Actions, ClusterAction, ClusterMutation, PumpSignal};
use crate::i18n::{t, tf};
use crate::essential::POLYPLUS;
use crate::invite_join::{self, InviteJoin};
use crate::launcher;
use crate::state::{AppChannel, AppState, AsyncStatus, InviteJoinPrompt, SocialTab};

const PAGE_SIZE: u64 = 50;

const RESYNC_FLOOR: Duration = Duration::from_secs(30);

const ROSTER_FLOOR: Duration = Duration::from_secs(60);

static RESYNC_RUNNING: AtomicBool = AtomicBool::new(false);
static RESYNC_QUEUED: AtomicBool = AtomicBool::new(false);
static LAST_RESYNC: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_ROSTER: Mutex<Option<Instant>> = Mutex::new(None);
static STATUS_TURN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static STATUS_UNSAVED: AtomicUsize = AtomicUsize::new(0);
static ROSTER_TICKET: AtomicU64 = AtomicU64::new(0);
static ROSTER_APPLIED: AtomicU64 = AtomicU64::new(0);
static CONVERSATIONS_TICKET: AtomicU64 = AtomicU64::new(0);
static CONVERSATIONS_APPLIED: AtomicU64 = AtomicU64::new(0);
static DRAFTS: LazyLock<Mutex<HashMap<(Uuid, i32), String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static JOINING: Mutex<Vec<i32>> = Mutex::new(Vec::new());

type Station = RadioStation<AppState, AppChannel>;

fn next_ticket(tickets: &AtomicU64) -> u64 {
    tickets.fetch_add(1, Ordering::SeqCst) + 1
}

fn is_newest(applied: &AtomicU64, ticket: u64) -> bool {
    applied.fetch_max(ticket, Ordering::SeqCst) < ticket
}

#[must_use]
pub fn chat_draft(owner: Uuid, group_id: i32) -> String {
    DRAFTS
        .lock()
        .ok()
        .and_then(|drafts| drafts.get(&(owner, group_id)).cloned())
        .unwrap_or_default()
}

pub fn save_chat_draft(owner: Uuid, group_id: i32, draft: String) {
    if let Ok(mut drafts) = DRAFTS.lock() {
        if draft.is_empty() {
            drafts.remove(&(owner, group_id));
        } else {
            drafts.insert((owner, group_id), draft);
        }
    }
}

struct JoinClaim(i32);

impl Drop for JoinClaim {
    fn drop(&mut self) {
        if let Ok(mut joining) = JOINING.lock() {
            joining.retain(|invite_id| *invite_id != self.0);
        }
    }
}

fn claim_join(invite_id: i32) -> Option<JoinClaim> {
    let mut joining = JOINING.lock().ok()?;
    if joining.contains(&invite_id) {
        return None;
    }
    joining.push(invite_id);
    Some(JoinClaim(invite_id))
}

fn join_claimed(invite_id: i32) -> bool {
    JOINING
        .lock()
        .is_ok_and(|joining| joining.contains(&invite_id))
}

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

fn restore_invite(station: Station, invite_id: i32) {
    station
        .clone()
        .write_channel(AppChannel::Chat)
        .chat
        .restore_invite(invite_id);
}

async fn roster_and_conversations_change(station: Station, result: Result<(), PlusError>) {
    match result {
        Ok(()) => {
            clear_error(station);
            load_roster(station).await;
            load_conversations(station).await;
        }
        Err(err) => {
            load_conversations(station).await;
            report(station, &err);
        }
    }
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
    let ticket = next_ticket(&ROSTER_TICKET);

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
            if !is_newest(&ROSTER_APPLIED, ticket) {
                return;
            }

            if let Ok(mut last) = LAST_ROSTER.lock() {
                *last = Some(Instant::now());
            }

            station
                .clone()
                .write_channel(AppChannel::ChatRoster)
                .chat
                .set_roster(roster);
            watch_members(station);
        }
        Err(err) => report(station, &err),
    }
}

fn watch_members(station: Station) {
    oneclient_polyplus::watch_players(station.peek().chat.watched_players());
}

fn roster_is_fresh() -> bool {
    LAST_ROSTER
        .lock()
        .ok()
        .and_then(|last| *last)
        .is_some_and(|at| at.elapsed() < ROSTER_FLOOR)
}

pub async fn load_conversations(station: Station) {
    if !refresh_conversations(station).await {
        return;
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

async fn refresh_conversations(station: Station) -> bool {
    let Some(client) = client() else { return false };
    let owner = owner_of(station);

    {
        let mut guard = station.clone().write_channel(AppChannel::Chat);
        if guard.chat.conversations().is_empty() {
            guard.chat.status = AsyncStatus::Loading;
        }
    }

    let ticket = next_ticket(&CONVERSATIONS_TICKET);
    let groups = client.groups().await;
    if owner_of(station) != owner {
        return false;
    }

    match groups {
        Ok(groups) => {
            if !is_newest(&CONVERSATIONS_APPLIED, ticket) {
                return false;
            }

            station
                .clone()
                .write_channel(AppChannel::Chat)
                .chat
                .set_conversations(groups);
            watch_members(station);
        }
        Err(err) => {
            report(station, &err);
            return false;
        }
    }

    true
}

pub async fn load_invite_games(station: Station) {
    let Some(client) = client() else { return };
    let owner = owner_of(station);

    let invites = client.incoming_session_invites().await;
    if owner_of(station) != owner {
        return;
    }

    match invites {
        Ok(invites) => station
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .set_invite_games(&invites),
        Err(err) => tracing::warn!("could not load incoming world invites: {err:#}"),
    }
}

async fn invite_game(station: Station, invite_id: i32) -> Result<Option<InviteGame>, PlusError> {
    let Some(client) = client() else {
        return Ok(station.peek().chat.invite_game(invite_id).cloned());
    };
    let owner = owner_of(station);

    match client.incoming_session_invites().await {
        Ok(invites) => {
            let game = invites
                .iter()
                .find(|invite| invite.id == invite_id)
                .map(InviteGame::from);
            if owner_of(station) == owner {
                station
                    .clone()
                    .write_channel(AppChannel::Chat)
                    .chat
                    .set_invite_games(&invites);
            }
            Ok(game)
        }
        Err(err) => match station.peek().chat.invite_game(invite_id).cloned() {
            Some(game) => {
                tracing::warn!("using the cached world invite: {err:#}");
                Ok(Some(game))
            }
            None => Err(err),
        },
    }
}

struct InviteClusters {
    ready: Vec<i64>,
    unlinked: Option<i64>,
    oneclient: bool,
}

async fn invite_candidates(game: InviteGame) -> Option<InviteClusters> {
    let state = launcher::state().ok()?;
    let lookup = state.clone();

    launcher::off_ui(async move {
        let content = lookup.services.content();
        let clusters = match lookup.clusters.list().await {
            Ok(clusters) => clusters,
            Err(err) => {
                tracing::error!("could not list clusters for a world invite: {err:#}");
                return None;
            }
        };

        let mut ready = Vec::new();
        let mut unlinked = None;
        for cluster in clusters {
            if !invite_join::cluster_matches(&game, &cluster.mc_version, cluster.mc_loader) {
                continue;
            }
            let artifacts = PackageStore::list_linked_artifacts(cluster.id, &content)
                .await
                .unwrap_or_default();
            let local = local_polyplus(&artifacts, &content).await;
            if invite_join::has_polyplus(&artifacts, &local) {
                ready.push(cluster.id);
            } else {
                unlinked = unlinked.max(Some(cluster.id));
            }
        }

        let meta_url_base = lookup.services.requester.config().meta_url_base.clone();
        let metadata = lookup.versions.metadata(&meta_url_base).await;
        let oneclient = game.known().is_some_and(|(version, loader)| {
            invite_join::oneclient_supports(&metadata, version, loader)
        });

        Some(InviteClusters {
            unlinked: unlinked.filter(|_| ready.is_empty() && game.known().is_some()),
            ready,
            oneclient,
        })
    })
    .await
}

async fn polyplus_ready(
    cluster_id: i64,
    content: &oneclient_content::ContentCtx,
) -> anyhow::Result<bool> {
    let artifacts = PackageStore::list_linked_artifacts(cluster_id, content)
        .await
        .unwrap_or_default();
    let local = local_polyplus(&artifacts, content).await;
    if invite_join::has_polyplus(&artifacts, &local) {
        return Ok(true);
    }

    let Some(hash) = invite_join::disabled_polyplus(&artifacts, &local) else {
        return Ok(false);
    };
    oneclient_content::bundles::set_artifact_enabled_to(cluster_id, hash, true, content).await?;
    Ok(true)
}

async fn local_polyplus(
    artifacts: &[LinkedArtifactInfo],
    content: &oneclient_content::ContentCtx,
) -> Vec<String> {
    let mut hashes = Vec::new();
    for artifact in artifacts.iter().filter(|a| invite_join::maybe_local(a)) {
        let id = oneclient_content::packages::artifact_mod_id(&artifact.hash, content).await;
        if id.as_deref() == Some(invite_join::POLYPLUS_MOD_ID) {
            hashes.push(artifact.hash.clone());
        }
    }
    hashes
}

async fn add_polyplus(actions: &Actions, invite_id: i32) -> bool {
    let Ok(Some(game)) = invite_game(actions.station(), invite_id).await else {
        return true;
    };
    let Ok(state) = launcher::state() else {
        return false;
    };

    let lookup = state.clone();
    let added = launcher::off_ui(async move {
        let state = lookup;
        let content = state.services.content();
        let newest = state
            .clusters
            .list()
            .await?
            .into_iter()
            .filter(|cluster| {
                invite_join::cluster_matches(&game, &cluster.mc_version, cluster.mc_loader)
            })
            .max_by_key(|cluster| cluster.id)
            .ok_or_else(|| anyhow::anyhow!("the new cluster does not match the invite"))?;

        if polyplus_ready(newest.id, &content).await? {
            return Ok(None);
        }

        if newest.uses_bundles() {
            let session = oneclient_events::GroupedProgressSession::start(
                &state.services.events,
                tf("oneclient.mutations.setting_up", &[&newest.name]),
            );
            if let Err(err) = oneclient_content::bundles::install_cluster_bundles(
                newest.id,
                state.bundles.as_ref(),
                Some(&session),
                &content,
            )
            .await
            {
                tracing::warn!(cluster_id = newest.id, error = %err, "failed to install bundle content before joining an invite");
            }
            session.finish();

            if polyplus_ready(newest.id, &content).await? {
                return Ok(None);
            }
        }

        let provider = state.services.packages.get(POLYPLUS.provider)?;
        let target = oneclient_content::packages::find_mod_for_cluster(
            provider,
            POLYPLUS.project_id,
            Some(POLYPLUS.name),
            None,
            newest.id,
            &content,
        )
        .await?
        .ok_or_else(|| anyhow::anyhow!("no Poly+ version fits this cluster"))?;

        let install = crate::install::install_package(
            &state,
            POLYPLUS.provider,
            &target.project_id,
            &target.version_id,
            newest.id,
            None,
            false,
        )
        .await
        .map_err(|flagged| anyhow::anyhow!("{} is flagged", flagged.name))?;

        anyhow::Ok(Some((newest.id, install)))
    })
    .await;

    match added {
        Ok(None) => true,
        Ok(Some((cluster_id, install))) => {
            let installed = install.result.is_ok();
            actions
                .settle_install(&state, cluster_id, install, None)
                .await;
            installed
        }
        Err(err) => {
            tracing::error!("could not add Poly+ for a world invite: {err:#}");
            actions
                .notify(t("oneclient.components.invite_join.polyplus_failed"))
                .body(t("oneclient.components.invite_join.polyplus_failed_body"))
                .error()
                .toast_only()
                .send();
            false
        }
    }
}

async fn finish_invite_join(
    actions: Actions,
    invite_id: i32,
    owner: Option<Uuid>,
    claim: JoinClaim,
) {
    let same_owner = |actions: &Actions| owner_of(actions.station()) == owner;
    let ready = same_owner(&actions) && add_polyplus(&actions, invite_id).await;
    drop(claim);
    if ready && same_owner(&actions) {
        start_invite_join(actions, invite_id).await;
    }
}

pub async fn start_invite_join(actions: Actions, invite_id: i32) {
    if join_claimed(invite_id) {
        return;
    }

    let station = actions.station();

    let game = match invite_game(station, invite_id).await {
        Ok(Some(game)) => game,
        Ok(None) => {
            actions
                .notify(t("oneclient.components.invite_join.expired"))
                .body(t("oneclient.components.invite_join.expired_body"))
                .error()
                .toast_only()
                .send();
            return;
        }
        Err(err) => {
            report(station, &err);
            return;
        }
    };

    let Some(clusters) = invite_candidates(game.clone()).await else {
        return;
    };

    if clusters
        .ready
        .iter()
        .chain(&clusters.unlinked)
        .any(|id| station.peek().game.is_active(*id))
    {
        actions
            .notify(t("oneclient.components.invite_join.running_title"))
            .body(t("oneclient.components.invite_join.running_body"))
            .send();
        return;
    }

    let prompt = match invite_join::plan(
        &game,
        clusters.ready,
        clusters.unlinked.is_some(),
        clusters.oneclient,
    ) {
        InviteJoin::Launch(cluster_id) => {
            actions.close_social();
            actions.launch_cluster_for_invite(cluster_id, invite_id);
            return;
        }
        InviteJoin::Pick(clusters) => InviteJoinPrompt::Pick {
            invite_id,
            clusters,
        },
        InviteJoin::Create {
            version,
            loader,
            oneclient,
            unlinked,
        } => InviteJoinPrompt::Create {
            invite_id,
            version,
            loader,
            oneclient,
            unlinked,
        },
        InviteJoin::Unknown => {
            actions
                .notify(t("oneclient.components.invite_join.unknown_title"))
                .body(t("oneclient.components.invite_join.unknown_body"))
                .send();
            return;
        }
    };

    set_invite_join(station, Some(prompt));
    crate::platform::focus_window();
}

fn set_invite_join(station: Station, prompt: Option<InviteJoinPrompt>) {
    station
        .clone()
        .write_channel(AppChannel::Social)
        .invite_join = prompt;
}

async fn load_names(station: Station) {
    let Some(client) = client() else { return };
    let owner = owner_of(station);

    let players = station.peek().chat.unnamed_players();
    if players.is_empty() {
        return;
    }

    let resolved = client.resolve_players(&players).await;
    if owner_of(station) != owner {
        return;
    }

    match resolved {
        Ok(resolved) => station
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .add_names(resolved.into_iter().map(|player| (player.id, player.username))),
        Err(err) => tracing::warn!("could not resolve player names: {err:#}"),
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
    let known = station.peek().chat.synced_to(group_id);
    let limit = PAGE_SIZE.min(MAX_PAGE_SIZE);
    let page = client.messages(group_id, None, Some(limit)).await;
    if owner_of(station) != owner {
        return;
    }

    match page {
        Ok(page) => {
            let complete = (page.len() as u64) < limit;
            let mut guard = station.clone().write_channel(AppChannel::Chat);
            guard.chat.set_messages(group_id, page, known);
            guard.chat.set_history_complete(group_id, complete);
            guard.chat.settle_thread_load(group_id);
        }
        Err(err) if err.is_forbidden() => {
            tracing::info!("no longer a member of conversation {group_id}");
            station
                .clone()
                .write_channel(AppChannel::Chat)
                .chat
                .forget_conversation(group_id);
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

    let owner = owner_of(station);
    if let Err(err) = client.mark_read(group_id, message_id).await {
        tracing::warn!("could not mark conversation {group_id} read: {err:#}");
        return;
    }

    if owner_of(station) != owner
        || station.peek().chat.newest_message(group_id) != Some(message_id)
    {
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
    refresh_conversations(station).await;
    load_invite_games(station).await;

    let active = station.peek().chat.active;
    let unread: Vec<i32> = station
        .peek()
        .chat
        .conversations()
        .iter()
        .filter(|conversation| conversation.unread && Some(conversation.id) != active)
        .map(|conversation| conversation.id)
        .collect();
    for group_id in unread {
        load_messages(station, group_id).await;
    }

    if let Some(group_id) = active {
        load_messages(station, group_id).await;
        if unread_in_view(station, group_id) {
            mark_read(station, group_id).await;
        }
    }
}

fn unread_in_view(station: Station, group_id: i32) -> bool {
    let state = station.peek();
    crate::chat::thread_in_view(
        crate::view::chat::chat_on_screen(state.social_open),
        state.social_tab,
        state.chat.active,
        group_id,
    ) && state
        .chat
        .conversation(group_id)
        .is_some_and(|conversation| conversation.unread)
}

async fn resync(station: Station, floor: Option<Duration>) {
    if floor.is_some_and(resynced_within) {
        let active = station.peek().chat.active;
        if let Some(group_id) = active.filter(|group_id| unread_in_view(station, *group_id)) {
            mark_read(station, group_id).await;
        }
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
    DeclineInvite(i32),
    CancelRequest(i32),
    RemoveFriend(Uuid),
    Block(Uuid),
    Unblock(Uuid),
    SetStatus {
        status: PlayerStatus,
        owner: Option<Uuid>,
    },
    AddMembers {
        group_id: i32,
        members: Vec<Uuid>,
    },
    LeaveGroup(i32),
    ResolveNames,
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
                enter_new_group(station, owner, opened).await;
            }
            Self::CreateGroup { name, members } => {
                let Some(client) = client() else { return };
                let owner = owner_of(station);
                let created = client.create_group(&name, &members).await;
                enter_new_group(station, owner, created).await;
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
                    Ok(Some(player)) if station.peek().chat.is_friend(player) => {
                        settle_friend_request(
                            station,
                            FriendRequestStatus::Failed(tf(
                                "oneclient.chat.errors.already_friends",
                                &[&username],
                            )),
                        );
                        return;
                    }
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
                roster_and_conversations_change(station, client.accept_request(request_id).await)
                    .await;
            }
            Self::DeclineInvite(invite_id) => {
                let Some(client) = client() else {
                    restore_invite(station, invite_id);
                    return;
                };
                let owner = owner_of(station);

                let declined = client.decline_session_invite(invite_id).await;
                if owner_of(station) != owner {
                    return;
                }

                match declined {
                    Err(err) if !err.is_not_found() => {
                        restore_invite(station, invite_id);
                        report(station, &err);
                    }
                    _ => {}
                }
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
                roster_and_conversations_change(station, client.block(player).await).await;
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
                load_conversations(station).await;
                if let Some(err) = failure {
                    report(station, &err);
                }
            }
            Self::ResolveNames => load_names(station).await,
            Self::LeaveGroup(group_id) => {
                let Some(client) = client() else { return };
                let Some(owner) = owner_of(station) else {
                    return;
                };

                let left = client.remove_member(group_id, owner).await;
                if owner_of(station) != Some(owner) {
                    return;
                }

                if left.is_ok() {
                    station
                        .clone()
                        .write_channel(AppChannel::Chat)
                        .chat
                        .forget_conversation(group_id);
                }
                load_conversations(station).await;
                if let Err(err) = left {
                    report(station, &err);
                }
            }
            Self::SetStatus { status, owner } => {
                let result = {
                    let _turn = STATUS_TURN.lock().await;
                    match client().filter(|_| owner_of(station) == owner) {
                        Some(client) => Some(client.set_status(status).await),
                        None => None,
                    }
                };
                STATUS_UNSAVED.fetch_sub(1, Ordering::SeqCst);
                if owner_of(station) != owner {
                    return;
                }

                if let Some(Err(err)) = result {
                    report(station, &err);
                    load_self_status(station).await;
                }
            }
        }
    }
}

async fn enter_new_group(
    station: Station,
    owner: Option<Uuid>,
    opened: Result<GroupSummary, PlusError>,
) {
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

const TOAST_PREVIEW_CHARS: usize = 120;

pub async fn notify_chat(actions: Actions, toast: ChatToast) {
    let owner = owner_of(actions.station());
    let name = player_name(toast.sender()).await;
    if owner_of(actions.station()) != owner {
        return;
    }

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
            t("oneclient.chat.notify.invite_body").to_string(),
            false,
        ),
        ChatToast::FriendRequest { .. } => (
            tf("oneclient.chat.notify.friend_request", &[&name]),
            t("oneclient.chat.notify.friend_request_body").to_string(),
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

pub async fn player_name(player: Uuid) -> String {
    let fallback = crate::chat::short_player_id(player);

    launcher::off_ui(async move {
        let state = launcher::state().ok()?;
        oneclient_mc::fetch_player_profile_view(
            &state.services.requester,
            &player.to_string(),
            None,
        )
        .await
        .ok()
        .map(|view| view.username)
    })
    .await
    .unwrap_or(fallback)
}

pub async fn load_self_status(station: Station) {
    let Some(client) = client() else { return };
    let _turn = STATUS_TURN.lock().await;
    let owner = owner_of(station);

    let status = client.status().await;
    if owner_of(station) != owner {
        return;
    }

    match status {
        Ok(_) if STATUS_UNSAVED.load(Ordering::SeqCst) > 0 => {}
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

    pub fn join_invite(&self, invite_id: i32) {
        self.nudge(PumpSignal::JoinInvite(invite_id));
    }

    pub fn create_cluster_and_join(
        &self,
        mutation: UseMutation<ClusterMutation>,
        action: ClusterAction,
        invite_id: i32,
    ) {
        let Some(claim) = claim_join(invite_id) else {
            return;
        };
        let actions = self.clone();
        let owner = owner_of(self.station());
        spawn_in_window(async move {
            let failed = matches!(
                &*mutation.mutate_async(action).await.state(),
                MutationStateData::Settled { res: Err(_), .. }
            );
            if !failed {
                finish_invite_join(actions, invite_id, owner, claim).await;
            }
        });
    }

    pub fn link_polyplus_and_join(&self, invite_id: i32) {
        let Some(claim) = claim_join(invite_id) else {
            return;
        };
        let owner = owner_of(self.station());
        spawn_in_window(finish_invite_join(self.clone(), invite_id, owner, claim));
    }

    pub fn clear_invite_join(&self) {
        set_invite_join(self.station(), None);
    }

    pub fn custom_invite_cluster(&self, invite_id: i32, version: String, loader: GameLoader) {
        set_invite_join(
            self.station(),
            Some(InviteJoinPrompt::Custom {
                invite_id,
                version,
                loader,
            }),
        );
    }

    pub fn resolve_chat_names(&self) {
        self.chat_job(ChatJob::ResolveNames);
    }

    pub fn sync_chat(&self) {
        self.chat_job(ChatJob::Resync { floored: true });
    }

    pub fn close_conversation(&self) {
        self.station()
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .active = None;
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
            let mut guard = self.station().clone().write_channel(AppChannel::Chat);
            guard.chat.active_opened = guard.chat.active_opened.wrapping_add(1);
        } else {
            let mut guard = self.station().clone().write_channel(AppChannel::Chat);
            guard.chat.active = None;
            guard.chat.open_friends = true;
        }

        let popped = crate::view::chat::is_chat_window_open();
        {
            let mut guard = self.station().clone().write_channel(AppChannel::Social);
            guard.social_tab = SocialTab::Chat;
            if !popped {
                guard.social_open = true;
                guard.social_popped_notice = false;
            }
        }
        if popped {
            crate::view::chat::open_chat_window();
        }
    }

    pub fn friends_shown(&self) {
        self.station()
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .open_friends = false;
    }

    pub fn show_party(&self) {
        let popped = crate::view::chat::is_chat_window_open();
        {
            let mut guard = self.station().clone().write_channel(AppChannel::Social);
            guard.social_tab = SocialTab::Party;
            if !popped {
                guard.social_open = true;
                guard.social_popped_notice = false;
            }
        }
        if popped {
            crate::view::chat::open_chat_window();
        }
    }

    pub fn set_social_tab(&self, tab: SocialTab) {
        if self.station().peek().social_tab == tab {
            return;
        }
        self.station()
            .clone()
            .write_channel(AppChannel::Social)
            .social_tab = tab;

        let active = self.station().peek().chat.active;
        if tab == SocialTab::Chat
            && let Some(group_id) = active
        {
            self.open_conversation(group_id);
        }
    }

    pub fn set_self_status(&self, status: PlayerStatus) {
        let station = self.station();
        let mut guard = station.clone().write_channel(AppChannel::Chat);
        if guard.chat.self_status == status {
            return;
        }
        guard.chat.self_status = status;
        let owner = guard.chat.owner;
        drop(guard);

        STATUS_UNSAVED.fetch_add(1, Ordering::SeqCst);
        self.chat_job(ChatJob::SetStatus { status, owner });
    }

    pub fn decline_chat_invite(&self, invite_id: i32) {
        self.station()
            .clone()
            .write_channel(AppChannel::Chat)
            .chat
            .dismiss_invite(invite_id);
        self.chat_job(ChatJob::DeclineInvite(invite_id));
    }

    pub fn sync_chat_owner(&self, account: Option<&MinecraftAccount>) {
        let owner = account
            .filter(|account| account.is_microsoft())
            .map(|account| account.id);
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

        set_invite_join(station.clone(), None);
        self.clear_chat_notices();

        if owner.is_none() {
            crate::view::chat::close_chat_window();
            self.close_social();
        }

        self.chat_job(ChatJob::SwitchOwner {
            signed_in: owner.is_some(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_reply_loses_to_a_newer_one() {
        let applied = AtomicU64::new(0);
        assert!(is_newest(&applied, 2));
        assert!(!is_newest(&applied, 1));
        assert!(is_newest(&applied, 3));
    }

    #[test]
    fn a_join_in_progress_blocks_a_second_one_until_it_ends() {
        let claim = claim_join(9_001).expect("first claim");
        assert!(join_claimed(9_001));
        assert!(claim_join(9_001).is_none());

        drop(claim);
        assert!(!join_claimed(9_001));
    }

    #[test]
    fn a_draft_belongs_to_one_account_and_group() {
        let owner = Uuid::from_u128(77);
        save_chat_draft(owner, 1, "hello".to_string());

        assert_eq!(chat_draft(owner, 1), "hello");
        assert_eq!(chat_draft(owner, 2), "");
        assert_eq!(chat_draft(Uuid::from_u128(78), 1), "");

        save_chat_draft(owner, 1, String::new());
        assert_eq!(chat_draft(owner, 1), "");
    }
}
