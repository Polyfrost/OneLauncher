//! Runs under freya's `spawn` not `tokio::spawn` radio state is `!Send`
//! Events are batched and folded because a radio write wakes every subscriber

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use freya::prelude::spawn_in_window;
use freya::radio::RadioStation;
use oneclient_events::{
    ChatEvent, Event, EventReceiver, GameEvent, LaunchStage, ProgressEvent, Signal,
};
use tokio::sync::mpsc;

use crate::chat::{ChatMessage, ChatToast};
use crate::hooks::{Actions, PumpSignal};
use crate::i18n::t;
use crate::notifications::{MESSAGE_TOAST_TTL, PendingPromptView};
use crate::state::{AppChannel, AppState, GameState, LoginProgress, StorageScanProgress};

/// Maximum time to batch live log updates before notifying game subscribers
const GAME_LOG_FLUSH: Duration = Duration::from_millis(120);

const COALESCE_BUDGET: usize = 1024;

pub struct EventPump {
    pub events: EventReceiver,
    pub signals: mpsc::UnboundedReceiver<PumpSignal>,
    pub station: RadioStation<AppState, AppChannel>,
    pub actions: Actions,
}

impl EventPump {
    pub async fn run(mut self) {
        let mut armed: HashMap<u64, ToastTimer> = HashMap::new();
        // Hovering any toast pauses every toast including ones arriving while hovering
        let mut paused = false;
        let mut game_flush: Option<tokio::time::Instant> = None;
        let mut pending_logs = Vec::new();
        let mut launched: HashSet<i64> = HashSet::new();
        let mut playing: Option<i64> = None;

        loop {
            let next_deadline = armed
                .values()
                .filter_map(ToastTimer::deadline)
                .min()
                .into_iter()
                .chain(game_flush)
                .min();

            let timer = async {
                match next_deadline {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending::<()>().await,
                }
            };

            tokio::select! {
                () = timer => {
                    let now = tokio::time::Instant::now();
                    let due: Vec<u64> = armed
                        .iter()
                        .filter(|(_, t)| t.deadline().is_some_and(|d| d <= now))
                        .map(|(id, _)| *id)
                        .collect();

                    if !due.is_empty() {
                        {
                            let mut guard = self.station.write_channel(AppChannel::Notifications);
                            let AppState {
                                notifications, inbox, ..
                            } = &mut **guard;
                            for id in &due {
                                notifications.expire_toast(inbox, *id);
                            }
                        }
                        for id in due {
                            armed.remove(&id);
                        }
                        reconcile(&self.station, &mut armed, paused);
                    }

                    if game_flush.is_some_and(|d| d <= now) {
                        game_flush = None;
                        self.flush_game_logs(&mut pending_logs);
                    }
                }

                event = self.events.recv() => {
                    let Some(first) = event else { break };
                    let mut batch = vec![first];
                    while batch.len() < COALESCE_BUDGET {
                        match self.events.try_recv() {
                            Ok(next) => batch.push(next),
                            Err(_) => break,
                        }
                    }

                    let folded = self.apply(batch, &mut pending_logs);
                    if folded.touched_engine {
                        reconcile(&self.station, &mut armed, paused);
                    }
                    if pending_logs.is_empty() {
                        game_flush = None;
                    } else if game_flush.is_none() {
                        game_flush = Some(tokio::time::Instant::now() + GAME_LOG_FLUSH);
                    }
                    if folded.saw_stage {
                        launched.extend(&folded.launched);
                        playing = crate::platform::follow_game(&self.station, &launched, playing);
                    }
                    if folded.crashed {
                        crate::platform::focus_window();
                    }
                }

                signal = self.signals.recv() => {
                    let Some(signal) = signal else { break };
                    match signal {
                        PumpSignal::Chat(job) => {
                            let station = self.station;
                            spawn_in_window(job.run(station));
                        }
                        PumpSignal::JoinInvite(invite_id) => {
                            let actions = self.actions.clone();
                            spawn_in_window(crate::hooks::start_invite_join(actions, invite_id));
                        }
                        PumpSignal::Reconcile => reconcile(&self.station, &mut armed, paused),
                        PumpSignal::PauseToasts => {
                            paused = true;
                            for timer in armed.values_mut() {
                                timer.pause();
                            }
                        }
                        PumpSignal::ResumeToasts => {
                            paused = false;
                            for timer in armed.values_mut() {
                                timer.resume();
                            }
                        }
                    }
                }
            }
        }

        self.flush_game_logs(&mut pending_logs);
    }

