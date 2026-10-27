use oneclient_common::PackageUpdateMode;
use oneclient_core::clusters::{
    apply_loader_update, auto_update_loader, available_loader_update, loader_update_policy,
    set_loader_update_policy,
};
use oneclient_db::models::ClusterId;
use oneclient_events::{Choice, Prompt};

use crate::hooks::Actions;
use crate::launcher;
use crate::launcher::off_ui;

enum LoaderCheck {
    Nothing,
    Updated(String),
    Offer(String, String),
}

pub async fn offer_before_launch(
    actions: &Actions,
    cluster_id: ClusterId,
    manual: bool,
) -> Option<bool> {
    let state = launcher::state().ok()?;

    let check = off_ui({
        let state = state.clone();
        async move {
            let cluster = state.clusters.get(cluster_id).await.ok()?;
            match loader_update_policy(&state, &cluster).await.ok()? {
                PackageUpdateMode::Prompt => {}
                PackageUpdateMode::Automatic if manual => {
                    return match auto_update_loader(&state, &cluster).await {
                        Ok(Some(updated)) => Some(LoaderCheck::Updated(format!(
                            "{} Loader {}",
                            updated.mc_loader,
                            updated.mc_loader_version.unwrap_or_default()
                        ))),
                        Ok(None) => Some(LoaderCheck::Nothing),
                        Err(err) => {
                            tracing::warn!(cluster_id, error = %err, "could not update the loader");
                            None
                        }
                    };
                }
                PackageUpdateMode::Automatic | PackageUpdateMode::Skip => {
                    return Some(LoaderCheck::Nothing);
                }
            }
            let Some(current) = cluster.mc_loader_version.clone() else {
                return Some(LoaderCheck::Nothing);
            };
            match available_loader_update(&state, &cluster).await {
                Ok(Some(latest)) => Some(LoaderCheck::Offer(current, latest)),
                Ok(None) => Some(LoaderCheck::Nothing),
                Err(err) => {
                    tracing::warn!(cluster_id, error = %err, "could not check for a loader update");
                    None
                }
            }
        }
    })
    .await?;
    let (current, latest) = match check {
        LoaderCheck::Nothing => return Some(false),
        LoaderCheck::Updated(loader) => {
            actions
                .notify("Loader updated")
                .body(format!("This instance now uses {loader}."))
                .send();
            return Some(true);
        }
        LoaderCheck::Offer(current, latest) => (current, latest),
    };

    let answer = match actions.events().ask(offer_prompt(&current, &latest)).await {
        Ok(Some(chosen)) => chosen.value,
        Ok(None) => return Some(true),
        Err(err) => {
            tracing::warn!(cluster_id, error = %err, "loader update offer failed");
            return None;
        }
    };

    let result = off_ui(async move {
        match answer {
            None => apply_loader_update(&state, cluster_id, latest)
                .await
                .map(drop),
            Some(mode) => set_loader_update_policy(&state, cluster_id, mode).await,
        }
    })
    .await;
    if let Err(err) = result {
        tracing::warn!(cluster_id, error = %err, "could not apply the loader update choice");
        return None;
    }
    Some(true)
}

fn offer_prompt(current: &str, latest: &str) -> Prompt<Option<PackageUpdateMode>> {
    Prompt::new(
        "Fabric Loader update",
        format!(
            "Fabric Loader {latest} is available for this instance, which is on {current}. \
             Would you like to update it?"
        ),
    )
    .option(
        Choice::new("loader.update.never", "Never ask again"),
        Some(PackageUpdateMode::Skip),
    )
    .option(
        Choice::new("loader.update.always", "Always update"),
        Some(PackageUpdateMode::Automatic),
    )
    .option(Choice::primary("loader.update.once", "Update"), None)
    .dismiss("Not now")
}
