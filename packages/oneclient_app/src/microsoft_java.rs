use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use freya::prelude::spawn_forever;
use oneclient_common::Patch;
use oneclient_core::{LauncherState, ProfileUpdate};
use oneclient_db::models::ClusterId;
use oneclient_events::{Choice, Prompt, Signal};
use oneclient_java::{JavaRuntime, JavaVendor};

use crate::hooks::Actions;
use crate::launcher;

/// Front-ends match on these
pub const MICROSOFT_JAVA_CHOICE_INSTALL: &str = "java.microsoft.install";
pub const MICROSOFT_JAVA_CHOICE_NEVER: &str = "java.microsoft.never";

/// Cancel works per session, "Don't ask again" is set in settings
static ASKED: Mutex<BTreeSet<ClusterId>> = Mutex::new(BTreeSet::new());

enum MicrosoftJavaAnswer {
    Install,
    Never,
}

pub fn spawn_auto_install() {
    spawn_forever(auto_install());
}

async fn auto_install() {
    let Ok(state) = launcher::state() else {
        return;
    };

    if opted_out(&state) {
        return;
    }

    let Some(cluster_id) = automatic_cluster(&state).await else {
        return;
    };

    let major = match oneclient_core::required_java_major(&state, cluster_id).await {
        Ok(Some(major)) => major,
        Ok(None) => {
            tracing::info!(cluster_id, "the cluster's manifest names no Java version");
            return;
        }
        Err(err) => {
            tracing::warn!(cluster_id, "could not read the cluster's Java version: {err:#}");
            return;
        }
    };

    match state
        .java
        .has_vendor_runtime(&JavaVendor::Microsoft, Some(major))
        .await
    {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            tracing::warn!("could not check for a Microsoft Java runtime: {err:#}");
            return;
        }
    }

    if !publishes(&state, major).await {
        return;
    }

    tracing::info!(
        cluster_id,
        major,
        "fetching a Microsoft runtime for the cluster on Automatic"
    );
    install_and_unpin(cluster_id, major);
}

/// Asked before the game starts the install that may follow lands on the next
/// launch not this one
pub async fn offer_for_pinned_cluster(actions: &Actions, cluster_id: ClusterId) {
    let Ok(state) = launcher::state() else {
        return;
    };

    if opted_out(&state) {
        return;
    }

    let Some(pinned) = pinned_runtime(&state, cluster_id).await else {
        return;
    };

    if pinned.vendor == JavaVendor::Microsoft {
        return;
    }

    if !publishes(&state, pinned.major).await || !claim_ask(cluster_id) {
        return;
    }

    match actions.events().ask(offer_prompt()).await {
        Ok(Some(chosen)) => match chosen.value {
            MicrosoftJavaAnswer::Install => install_and_unpin(cluster_id, pinned.major),
            MicrosoftJavaAnswer::Never => actions.skip_microsoft_java(),
        },
        Ok(None) => tracing::info!(cluster_id, "Microsoft Java offer dismissed"),
        Err(err) => tracing::warn!(cluster_id, "Microsoft Java offer failed: {err:#}"),
    }
}

fn offer_prompt() -> Prompt<MicrosoftJavaAnswer> {
    Prompt::new(
        "Microsoft Java runtime",
        "Based on our research, we now recommend Microsoft's OpenJDK \
         as they have specific optimizations for Minecraft. \
         Would you like to use Microsoft OpenJDK as your default Java installation? \
         Minecraft will launch with Microsoft OpenJDK next time.",
    )
    .option(
        Choice::new(MICROSOFT_JAVA_CHOICE_NEVER, "Don't ask again"),
        MicrosoftJavaAnswer::Never,
    )
    .option(
        Choice::primary(MICROSOFT_JAVA_CHOICE_INSTALL, "Proceed"),
        MicrosoftJavaAnswer::Install,
    )
    .dismiss("Cancel")
}