    fn flush_game_logs(&mut self, pending_logs: &mut Vec<(i64, String)>) {
        if !pending_logs.is_empty() {
            let mut guard = self.station.write_channel(AppChannel::Game);
            append_game_logs(&mut guard.game, pending_logs.drain(..));
        }
    }

    fn apply(&mut self, batch: Vec<Event>, pending_logs: &mut Vec<(i64, String)>) -> Folded {
        let mut folded = Folded::default();
        let mut engine_events = Vec::new();
        let mut stages: Vec<(i64, LaunchStage)> = Vec::new();
        let mut logs: Vec<(i64, String)> = Vec::new();
        let mut failed: Option<(i64, String)> = None;
        let mut crashed: Vec<oneclient_events::GameCrash> = Vec::new();
        let mut login: Option<Option<LoginProgress>> = None;
        let mut storage_scan: Option<Option<StorageScanProgress>> = None;
        let mut sync_complete = false;
        let mut chat_events: Vec<ChatEvent> = Vec::new();
        let mut presence: Vec<(uuid::Uuid, bool)> = Vec::new();

        for event in batch {
            match event {
                Event::Chat(ChatEvent::PresenceChanged { player, online }) => {
                    presence.push((player, online));
                }
                Event::Chat(event) => chat_events.push(event),
                Event::Signal(Signal::ClustersChanged) => {
                    folded.clusters = true;
                    crate::hooks::cloud_sync_signal(crate::hooks::CloudSignal::Changed(None));
                }
                Event::Signal(Signal::JavaChanged) => folded.java = true,
                Event::Signal(Signal::SyncComplete) => {
                    sync_complete = true;
                    folded.clusters = true;
                }
                Event::Game(GameEvent::Stage { cluster_id, stage }) => {
                    folded.saw_stage = true;
                    if stage == LaunchStage::Exited {
                        crate::hooks::cloud_sync_signal(crate::hooks::CloudSignal::Exited(
                            cluster_id,
                        ));
                    }
                    if stage.is_busy() {
                        folded.launched.push(cluster_id);
                    }
                    stages.push((cluster_id, stage));
                }
                Event::Game(GameEvent::Log { cluster_id, line }) => {
                    logs.push((cluster_id, line));
                }
                Event::Game(GameEvent::Failed {
                    cluster_id,
                    message,
                }) => {
                    folded.saw_stage = true;
                    failed = Some((cluster_id, message));
                }
                Event::Game(GameEvent::Crashed(crash)) => crashed.push(*crash),
                Event::Progress(ProgressEvent::Update {
                    id,
                    ref label,
                    current,
                    total,
                }) if id == oneclient_auth::MICROSOFT_LOGIN_PROGRESS => {
                    login = Some((current < total).then(|| LoginProgress {
                        label: label.clone(),
                        current,
                        total,
                    }));
                }
                Event::Progress(ProgressEvent::Update {
                    id,
                    ref label,
                    current,
                    total,
                }) if id == oneclient_core::storage::STORAGE_SCAN_PROGRESS => {
                    storage_scan = Some((current < total).then(|| StorageScanProgress {
                        label: label.clone(),
                        current,
                        total,
                    }));
                }
                other => engine_events.push(other),
            }
        }

        if stages.is_empty() && failed.is_none() && crashed.is_empty() {
            pending_logs.extend(logs);
        } else {
            let mut guard = self.station.write_channel(AppChannel::Game);
            folded.crashed =
                apply_game_status(&mut guard.game, pending_logs, stages, logs, failed, crashed);
        }

        let mut toasts: Vec<ChatToast> = Vec::new();
        let mut presence_lost = false;
        if !chat_events.is_empty() {
            let mut guard = self.station.write_channel(AppChannel::Chat);
            for event in chat_events {
                match event {
                    ChatEvent::MessageReceived {
                        account,
                        group_id,
                        message_id,
                        sender,
                        content,
                        invite_id,
                        invite_status,
                    } => {
                        if guard.chat.owner != Some(account) {
                            continue;
                        }

                        folded.chat_invites |= invite_id.is_some();

                        let visible = crate::view::chat::chat_on_screen(guard.social_open);
                        let tab = guard.social_tab;
                        let received = guard.chat.receive(
                            group_id,
                            ChatMessage {
                                id: message_id,
                                sender,
                                content,
                                sent_at: Some(chrono::Utc::now().fixed_offset()),
                                edited: false,
                                invite: invite_id
                                    .map(|id| crate::chat::MessageInvite::new(id, invite_status)),
                            },
                            visible,
                            tab,
                        );

                        folded.chat_unknown_group |= received.unknown_group;
                        if received.read && !folded.chat_read.contains(&group_id) {
                            folded.chat_read.push(group_id);
                        }
                        if let Some(toast) = received.toast {
                            toasts.retain(|known| known.group_id() != Some(group_id));
                            toasts.push(toast);
                        }
                    }
                    ChatEvent::MessageEdited {
                        group_id,
                        message_id,
                        content,
                        invite_status,
                    } => guard
                        .chat
                        .edit(group_id, message_id, content, invite_status),
                    ChatEvent::MessageDeleted {
                        group_id,
                        message_id,
                    } => guard.chat.remove(group_id, message_id),
                    ChatEvent::GroupRenamed { group_id, name } => guard.chat.rename(group_id, name),
                    ChatEvent::PresenceChanged { .. } => {}
                    ChatEvent::RosterChanged => folded.chat_roster = true,
                    ChatEvent::FriendRequestReceived { account, sender } => {
                        folded.chat_roster = true;
                        if guard.chat.owner == Some(account) && !guard.chat.is_friend(sender) {
                            toasts.push(ChatToast::FriendRequest { sender });
                        }
                    }
                    ChatEvent::ConnectionChanged { connected } => {
                        guard.chat.connected = connected;
                        if !connected {
                            presence.clear();
                            presence_lost = true;
                        }
                        folded.chat_reconnected |= connected;
                        folded.chat_roster |= connected;
                    }
                }
            }
        }

        if presence_lost || !presence.is_empty() {
            let mut guard = self.station.write_channel(AppChannel::ChatPresence);
            if presence_lost {
                guard.chat.clear_presence();
            }
            for (player, online) in presence {
                guard.chat.set_presence(player, online);
            }
        }

        if sync_complete {
            self.station
                .write_channel(AppChannel::Launcher)
                .launcher
                .fetching = false;
        }

        if let Some(progress) = login {
            self.station
                .write_channel(AppChannel::MicrosoftLogin)
                .microsoft_login = progress;
        }

        if let Some(progress) = storage_scan {
            self.station
                .write_channel(AppChannel::StorageScan)
                .storage_scan = progress;
        }

        if !engine_events.is_empty() {
            folded.touched_engine = true;
            let mut guard = self.station.write_channel(AppChannel::Notifications);
            let AppState {
                notifications,
                inbox,
                prompt,
                ..
            } = &mut **guard;
            for event in engine_events {
                if let (_timers, Some(pending)) = notifications.dispatch(inbox, event) {
                    *prompt = Some(pending);
                }
            }
        }

        if folded.clusters {
            spawn_in_window(async move { crate::hooks::invalidate_cluster_queries().await });
        }
        if folded.java {
            spawn_in_window(async move { crate::hooks::invalidate_java_queries().await });
        }
        if folded.chat_roster {
            let station = self.station;
            spawn_in_window(async move { crate::hooks::load_roster(station).await });
        }
        if folded.chat_invites {
            let station = self.station;
            spawn_in_window(async move { crate::hooks::load_invite_games(station).await });
        }
        if folded.chat_unknown_group {
            let station = self.station;
            spawn_in_window(async move { crate::hooks::load_conversations(station).await });
        }
        if folded.chat_reconnected {
            let station = self.station;
            spawn_in_window(async move { crate::hooks::resync_chat(station).await });
            spawn_in_window(async move { crate::hooks::load_self_status(station).await });
        }
        for toast in toasts {
            let actions = self.actions.clone();
            spawn_in_window(async move { crate::hooks::notify_chat(actions, toast).await });
        }
        for group_id in std::mem::take(&mut folded.chat_read) {
            let station = self.station;
            spawn_in_window(async move { crate::hooks::mark_chat_read(station, group_id).await });
        }

        folded
    }
}

