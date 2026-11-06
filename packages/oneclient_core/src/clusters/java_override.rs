use std::sync::Arc;

use oneclient_cluster::ProfileUpdate;
use oneclient_cluster::profiles::get_profile_or_default;
use oneclient_common::patch::Patch;
use oneclient_db::dao::cluster_flag::{self as flag_dao, ClusterFlag};
use oneclient_events::GroupedProgressSession;

use super::Cluster;
use crate::LauncherResult;
use crate::state::LauncherState;

pub(super) async fn java_major_to_apply(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<Option<u32>> {
    let Some((_, major)) = pending_override(state, cluster).await? else {
        return Ok(None);
    };
    let path = own_java_path(state, cluster).await?;
    Ok(path.is_none().then_some(major))
}

async fn own_java_path(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<Option<String>> {
    let global = state.settings.read().global_game_settings.clone();
    let name = cluster.setting_profile_name.as_deref();
    let profile = get_profile_or_default(&state.services.db, &global, name).await?;
    Ok(profile.java_path)
}

async fn pending_override(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<Option<(String, u32)>> {
    if !cluster.uses_bundles() || cluster.setting_profile_name.is_none() {
        return Ok(None);
    }

    let archives = state
        .bundles
        .archives_for(
            &state.services.content(),
            &cluster.mc_version,
            cluster.mc_loader,
        )
        .await?;
    if archives.is_empty() {
        return Ok(None);
    }

    let taken = oneclient_content::bundles::taken_bundle_names(
        cluster.id,
        &archives,
        &state.services.content(),
    )
    .await?;
    let mut overrides = archives
        .iter()
        .filter(|archive| taken.contains(&archive.manifest.name))
        .filter_map(|archive| {
            archive
                .manifest
                .java_version_override
                .map(|major| (archive.manifest.name.as_str(), major))
        });

    let Some((bundle, major)) = overrides.next() else {
        return Ok(None);
    };

    let applied = flag_dao::get(
        &state.services.db,
        cluster.id,
        ClusterFlag::BundleJavaOverride,
    )
    .await?;
    if applied.as_deref() == Some(major.to_string().as_str()) {
        return Ok(None);
    }

    for (other, other_major) in overrides {
        if other_major != major {
            tracing::warn!(
                cluster_id = cluster.id,
                bundle,
                major,
                other,
                other_major,
                "bundles disagree on the java version override; using the first"
            );
        }
    }

    Ok(Some((bundle.to_string(), major)))
}

#[tracing::instrument(skip(state, progress))]
pub async fn apply_bundle_java_override(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    search_for_java: bool,
    auto_install_java: bool,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<()> {
    let cluster = state.clusters.get(cluster_id).await?;
    let Some((bundle, major)) = pending_override(state, &cluster).await? else {
        return Ok(());
    };

    if own_java_path(state, &cluster).await?.is_none() {
        let runtime = state
            .java
            .prepare(major, search_for_java, auto_install_java, progress)
            .await?;

        state
            .clusters
            .update_profile(
                cluster_id,
                ProfileUpdate {
                    java_path: Patch::Set(runtime.absolute_path.clone()),
                    ..Default::default()
                },
            )
            .await?;

        tracing::info!(
            cluster_id,
            bundle,
            major,
            java_path = %runtime.absolute_path,
            "applied bundle java version override"
        );
    } else {
        tracing::debug!(
            cluster_id,
            bundle,
            major,
            "cluster already has a java runtime; leaving the bundle override unapplied"
        );
    }

    flag_dao::set(
        &state.services.db,
        cluster_id,
        ClusterFlag::BundleJavaOverride,
        Some(&major.to_string()),
    )
    .await?;
    Ok(())
}