fn install_and_unpin(cluster_id: ClusterId, major: u32) {
    spawn_forever(async move {
        let Ok(state) = launcher::state() else { return };
        let events = state.services.events.clone();

        let runtime = match state
            .java
            .install_runtime_from(&JavaVendor::Microsoft, major)
            .await
        {
            Ok(runtime) => runtime,
            Err(err) => {
                events
                    .notify("Java install failed")
                    .body(err.to_string())
                    .error()
                    .send();
                return;
            }
        };

        events.signal(Signal::JavaChanged);
        tracing::info!(cluster_id, version = %runtime.version, "installed a Microsoft runtime");

        // Cleared rather than pointed at the new runtime Automatic ranks the
        // default vendor first so it lands on this one anyway and the cluster
        // keeps following later Microsoft installs instead of freezing on one
        let update = ProfileUpdate {
            java_path: Patch::Clear,
            ..Default::default()
        };

        match state.clusters.update_profile(cluster_id, update).await {
            Ok(_) => {
                crate::hooks::invalidate_profile_queries().await;
                events
                    .notify("Java switched")
                    .body(format!(
                        "This cluster is back on Automatic and picks Microsoft {major} from its \
                         next launch."
                    ))
                    .send();
            }
            // Only when installation was successfull and the cluster is not pointing to the new version
            Err(err) => events
                .notify("Cluster not switched")
                .body(format!(
                    "Microsoft {major} was installed but the cluster still points at its old \
                     runtime: {err}"
                ))
                .error()
                .send(),
        }
    });
}

fn opted_out(state: &Arc<LauncherState>) -> bool {
    let settings = state.settings.read();
    settings.skip_microsoft_java || !settings.seen_onboarding
}

fn claim_ask(cluster_id: ClusterId) -> bool {
    ASKED
        .lock()
        .map(|mut asked| asked.insert(cluster_id))
        .unwrap_or(false)
}

/// If there is not Microsoft build for given host, the offer is abandoned
async fn publishes(state: &Arc<LauncherState>, major: u32) -> bool {
    match state
        .java
        .latest_package(&JavaVendor::Microsoft, major)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => {
            tracing::info!(major, "Microsoft publishes no build for this host");
            false
        }
        Err(err) => {
            tracing::warn!("could not reach Microsoft's downloads: {err:#}");
            false
        }
    }
}

/// Only the cluster the launcher opens on the one the home panel preselects last played first and newest version when nothing has been played yet
async fn automatic_cluster(state: &Arc<LauncherState>) -> Option<ClusterId> {
    let clusters = match state.clusters.list().await {
        Ok(clusters) => clusters,
        Err(err) => {
            tracing::warn!("could not read the cluster list: {err:#}");
            return None;
        }
    };

    let cluster = crate::utils::sort_clusters_for_home(clusters)
        .into_iter()
        .next()?;

    let global = state.settings.read().global_game_settings.clone();

    match state.clusters.resolve_settings(&global, &cluster).await {
        Ok(profile) => {
            let automatic = profile.java_path.is_none();
            tracing::info!(
                cluster_id = cluster.id,
                name = %cluster.name,
                automatic,
                "checked the Java of the cluster the launcher opens on"
            );
            automatic.then_some(cluster.id)
        }
        Err(err) => {
            tracing::warn!(
                cluster_id = cluster.id,
                "could not resolve cluster settings: {err:#}"
            );
            None
        }
    }
}

/// `None` when the cluster is on Automatic or when its pin points at a runtime that is no longer on disk
async fn pinned_runtime(state: &Arc<LauncherState>, cluster_id: ClusterId) -> Option<JavaRuntime> {
    let cluster = state.clusters.get(cluster_id).await.ok()?;
    let global = state.settings.read().global_game_settings.clone();
    let profile = state
        .clusters
        .resolve_settings(&global, &cluster)
        .await
        .ok()?;

    let path = profile.java_path?;
    state.java.runtime_for_profile(Some(&path)).await.ok()?
}