fn append_game_logs(game: &mut GameState, logs: impl IntoIterator<Item = (i64, String)>) {
    for (cluster_id, line) in logs {
        let buffer = game.logs.entry(cluster_id).or_default();
        std::sync::Arc::make_mut(buffer).push(std::sync::Arc::from(line));
    }
}

fn apply_game_status(
    game: &mut GameState,
    pending_logs: &mut Vec<(i64, String)>,
    stages: Vec<(i64, LaunchStage)>,
    logs: Vec<(i64, String)>,
    failed: Option<(i64, String)>,
    crashed: Vec<oneclient_events::GameCrash>,
) -> bool {
    // Logs from earlier batches must land before Checking resets the buffer.
    append_game_logs(game, pending_logs.drain(..));
    for (cluster_id, stage) in stages {
        game.stages.insert(cluster_id, stage);
        if stage == LaunchStage::Checking {
            game.error = None;
            game.clear_crash(cluster_id);
            game.logs
                .insert(cluster_id, std::sync::Arc::new(Vec::new()));
        }
    }
    append_game_logs(game, logs);
    if let Some((cluster_id, message)) = failed {
        game.stages.insert(cluster_id, LaunchStage::Exited);
        game.error = Some(message);
    }
    let mut show_crash = false;
    for crash in crashed {
        if game.is_active(crash.cluster_id) {
            continue;
        }
        game.push_crash(crash);
        show_crash = true;
    }
    show_crash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checking_discards_only_its_previous_session_logs() {
        let mut game = GameState::default();
        let mut pending = vec![(1, "old session".into()), (2, "other instance".into())];

        apply_game_status(
            &mut game,
            &mut pending,
            vec![(1, LaunchStage::Checking)],
            vec![(1, "new session".into())],
            None,
            Vec::new(),
        );

        assert!(pending.is_empty());
        assert_eq!(game.logs[&1][0].as_ref(), "new session");
        assert_eq!(game.logs[&1].len(), 1);
        assert_eq!(game.logs[&2][0].as_ref(), "other instance");
    }

    #[test]
    fn failure_publishes_pending_and_current_logs_in_order() {
        let mut game = GameState::default();
        let mut pending = vec![(1, "first".into()), (1, "second".into())];

        apply_game_status(
            &mut game,
            &mut pending,
            Vec::new(),
            vec![(1, "third".into())],
            Some((1, "launch failed".into())),
            Vec::new(),
        );

        assert!(pending.is_empty());
        let lines: Vec<&str> = game.logs[&1].iter().map(AsRef::as_ref).collect();
        assert_eq!(lines, ["first", "second", "third"]);
        assert_eq!(game.stage(1), Some(LaunchStage::Exited));
        assert_eq!(game.error.as_deref(), Some("launch failed"));
    }
}

