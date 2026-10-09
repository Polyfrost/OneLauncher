use freya::prelude::*;
use freya::router::*;
use oneclient_cluster::Cluster;
use oneclient_core::LauncherState;

use crate::Actions;
use crate::components::IconType;
use crate::hooks::{use_dispatch, use_launcher, use_pending_launch, use_settings_snapshot};
use crate::launcher;
use crate::routes::Route;
use crate::state::LaunchBlock;

#[derive(PartialEq)]
pub struct PendingLaunchDriver;

impl Component for PendingLaunchDriver {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let waiting = use_pending_launch().is_some();
        let ready = use_launcher().ready;
        let onboarded = use_settings_snapshot().settings.seen_onboarding;

        let router = RouterContext::get();

        let armed = waiting && ready && onboarded;
        use_side_effect_with_deps(&armed, move |&armed| {
            if !armed {
                return;
            }

            let Some(folder) = dispatch.take_pending_launch() else {
                return;
            };

            let actions = dispatch.clone();
            spawn(async move { launch_shortcut(actions, router, folder).await });
        });

        rect().into_element()
    }
}

async fn launch_shortcut(actions: Actions, router: RouterContext, folder: String) {
    let Ok(state) = launcher::state() else { return };

    let found = match state.clusters.find_by_folder_name(&folder).await {
        Ok(found) => found,
        Err(err) => {
            tracing::error!(folder, "shortcut lookup failed: {err:#}");
            actions
                .notify("Couldn't open that version")
                .body(format!("{err:#}"))
                .error()
                .send();
            return;
        }
    };

    let Some(cluster) = found else {
        tracing::warn!(folder, "shortcut names a cluster that no longer exists");
        actions.report_missing_shortcut_target(&folder);
        return;
    };

    let _ = router.replace(Route::ClusterOverview {
        cluster_id: cluster.id,
    });

    let shared = shared_dir_neighbours(&state, &cluster).await;
    let Some(block) = actions.launch_block(cluster.id, &shared) else {
        actions.launch_cluster(cluster.id);
        return;
    };

    let blocker = blocker_name(&cluster, block).await;
    report_already_open(&actions, &cluster, block, blocker.as_deref());
}

/// The other clusters that launch into the shared game dir empty when this
/// cluster has a dedicated one
async fn shared_dir_neighbours(state: &LauncherState, cluster: &Cluster) -> Vec<i64> {
    if cluster.uses_dedicated_dir() {
        return Vec::new();
    }

    match state.clusters.list().await {
        Ok(clusters) => clusters
            .into_iter()
            .filter(|other| other.id != cluster.id && !other.uses_dedicated_dir())
            .map(|other| other.id)
            .collect(),
        Err(err) => {
            // Core refuses a busy shared dir anyway this only words the notice
            tracing::warn!(cluster_id = cluster.id, "cluster list failed: {err:#}");
            Vec::new()
        }
    }
}

async fn blocker_name(cluster: &Cluster, block: LaunchBlock) -> Option<String> {
    if block.cluster_id() == cluster.id {
        return None;
    }

    let Ok(state) = launcher::state() else {
        return Some("Another version".to_string());
    };

    Some(
        state
            .clusters
            .get(block.cluster_id())
            .await
            .map_or_else(|_| "Another version".to_string(), |other| other.name),
    )
}

fn report_already_open(
    actions: &Actions,
    cluster: &Cluster,
    block: LaunchBlock,
    blocker: Option<&str>,
) {
    let running = matches!(block, LaunchBlock::Running(_));

    tracing::info!(
        cluster_id = cluster.id,
        blocking = block.cluster_id(),
        running,
        "refusing a shortcut launch, a game is already open"
    );

    let title = if running {
        "Minecraft is already running"
    } else {
        "Minecraft is already starting"
    };

    let body = match (blocker, running) {
        (None, true) => format!("{} is open already.", cluster.name),
        (None, false) => format!("{} is on its way up.", cluster.name),
        (Some(other), true) => {
            format!(
                "{other} is open. Close it before launching {}.",
                cluster.name
            )
        }
        (Some(other), false) => format!(
            "{other} is on its way up. Wait for it before launching {}.",
            cluster.name
        ),
    };

    actions
        .notify(title)
        .body(body)
        .icon(IconType::Rocket02)
        .send();
}
