use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use freya::prelude::spawn_forever;
use oneclient_core::sync::{
    self, CloudSync, Resolution, SyncError, SyncOutcome, SyncReport, SyncStatus,
};
use oneclient_core::{LauncherError, LauncherResult, LauncherState};
use oneclient_db::models::ClusterId;
use oneclient_events::{Choice, Prompt};
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::{Actions, listed};
use crate::launcher::{self, off_ui};
use crate::state::{AppChannel, CloudSyncState};

const CHANGE_DEBOUNCE: Duration = Duration::from_secs(20);
const EXIT_DELAY: Duration = Duration::from_secs(3);
const FULL_SYNC_EVERY: Duration = Duration::from_secs(10 * 60);
const BEFORE_LAUNCH_LIMIT: Duration = Duration::from_secs(90);

static ENGINE: OnceLock<Option<Arc<CloudSync>>> = OnceLock::new();
static SIGNALS: OnceLock<mpsc::UnboundedSender<CloudSignal>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudSignal {
    Changed(Option<ClusterId>),
    Exited(ClusterId),
}

pub fn cloud_sync_signal(signal: CloudSignal) {
    if let Some(tx) = SIGNALS.get() {
        let _ = tx.send(signal);
    }
}

#[must_use]
pub fn cloud_engine() -> Option<Arc<CloudSync>> {
    if let Some(engine) = ENGINE.get() {
        return engine.clone();
    }
    let state = launcher::state().ok()?;
    ENGINE
        .get_or_init(|| match CloudSync::from_defaults(state.auth.clone()) {
            Ok(engine) => Some(Arc::new(engine)),
            Err(err) => {
                tracing::warn!(error = %err, "cloud sync is unavailable");
                None
            }
        })
        .clone()
}

pub async fn forget_deleted_cluster(state: &LauncherState, cluster_id: ClusterId) {
    let Some(engine) = cloud_engine() else {
        return;
    };
    let account = sync::current_account(state).await.ok();
    if let Err(err) = engine.cluster_deleted(account.as_ref(), cluster_id).await {
        tracing::warn!(cluster_id, error = %err, "could not remove the deleted instance from cloud sync");
    }
}

pub async fn load_sync_left_out(cluster_id: ClusterId) -> BTreeSet<String> {
    let Ok(engine) = engine_or_err() else {
        return BTreeSet::new();
    };
    match off_ui(async move { engine.left_out(cluster_id).await }).await {
        Ok(left_out) => left_out,
        Err(err) => {
            tracing::warn!(cluster_id, error = %err, "could not read which settings files stay on this computer");
            BTreeSet::new()
        }
    }
}

fn engine_or_err() -> LauncherResult<Arc<CloudSync>> {
    cloud_engine()
        .ok_or_else(|| SyncError::Unavailable("its folder could not be set up".into()).into())
}

fn quiet(err: &LauncherError) -> bool {
    matches!(
        err,
        LauncherError::SyncError(SyncError::NotSignedIn | SyncError::NotSynced)
    )
}

impl Actions {
    pub fn start_cloud_sync(&self) {
        let (tx, rx) = mpsc::unbounded_channel();
        if SIGNALS.set(tx).is_err() {
            return;
        }
        let actions = self.clone();
        spawn_forever(async move { actions.run_cloud_sync(rx).await });
    }

    async fn run_cloud_sync(self, mut rx: mpsc::UnboundedReceiver<CloudSignal>) {
        self.full_cloud_sync().await;
        let mut due: HashMap<ClusterId, Instant> = HashMap::new();
        let mut next_full = Instant::now() + FULL_SYNC_EVERY;
        loop {
            let wake = due
                .values()
                .min()
                .copied()
                .map_or(next_full, |at| at.min(next_full));
            tokio::select! {
                signal = rx.recv() => {
                    let Some(signal) = signal else { return };
                    self.schedule_cloud_sync(signal, &mut due);
                }
                () = tokio::time::sleep_until(wake) => {
                    let now = Instant::now();
                    if now >= next_full {
                        next_full = now + FULL_SYNC_EVERY;
                        self.full_cloud_sync().await;
                    }
                    let ready: Vec<ClusterId> = due
                        .iter()
                        .filter(|(_, at)| **at <= now)
                        .map(|(id, _)| *id)
                        .collect();
                    for cluster_id in ready {
                        due.remove(&cluster_id);
                        self.sync_cloud_cluster(cluster_id).await;
                    }
                }
            }
        }
    }

