use oneclient_core::clusters::{
    LoaderUpdatePolicy, apply_loader_update, available_loader_update, loader_update_policy,
    set_loader_update_policy,
};
use oneclient_db::models::ClusterId;
use oneclient_events::{Choice, Prompt};

use crate::hooks::Actions;
use crate::launcher;
use crate::launcher::off_ui;

enum LoaderUpdateAnswer {
    Once,
    Always,
    Never,
}

pub async fn offer_before_launch(actions: &Actions, cluster_id: ClusterId) {
    let Ok(state) = launcher::state() else {
        return;
    };

    let offer = off_ui({
        let state = state.clone();
        async move {
            let cluster = state.clusters.get(cluster_id).await.ok()?;
            match loader_update_policy(&state, &cluster).await {
                Ok(LoaderUpdatePolicy::Ask) => {}
                Ok(_) => return None,
                Err(err) => {
                    tracing::warn!(
                        cluster_id,
                        "could not read the loader update policy: {err:#}"
                    );
                    return None;
                }
            }
            let current = cluster.mc_loader_version.clone()?;
            match available_loader_update(&state, &cluster).await {
                Ok(latest) => latest.map(|latest| (current, latest)),
                Err(err) => {
                    tracing::warn!(cluster_id, "could not check for a loader update: {err:#}");
                    None
                }
            }
        }
    })
    .await;
    let Some((current, latest)) = offer else {
        return;
    };

    let answer = match actions.events().ask(offer_prompt(&current, &latest)).await {
        Ok(Some(chosen)) => chosen.value,
        Ok(None) => return,
        Err(err) => {
            tracing::warn!(cluster_id, "loader update offer failed: {err:#}");
            return;
        }
    };

    let result = off_ui(async move {
        match answer {
            LoaderUpdateAnswer::Once => apply_loader_update(&state, cluster_id, latest)
                .await
                .map(drop),
            LoaderUpdateAnswer::Always => {
                set_loader_update_policy(&state, cluster_id, LoaderUpdatePolicy::Always).await
            }
            LoaderUpdateAnswer::Never => {
                set_loader_update_policy(&state, cluster_id, LoaderUpdatePolicy::Never).await
            }
        }
    })
    .await;
    if let Err(err) = result {
        tracing::warn!(
            cluster_id,
            "could not apply the loader update choice: {err:#}"
        );
    }
}

fn offer_prompt(current: &str, latest: &str) -> Prompt<LoaderUpdateAnswer> {
    Prompt::new(
        "Fabric Loader update",
        format!(
            "Fabric Loader {latest} is available for this instance, which is on {current}. \
             Would you like to update it before launching?"
        ),
    )
    .option(
        Choice::new("loader.update.never", "Never ask again"),
        LoaderUpdateAnswer::Never,
    )
    .option(
        Choice::new("loader.update.always", "Always update"),
        LoaderUpdateAnswer::Always,
    )
    .option(
        Choice::primary("loader.update.once", "Update"),
        LoaderUpdateAnswer::Once,
    )
    .dismiss("Not now")
}
