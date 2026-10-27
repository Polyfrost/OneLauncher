use std::cmp::Ordering;

use freya::prelude::spawn_forever;
use freya::radio::RadioStation;
use oneclient_cluster::SyncReport;
use oneclient_content::packages::fabric_version;
use oneclient_core::LauncherState;
use oneclient_core::clusters::DirTarget;
use oneclient_db::models::ClusterId;
use oneclient_events::{Choice, Prompt};
use uuid::Uuid;

use super::actions::{Actions, listed};
use super::queries::invalidate_cluster_worlds;
use crate::launcher::{self, off_ui};
use crate::state::{AppChannel, AppState};

impl Actions {
    pub fn sync_from_shared_dir(&self, cluster_id: ClusterId, from: Option<String>) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };

            let newer = newer_shared_versions(&state, cluster_id, from.as_deref()).await;
            let neighbours = folder_neighbours(&state, cluster_id).await;
            let include_worlds = match actions.events().ask(sync_prompt(&newer, &neighbours)).await
            {
                Ok(Some(chosen)) => chosen.value,
                Ok(None) => return,
                Err(err) => {
                    tracing::warn!(cluster_id, "sync prompt failed: {err:#}");
                    return;
                }
            };

            let dirs = async {
                let shared = oneclient_common::paths::shared_group_dir(from.as_deref())?;
                let dest = state.clusters.get(cluster_id).await?.game_dir()?;
                Ok::<_, oneclient_cluster::ClusterError>((shared, dest))
            }
            .await;
            let (shared_dir, dest) = match dirs {
                Ok(dirs) => dirs,
                Err(err) => {
                    actions
                        .notify("Couldn't sync from the shared folder")
                        .body(err.to_string())
                        .error()
                        .send();
                    return;
                }
            };
            let _sync = match state.games.begin_sync(cluster_id, &[&shared_dir, &dest]) {
                Ok(sync) => sync,
                Err(block) => {
                    actions
                        .notify("Couldn't sync from the shared folder")
                        .body(block.to_string())
                        .error()
                        .send();
                    return;
                }
            };
            if oneclient_core::clusters::is_resetting(cluster_id) {
                actions
                    .notify("Couldn't sync from the shared folder")
                    .body("This instance is being reset. Wait for the reset to finish, then try again.")
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
                    .sync_from_shared(
                        cluster_id,
                        from.as_deref(),
                        include_worlds,
                        |label, done, total| progress.progress(progress_id, label, done, total),
                    )
                    .await
            })
            .await;
            invalidate_cluster_worlds().await;

            match result {
                Ok(report) => events.finish_progress(
                    progress_id,
                    "Synced from the shared folder",
                    report_summary(&report, include_worlds),
                ),
                Err(err) => events.fail_progress(
                    progress_id,
                    "Couldn't sync from the shared folder",
                    err.to_string(),
                ),
            }
        });
    }
}

pub(crate) async fn sync_launch_block(cluster_id: ClusterId) -> Option<&'static str> {
    let state = launcher::state().ok()?;
    if state.games.is_syncing(cluster_id) {
        return Some(
            "This instance is syncing from the shared folder. Press Play again once it finishes.",
        );
    }
    if !state.games.any_syncing() {
        return None;
    }
    let shared = state
        .clusters
        .get(cluster_id)
        .await
        .is_ok_and(|cluster| !cluster.uses_dedicated_dir());
    shared.then_some(
        "An instance is syncing from the shared folder. Press Play again once it finishes.",
    )
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

async fn newer_shared_versions(
    state: &LauncherState,
    cluster_id: ClusterId,
    from: Option<&str>,
) -> Vec<String> {
    let Ok(clusters) = state.clusters.list().await else {
        return Vec::new();
    };
    let Some(this) = clusters.iter().find(|cluster| cluster.id == cluster_id) else {
        return Vec::new();
    };

    let mut newer: Vec<String> = clusters
        .iter()
        .filter(|other| other.dir_target() == DirTarget::Shared(from.map(str::to_owned)))
        .filter(|other| {
            fabric_version::compare(&other.mc_version, &this.mc_version) == Ordering::Greater
        })
        .map(|other| other.mc_version.clone())
        .collect();
    newer.sort_by(|a, b| fabric_version::compare(a, b));
    newer.dedup();
    newer
}

async fn folder_neighbours(state: &LauncherState, cluster_id: ClusterId) -> Vec<String> {
    let Ok(clusters) = state.clusters.list().await else {
        return Vec::new();
    };
    let Some(this) = clusters.iter().find(|cluster| cluster.id == cluster_id) else {
        return Vec::new();
    };
    clusters
        .iter()
        .filter(|other| other.id != cluster_id && other.shares_dir_with(this))
        .map(|other| other.name.clone())
        .collect()
}

fn sync_prompt(newer: &[String], neighbours: &[String]) -> Prompt<bool> {
    const COPIED: &str = "Copy settings, keybinds, the server list and mod configs from the \
        shared folder";
    const WORLDS: &str = "Worlds missing here are copied too; worlds already there are left \
        untouched.";
    let body = if neighbours.is_empty() {
        format!("{COPIED} into this instance, replacing its own. {WORLDS}")
    } else {
        format!(
            "{COPIED} into the folder this instance uses, replacing the ones there. {} also \
             {} that folder, so their settings change too. {WORLDS}",
            listed(neighbours.iter().cloned()),
            if neighbours.len() == 1 { "uses" } else { "use" }
        )
    };

    if newer.is_empty() {
        return Prompt::new("Sync from the shared folder", body)
            .option(Choice::primary("sync_shared.everything", "Sync"), true)
            .dismiss("Cancel");
    }

    Prompt::new(
        "Sync from the shared folder",
        format!(
            "{body} The shared folder is also used by Minecraft {}. Worlds saved there can break \
             in this older version, and its settings may reset some keybinds.",
            listed(newer.iter().cloned())
        ),
    )
    .option(
        Choice::new("sync_shared.everything", "Sync everything"),
        true,
    )
    .option(
        Choice::primary("sync_shared.settings", "Settings only"),
        false,
    )
    .dismiss("Cancel")
}

fn report_summary(report: &SyncReport, include_worlds: bool) -> String {
    if report.total == 0 {
        return "Nothing to sync in the shared folder.".to_string();
    }
    if !include_worlds {
        return "Settings are now copied into this instance.".to_string();
    }
    if report.skipped.is_empty() {
        return "Settings and worlds are now copied into this instance.".to_string();
    }
    format!(
        "Settings copied. Skipped worlds this instance already has: {}.",
        listed(report.skipped.iter().cloned())
    )
}