    fn synced_clusters(&self) -> Vec<ClusterId> {
        self.station
            .peek()
            .cloud_sync
            .statuses
            .keys()
            .copied()
            .collect()
    }

    fn cloud_status(&self, cluster_id: ClusterId) -> Option<SyncStatus> {
        self.station
            .peek()
            .cloud_sync
            .statuses
            .get(&cluster_id)
            .cloned()
    }

    fn schedule_cloud_sync(&self, signal: CloudSignal, due: &mut HashMap<ClusterId, Instant>) {
        let now = Instant::now();
        match signal {
            CloudSignal::Changed(None) => {
                for cluster_id in self.synced_clusters() {
                    due.entry(cluster_id).or_insert(now + CHANGE_DEBOUNCE);
                }
            }
            CloudSignal::Changed(Some(cluster_id)) => {
                if self.cloud_status(cluster_id).is_some() {
                    due.insert(cluster_id, now + CHANGE_DEBOUNCE);
                    self.set_cloud_status(cluster_id, SyncStatus::Pending);
                }
            }
            CloudSignal::Exited(cluster_id) => {
                if self.cloud_status(cluster_id).is_some() {
                    due.insert(cluster_id, now + EXIT_DELAY);
                    self.set_cloud_status(cluster_id, SyncStatus::Pending);
                }
            }
        }
    }

    fn set_cloud_status(&self, cluster_id: ClusterId, status: SyncStatus) {
        let mut guard = self.station.clone().write_channel(AppChannel::CloudSync);
        if let Some(current) = guard.cloud_sync.statuses.get_mut(&cluster_id)
            && !matches!(current, SyncStatus::Conflict { .. })
        {
            *current = status;
        }
    }

    pub async fn refresh_cloud_sync(&self) {
        let Some(engine) = cloud_engine() else { return };
        let Ok(state) = launcher::state() else { return };
        let fresh = off_ui(async move {
            match sync::current_account(&state).await {
                Ok(account) => match engine.statuses(&account).await {
                    Ok(statuses) => CloudSyncState {
                        signed_in: true,
                        statuses,
                    },
                    Err(err) => {
                        tracing::warn!(error = %err, "could not read the cloud sync state");
                        CloudSyncState {
                            signed_in: true,
                            statuses: HashMap::new(),
                        }
                    }
                },
                Err(_) => CloudSyncState::default(),
            }
        })
        .await;
        let mut guard = self.station.clone().write_channel(AppChannel::CloudSync);
        if guard.cloud_sync != fresh {
            guard.cloud_sync = fresh;
        }
    }

    async fn full_cloud_sync(&self) {
        let Some(engine) = cloud_engine() else { return };
        let Ok(state) = launcher::state() else { return };
        self.refresh_cloud_sync().await;
        let report = off_ui(async move {
            let account = sync::current_account(&state).await?;
            engine.sync_all(&state, &account).await
        })
        .await;
        self.refresh_cloud_sync().await;
        match report {
            Ok(report) => self.report_full_sync(report).await,
            Err(err) if quiet(&err) => {}
            Err(err) => tracing::warn!(error = %err, "cloud sync could not reach the cloud"),
        }
    }