#[derive(Default)]
struct Folded {
    touched_engine: bool,
    saw_stage: bool,
    crashed: bool,
    launched: Vec<i64>,
    clusters: bool,
    java: bool,
    chat_roster: bool,
    chat_unknown_group: bool,
    chat_invites: bool,
    chat_reconnected: bool,
    chat_read: Vec<i32>,
}

fn reconcile(
    station: &RadioStation<AppState, AppChannel>,
    armed: &mut HashMap<u64, ToastTimer>,
    paused: bool,
) {
    let state = station.peek();
    let want: HashSet<u64> = state
        .notifications
        .active_toast_entry_ids()
        .into_iter()
        .filter(|id| state.inbox.iter().any(|e| e.id == *id && !e.is_loading))
        .collect();

    for id in &want {
        armed
            .entry(*id)
            .or_insert_with(|| ToastTimer::armed(paused));
    }
    armed.retain(|id, _| want.contains(id));
}

enum ToastTimer {
    Running(tokio::time::Instant),
    Paused(Duration),
}

impl ToastTimer {
    fn armed(paused: bool) -> Self {
        if paused {
            Self::Paused(MESSAGE_TOAST_TTL)
        } else {
            Self::Running(tokio::time::Instant::now() + MESSAGE_TOAST_TTL)
        }
    }

