use std::sync::Arc;

use interfrost::api::minecraft::VersionInfo;
use oneclient_cluster::profiles::get_profile_or_default;
use oneclient_cluster::{Cluster, ProfileUpdate};
use oneclient_common::patch::Patch;
use oneclient_db::dao::cluster_flag::{self as flag_dao, ClusterFlag};
use oneclient_events::GroupedProgressSession;

use crate::LauncherResult;
use crate::state::LauncherState;

#[tracing::instrument(skip(state, progress))]
pub async fn apply_bundle_java_override(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    search_for_java: bool,
    auto_install_java: bool,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<()> {
    let db = &state.services.db;
    let cluster = state.clusters.get(cluster_id).await?;
    let Some(profile_name) = cluster.setting_profile_name.as_deref() else {
        return Ok(());
    };
    let Some((bundle, major)) = bundle_java_major(state, &cluster).await? else {
        return Ok(());
    };
    let bundle = bundle.as_str();

    let applied = major.to_string();
    if flag_dao::get(db, cluster_id, ClusterFlag::BundleJavaOverride)
        .await?
        .as_deref()
        == Some(&applied)
    {
        return Ok(());
    }

    let global = state.settings.read().global_game_settings.clone();
    let profile = get_profile_or_default(db, &global, Some(profile_name)).await?;

    if profile.java_path.is_none() {
        let runtime = state
            .java
            .prepare(major, false, search_for_java, auto_install_java, progress)
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
        db,
        cluster_id,
        ClusterFlag::BundleJavaOverride,
        Some(&applied),
    )
    .await?;
    Ok(())
}

pub(crate) async fn bundle_java_major(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<Option<(String, u32)>> {
    if !cluster.uses_bundles() {
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

    Ok(Some((bundle.to_owned(), major)))
}

pub(crate) async fn effective_java_major(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    info: &VersionInfo,
) -> Option<u32> {
    match bundle_java_major(state, cluster).await {
        Ok(Some((_, major))) => return Some(major),
        Ok(None) => {}
        Err(err) => {
            tracing::warn!(cluster_id = cluster.id, error = %err, "could not read the bundle java override");
        }
    }
    info.java_version.as_ref().map(|java| java.major_version)
}