    async fn report_full_sync(&self, report: SyncReport) {
        let Ok(state) = launcher::state() else { return };
        if !report.created.is_empty() || !report.pulled.is_empty() || !report.replaced.is_empty() {
            crate::hooks::invalidate_cluster_queries().await;
            crate::hooks::invalidate_cluster_content_queries().await;
        }
        let names = async |ids: &[ClusterId]| {
            let mut names = Vec::new();
            for id in ids {
                if let Ok(cluster) = state.clusters.get(*id).await {
                    names.push(cluster.name);
                }
            }
            names
        };

        let created = names(&report.created).await;
        if !created.is_empty() {
            self.notify("Synced instances added")
                .body(format!(
                    "{} came from your other computers. Mods download from Modrinth or CurseForge the first time you play.",
                    listed(created.into_iter())
                ))
                .send();
        }
        let conflicts = names(&report.conflicts).await;
        if !conflicts.is_empty() {
            self.notify("Sync conflict")
                .body(format!(
                    "{} changed here and on another computer. Open the instance settings to pick which copy to keep.",
                    listed(conflicts.into_iter())
                ))
                .error()
                .send();
        }
        let removed = names(&report.removed).await;
        if !removed.is_empty() {
            self.notify("Stopped syncing")
                .body(format!(
                    "{} was removed from the cloud on another computer. The copy here is kept.",
                    listed(removed.into_iter())
                ))
                .send();
        }
        for (old, new) in report.replaced {
            self.report_replaced(&state, old, new).await;
        }
        if !report.failed.is_empty() {
            tracing::warn!(failed = ?report.failed, "some instances could not sync");
            self.notify("Some instances couldn't sync")
                .body(
                    report
                        .failed
                        .iter()
                        .map(|(_, message)| message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .error()
                .send();
        }
    }

    async fn report_replaced(&self, state: &LauncherState, old: ClusterId, new: ClusterId) {
        let name = state
            .clusters
            .get(new)
            .await
            .map(|cluster| cluster.name)
            .unwrap_or_else(|_| "An instance".into());
        let old_name = state
            .clusters
            .get(old)
            .await
            .map(|cluster| cluster.name)
            .unwrap_or_else(|_| "the old one".into());
        self.notify("Synced instance moved to a new version")
            .body(format!(
                "{name} uses another Minecraft version on your other computer, so the cloud copy was set up as a new instance. The old copy, {old_name}, is kept and no longer syncs."
            ))
            .send();
    }

    async fn sync_cloud_cluster(&self, cluster_id: ClusterId) {
        let Ok(state) = launcher::state() else { return };
        if state.games.is_active(cluster_id)
            || super::modpack_job_running(cluster_id)
            || oneclient_core::clusters::is_resetting(cluster_id)
        {
            return;
        }
        let Ok(engine) = engine_or_err() else { return };
        self.set_cloud_status(cluster_id, SyncStatus::Syncing);
        let outcome = off_ui({
            let state = state.clone();
            async move {
                let account = sync::current_account(&state).await?;
                engine.sync_cluster(&state, &account, cluster_id).await
            }
        })
        .await;
        self.finish_cloud_sync(&state, cluster_id, outcome).await;
    }

    pub fn set_sync_left_out(&self, cluster_id: ClusterId, left_out: BTreeSet<String>) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let engine = match engine_or_err() {
                Ok(engine) => engine,
                Err(err) => {
                    actions.report_cloud_failure(&err.to_string());
                    return;
                }
            };
            actions.set_cloud_status(cluster_id, SyncStatus::Syncing);
            let outcome = off_ui({
                let state = state.clone();
                async move {
                    let account = sync::current_account(&state).await?;
                    engine
                        .set_left_out(&state, &account, cluster_id, left_out)
                        .await
                }
            })
            .await;
            actions.finish_cloud_sync(&state, cluster_id, outcome).await;
        });
    }

    async fn finish_cloud_sync(
        &self,
        state: &LauncherState,
        cluster_id: ClusterId,
        outcome: LauncherResult<SyncOutcome>,
    ) {
        self.refresh_cloud_sync().await;
        let name = state
            .clusters
            .get(cluster_id)
            .await
            .map(|cluster| cluster.name)
            .unwrap_or_else(|_| "This instance".into());
        match outcome {
            Ok(SyncOutcome::Pulled { .. } | SyncOutcome::Copied { .. }) => {
                crate::hooks::invalidate_cluster_queries().await;
                crate::hooks::invalidate_cluster_content_queries().await;
            }
            Ok(SyncOutcome::Replaced {
                old_cluster_id,
                cluster_id,
            }) => {
                crate::hooks::invalidate_cluster_queries().await;
                self.report_replaced(state, old_cluster_id, cluster_id)
                    .await;
            }
            Ok(SyncOutcome::Conflict { .. }) => {
                self.notify("Sync conflict")
                    .body(format!(
                        "{name} changed here and on another computer. Open its settings to pick which copy to keep."
                    ))
                    .error()
                    .send();
            }
            Ok(SyncOutcome::RemovedFromCloud) => {
                self.notify("Stopped syncing")
                    .body(format!(
                        "{name} was removed from the cloud on another computer. The copy here is kept."
                    ))
                    .send();
            }
            Ok(SyncOutcome::Unchanged | SyncOutcome::Pushed { .. }) => {}
            Err(err) if quiet(&err) => {}
            Err(err) => {
                tracing::warn!(cluster_id, error = %err, "cloud sync failed");
            }
        }
    }

    pub fn set_cloud_sync(&self, cluster_id: ClusterId, enabled: bool) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let engine = match engine_or_err() {
                Ok(engine) => engine,
                Err(err) => {
                    actions.report_cloud_failure(&err.to_string());
                    return;
                }
            };
            if enabled {
                actions
                    .station
                    .clone()
                    .write_channel(AppChannel::CloudSync)
                    .cloud_sync
                    .statuses
                    .insert(cluster_id, SyncStatus::Syncing);
            }
            let done = off_ui({
                let state = state.clone();
                async move {
                    let account = sync::current_account(&state).await?;
                    if enabled {
                        engine.enable(&state, &account, cluster_id).await.map(Some)
                    } else {
                        engine.disable(&account, cluster_id).await.map(|()| None)
                    }
                }
            })
            .await;
            actions.refresh_cloud_sync().await;
            match done {
                Ok(Some(outcome)) => {
                    actions
                        .finish_cloud_sync(&state, cluster_id, Ok(outcome))
                        .await
                }
                Ok(None) => {}
                Err(err) => actions.report_cloud_failure(&err.to_string()),
            }
        });
    }

    pub fn sync_cluster_now(&self, cluster_id: ClusterId) {
        let actions = self.clone();
        spawn_forever(async move { actions.sync_cloud_cluster(cluster_id).await });
    }

    pub fn resolve_cloud_conflict(&self, cluster_id: ClusterId, name: String) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let Ok(engine) = engine_or_err() else { return };
            let resolution = match actions.events().ask(conflict_prompt(&name)).await {
                Ok(Some(chosen)) => chosen.value,
                Ok(None) => return,
                Err(err) => {
                    tracing::warn!(cluster_id, "conflict prompt failed: {err:#}");
                    return;
                }
            };
            if state.games.is_active(cluster_id) {
                actions.report_cloud_failure("Close the game first, then pick a copy again.");
                return;
            }
            actions
                .station
                .clone()
                .write_channel(AppChannel::CloudSync)
                .cloud_sync
                .statuses
                .insert(cluster_id, SyncStatus::Syncing);
            let outcome = off_ui({
                let state = state.clone();
                async move {
                    let account = sync::current_account(&state).await?;
                    engine
                        .resolve(&state, &account, cluster_id, resolution)
                        .await
                }
            })
            .await;
            if let Err(err) = &outcome {
                actions.report_cloud_failure(&err.to_string());
            }
            if let Ok(SyncOutcome::Copied { .. }) = &outcome {
                actions
                    .notify("Kept both copies")
                    .body(format!(
                        "The cloud copy of {name} is now its own instance. Both keep syncing."
                    ))
                    .send();
            }
            actions.finish_cloud_sync(&state, cluster_id, outcome).await;
        });
    }

    pub(crate) async fn sync_before_launch(
        &self,
        state: &Arc<LauncherState>,
        cluster_id: ClusterId,
    ) {
        if self.cloud_status(cluster_id).is_none() {
            return;
        }
        let Ok(engine) = engine_or_err() else { return };
        self.set_cloud_status(cluster_id, SyncStatus::Syncing);
        let work = off_ui({
            let state = state.clone();
            async move {
                let account = sync::current_account(&state).await?;
                engine.sync_cluster(&state, &account, cluster_id).await
            }
        });
        let outcome = match tokio::time::timeout(BEFORE_LAUNCH_LIMIT, work).await {
            Ok(outcome) => outcome,
            Err(_) => {
                tracing::warn!(
                    cluster_id,
                    "cloud sync took too long before launch; launching the local copy"
                );
                Err(SyncError::Unavailable("it took too long".into()).into())
            }
        };
        if let Ok(SyncOutcome::Conflict { .. }) = &outcome {
            self.notify("Launching this computer's copy")
                .body("The cloud copy changed too. Pick which one to keep in the instance settings after you play.")
                .send();
            self.refresh_cloud_sync().await;
            return;
        }
        self.finish_cloud_sync(state, cluster_id, outcome).await;
    }

    fn report_cloud_failure(&self, message: &str) {
        self.notify("Cloud sync failed")
            .body(message.to_string())
            .error()
            .send();
    }
}

fn conflict_prompt(name: &str) -> Prompt<Resolution> {
    Prompt::new(
        format!("{name} changed in two places"),
        "This instance changed here and on another computer since it last synced. Pick the copy \
         to keep. Keeping both turns the cloud copy into its own instance. Worlds are never \
         touched.",
    )
    .option(
        Choice::primary("cloud_sync.keep_local", "Keep this computer's"),
        Resolution::KeepLocal,
    )
    .option(
        Choice::new("cloud_sync.keep_cloud", "Keep the cloud's"),
        Resolution::KeepCloud,
    )
    .option(
        Choice::new("cloud_sync.keep_both", "Keep both"),
        Resolution::KeepBoth,
    )
    .dismiss("Decide later")
}
