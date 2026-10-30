use std::cmp::Ordering;

use freya::prelude::spawn_forever;
use freya::radio::RadioStation;
use oneclient_cluster::SyncReport;
use oneclient_content::packages::fabric_version;
use oneclient_core::LauncherState;
use oneclient_db::models::ClusterId;
use oneclient_events::{Choice, Prompt};
use uuid::Uuid;

use super::actions::{Actions, listed};
use super::queries::invalidate_cluster_worlds;
use crate::i18n::{t, tf};
use crate::launcher::{self, off_ui};
use crate::state::{AppChannel, AppState};

impl Actions {
    pub fn sync_from_shared_dir(&self, cluster_id: ClusterId) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };

            let newer = newer_shared_versions(&state, cluster_id).await;
            let include_worlds = match actions.events().ask(sync_prompt(&newer)).await {
                Ok(Some(chosen)) => chosen.value,
                Ok(None) => return,
                Err(err) => {
                    tracing::warn!(cluster_id, "sync prompt failed: {err:#}");
                    return;
                }
            };
            if actions.refuse_during_update_check(cluster_id) {
                return;
            }

            let shared_dir = match oneclient_common::paths::shared_minecraft_dir() {
                Ok(dir) => dir,
                Err(err) => {
                    actions
                        .notify(t("oneclient.sync_actions.failed"))
                        .body(err.to_string())
                        .error()
                        .send();
                    return;
                }
            };
            let _sync = match state.games.begin_sync(cluster_id, &shared_dir) {
                Ok(sync) => sync,
                Err(block) => {
                    actions
                        .notify(t("oneclient.sync_actions.failed"))
                        .body(block.to_string())
                        .error()
                        .send();
                    return;
                }
            };
            if oneclient_core::clusters::is_resetting(cluster_id) {
                actions
                    .notify(t("oneclient.sync_actions.failed"))
                    .body(t("oneclient.sync_actions.resetting"))
                    .error()
                    .send();
                return;
            }
            let _shown = ShownSync::start(actions.station(), cluster_id);

            let events = actions.events();
            let progress_id = Uuid::new_v4();
            let worker = state.clone();
            let progress = events.clone();
            let result = off_ui(async move {
                worker
                    .clusters
                    .sync_from_shared(cluster_id, include_worlds, |label, done, total| {
                        progress.progress(progress_id, label, done, total)
                    })
                    .await
            })
            .await;
            invalidate_cluster_worlds().await;

            match result {
                Ok(report) => events.finish_progress(
                    progress_id,
                    t("oneclient.sync_actions.synced"),
                    report_summary(&report, include_worlds),
                ),
                Err(err) => events.fail_progress(
                    progress_id,
                    t("oneclient.sync_actions.failed"),
                    err.to_string(),
                ),
            }
        });
    }
}

pub(crate) async fn sync_launch_block(cluster_id: ClusterId) -> Option<&'static str> {
    let state = launcher::state().ok()?;
    if state.games.is_syncing(cluster_id) {
        return Some(t("oneclient.sync_actions.launch_block.this"));
    }
    if !state.games.any_syncing() {
        return None;
    }
    let shared = state
        .clusters
        .get(cluster_id)
        .await
        .is_ok_and(|cluster| !cluster.uses_dedicated_dir());
    shared.then(|| t("oneclient.sync_actions.launch_block.other"))
}

struct ShownSync {
    station: RadioStation<AppState, AppChannel>,
    cluster_id: ClusterId,
}

impl ShownSync {
    fn start(station: RadioStation<AppState, AppChannel>, cluster_id: ClusterId) -> Self {
        let shown = Self {
            station,
            cluster_id,
        };
        shown.set(true);
        shown
    }

    fn set(&self, syncing: bool) {
        self.station
            .clone()
            .write_channel(AppChannel::Game)
            .game
            .set_shared_sync(self.cluster_id, syncing);
    }
}

impl Drop for ShownSync {
    fn drop(&mut self) {
        self.set(false);
    }
}

async fn newer_shared_versions(state: &LauncherState, cluster_id: ClusterId) -> Vec<String> {
    let Ok(clusters) = state.clusters.list().await else {
        return Vec::new();
    };
    let Some(this) = clusters.iter().find(|cluster| cluster.id == cluster_id) else {
        return Vec::new();
    };

    let mut newer: Vec<String> = clusters
        .iter()
        .filter(|other| !other.is_isolated() && !other.uses_dedicated_dir())
        .filter(|other| {
            fabric_version::compare(&other.mc_version, &this.mc_version) == Ordering::Greater
        })
        .map(|other| other.mc_version.clone())
        .collect();
    newer.sort_by(|a, b| fabric_version::compare(a, b));
    newer.dedup();
    newer
}

fn sync_prompt(newer: &[String]) -> Prompt<bool> {
    let body = t("oneclient.sync_actions.prompt.body");

    if newer.is_empty() {
        return Prompt::new(t("oneclient.sync_actions.prompt.title"), body)
            .option(
                Choice::primary(
                    "sync_shared.everything",
                    t("oneclient.sync_actions.prompt.sync"),
                ),
                true,
            )
            .dismiss(t("oneclient.common.cancel"));
    }

    Prompt::new(
        t("oneclient.sync_actions.prompt.title"),
        tf(
            "oneclient.sync_actions.prompt.body_newer",
            &[&body, &listed(newer.iter().cloned())],
        ),
    )
    .option(
        Choice::new(
            "sync_shared.everything",
            t("oneclient.sync_actions.prompt.everything"),
        ),
        true,
    )
    .option(
        Choice::primary(
            "sync_shared.settings",
            t("oneclient.sync_actions.prompt.settings_only"),
        ),
        false,
    )
    .dismiss(t("oneclient.common.cancel"))
}

fn report_summary(report: &SyncReport, include_worlds: bool) -> String {
    if report.total == 0 {
        return t("oneclient.sync_actions.report.nothing").to_string();
    }
    if !include_worlds {
        return t("oneclient.sync_actions.report.settings").to_string();
    }
    if report.skipped.is_empty() {
        return t("oneclient.sync_actions.report.settings_and_worlds").to_string();
    }
    tf(
        "oneclient.sync_actions.report.skipped",
        &[&listed(report.skipped.iter().cloned())],
    )
}
