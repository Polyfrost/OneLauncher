use freya::prelude::*;
use freya::router::*;

use crate::Actions;
use crate::hooks::{use_dispatch, use_launcher, use_pending_launch, use_settings_snapshot};
use crate::launcher;
use crate::routes::Route;

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

    actions.launch_cluster(cluster.id);
}