    /// `None` while paused
    fn deadline(&self) -> Option<tokio::time::Instant> {
        match self {
            Self::Running(deadline) => Some(*deadline),
            Self::Paused(_) => None,
        }
    }

    fn pause(&mut self) {
        if let Self::Running(deadline) = self {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            *self = Self::Paused(remaining);
        }
    }

    fn resume(&mut self) {
        if let Self::Paused(remaining) = self {
            let remaining = *remaining;
            *self = Self::Running(tokio::time::Instant::now() + remaining);
        }
    }
}

#[must_use]
pub fn prompt_view(state: &AppState) -> Option<PendingPromptView> {
    state.prompt.as_ref().map(|prompt| PendingPromptView {
        title: prompt.title.clone(),
        question: prompt.question.clone(),
        choices: prompt.choices.clone(),
        dismiss: prompt.dismiss.clone(),
    })
}

pub async fn start_launcher(
    station: RadioStation<AppState, AppChannel>,
    events: oneclient_events::EventBus,
) -> Result<(), anyhow::Error> {
    let state = crate::launcher::install(
        crate::launcher::off_ui(oneclient_core::LauncherState::new(events)).await?,
    );

    oneclient_net::status::start(state.services.requester.clone());
    crate::translations::start(state.services.requester.clone());
    oneclient_polyplus::start(
        std::sync::Arc::clone(&state.auth),
        state.services.events.clone(),
    );
    oneclient_core::run_startup_tasks(&state);

    let data_dir = oneclient_common::paths::data_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let settings = state.settings.read().clone();
    let auto_update = settings.auto_update;

    let mut station = station;
    {
        let mut guard = station.write_channel(AppChannel::Launcher);
        guard.launcher = crate::state::LauncherInit {
            ready: true,
            fetching: true,
            syncing_bundles: true,
            error: None,
            snapshots: 0,
            data_dir,
            needs_location: false,
        };
    }
    {
        let mut guard = station.write_channel(AppChannel::Settings);
        guard.settings.settings = settings;
        guard.settings.status = crate::state::AsyncStatus::Ready;
        guard.settings.error = None;
    }

    crate::hooks::invalidate_profile_queries().await;
    crate::updater::spawn_update_check(auto_update, state.services.events.clone());
    crate::servers::spawn_autostart(station);
    Ok(())
}

pub fn report_startup_failure(station: &RadioStation<AppState, AppChannel>, err: &anyhow::Error) {
    let message = err.to_string();
    tracing::error!("launcher init failed: {err:#}");

    let data_dir = oneclient_common::paths::data_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let mut station = *station;
    {
        let mut guard = station.write_channel(AppChannel::Launcher);
        guard.launcher.error = Some(message.clone());
        guard.launcher.data_dir = data_dir;
        guard.launcher.snapshots = crate::recovery::snapshots().len();
    }

    let mut guard = station.write_channel(AppChannel::Notifications);
    let AppState {
        notifications,
        inbox,
        ..
    } = &mut **guard;
    notifications.dispatch(
        inbox,
        Event::Notification(oneclient_events::Notification::Message(
            oneclient_events::Message {
                title: t("oneclient.events.launcher_failed").into(),
                body: message,
                level: oneclient_events::Level::Error,
                link: None,
            },
        )),
    );
}
