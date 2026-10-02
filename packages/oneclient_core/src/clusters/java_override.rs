use std::sync::Arc;

use oneclient_cluster::ProfileUpdate;
use oneclient_cluster::profiles::get_profile_or_default;
use oneclient_common::patch::Patch;
use oneclient_db::dao::applied_migration as migration_dao;
use oneclient_events::GroupedProgressSession;

use crate::LauncherResult;
use crate::state::LauncherState;

const BUNDLE_JAVA_OVERRIDE: &str = "bundle-java-override";

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
    if !cluster.uses_bundles() {
        return Ok(());
    }
    let Some(profile_name) = cluster.setting_profile_name.as_deref() else {
        return Ok(());
    };

    let archives = state
        .bundles
        .archives_for(
            &state.services.content(),
            &cluster.mc_version,
            cluster.mc_loader,
        )
        .await?;
    if archives.is_empty() {
        return Ok(());
    }

    let mut overrides = archives.iter().filter_map(|archive| {
        archive
            .manifest
            .java_version_override
            .map(|major| (archive.manifest.name.as_str(), major))
    });

    let Some((bundle, major)) = overrides.next() else {
        return Ok(());
    };

    let marker = format!("{BUNDLE_JAVA_OVERRIDE}:{cluster_id}:{major}");
    if migration_dao::is_applied(db, &marker).await? {
        return Ok(());
    }

    for (other, other_major) in overrides {
        if other_major != major {
            tracing::warn!(
                cluster_id,
                bundle,
                major,
                other,
                other_major,
                "bundles disagree on the java version override; using the first"
            );
        }
    }

    let global = state.settings.read().global_game_settings.clone();
    let profile = get_profile_or_default(db, &global, Some(profile_name)).await?;

    if profile.java_path.is_none() {
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

    migration_dao::mark_applied(db, &marker).await?;
    Ok(())
}
