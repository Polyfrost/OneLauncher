//! Runs under freya's `spawn` not `tokio::spawn` radio state is `!Send`
//! Events are batched and folded because a radio write wakes every subscriber

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use freya::prelude::spawn_forever;
use freya::radio::RadioStation;
use oneclient_events::{Event, EventReceiver, GameEvent, LaunchStage, ProgressEvent, Signal};
use tokio::sync::mpsc;

use crate::hooks::PumpSignal;
use crate::notifications::{MESSAGE_TOAST_TTL, PendingPromptView};
use crate::state::{AppChannel, AppState, LoginProgress};

/// Quiet period before log lines are written without it every line wakes the log view
const GAME_LOG_FLUSH: Duration = Duration::from_millis(120);

const COALESCE_BUDGET: usize = 1024;

pub struct EventPump {
    pub events: EventReceiver,
    pub signals: mpsc::UnboundedReceiver<PumpSignal>,
    pub station: RadioStation<AppState, AppChannel>,
}

impl EventPump {
    pub async fn run(mut self) {
        let mut armed: HashMap<u64, ToastTimer> = HashMap::new();
        // Hovering any toast pauses every toast including ones arriving while hovering
        let mut paused = false;
        let mut game_flush: Option<tokio::time::Instant> = None;

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
                        // Lines were already folded in on arrival this only ends the quiet period
                        self.station.write_channel(AppChannel::Game);
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

                    let folded = self.apply(batch);
                    if folded.touched_engine {
                        reconcile(&self.station, &mut armed, paused);
                    }
                    if folded.saw_game_log && game_flush.is_none() {
                        game_flush = Some(tokio::time::Instant::now() + GAME_LOG_FLUSH);
                    }
                }

                signal = self.signals.recv() => {
                    let Some(signal) = signal else { break };
                    match signal {
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
    }

    fn apply(&mut self, batch: Vec<Event>) -> Folded {
        let mut folded = Folded::default();
        let mut engine_events = Vec::new();
        let mut stages: Vec<(i64, LaunchStage)> = Vec::new();
        let mut logs: Vec<(i64, String)> = Vec::new();
        let mut failed: Option<(i64, String)> = None;
        let mut login: Option<Option<LoginProgress>> = None;
        let mut sync_complete = false;

        for event in batch {
            match event {
                Event::Signal(Signal::ClustersChanged) => folded.clusters = true,
                Event::Signal(Signal::JavaChanged) => folded.java = true,
                Event::Signal(Signal::SyncComplete) => {
                    sync_complete = true;
                    folded.clusters = true;
                }
                Event::Game(GameEvent::Stage { cluster_id, stage }) => {
                    stages.push((cluster_id, stage));
                }
                Event::Game(GameEvent::Log { cluster_id, line }) => {
                    folded.saw_game_log = true;
                    logs.push((cluster_id, line));
                }
                Event::Game(GameEvent::Failed {
                    cluster_id,
                    message,
                }) => failed = Some((cluster_id, message)),
                // Lifted out so it never reaches the engine the sign-in modal renders it inline
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
                other => engine_events.push(other),
            }
        }

        if !stages.is_empty() || !logs.is_empty() || failed.is_some() {
            let mut guard = self.station.write_channel(AppChannel::Game);
            for (cluster_id, stage) in stages {
                guard.game.stages.insert(cluster_id, stage);
                if stage == LaunchStage::Checking {
                    guard.game.error = None;
                    guard.game.logs.insert(cluster_id, std::sync::Arc::new(Vec::new()));
                }
            }
            for (cluster_id, line) in logs {
                let buffer = guard.game.logs.entry(cluster_id).or_default();
                std::sync::Arc::make_mut(buffer).push(std::sync::Arc::from(line));
            }
            if let Some((cluster_id, message)) = failed {
                guard.game.stages.insert(cluster_id, LaunchStage::Exited);
                guard.game.error = Some(message);
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
            spawn_forever(async move { crate::hooks::invalidate_cluster_queries().await });
        }
        if folded.java {
            spawn_forever(async move { crate::hooks::invalidate_java_queries().await });
        }

        folded
    }
}

#[derive(Default)]
struct Folded {
    touched_engine: bool,
    saw_game_log: bool,
    clusters: bool,
    java: bool,
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
        armed.entry(*id).or_insert_with(|| ToastTimer::armed(paused));
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
    let state = crate::launcher::install(oneclient_core::LauncherState::new(events).await?);

    oneclient_net::status::start(state.services.requester.clone());
    oneclient_polyplus::start(std::sync::Arc::clone(&state.auth));
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
    Ok(())
}

pub fn report_startup_failure(
    station: &RadioStation<AppState, AppChannel>,
    err: &anyhow::Error,
) {
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
        notifications, inbox, ..
    } = &mut **guard;
    notifications.dispatch(
        inbox,
        Event::Notification(oneclient_events::Notification::Message(
            oneclient_events::Message {
                title: "Launcher failed to start".into(),
                body: message,
                level: oneclient_events::Level::Error,
            },
        )),
    );
}
