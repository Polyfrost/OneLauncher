use oneclient_db::dao::{artifact as artifact_dao, cluster_bundle as cluster_bundle_dao};
use oneclient_db::models::OverrideType;

use oneclient_cluster::{Cluster, CreateClusterOptions};

use crate::LauncherResult;
use crate::state::LauncherState;

#[tracing::instrument(skip(state))]
pub async fn duplicate_cluster(
    state: &LauncherState,
    source_id: i64,
    name: &str,
    dedicated: bool,
) -> LauncherResult<Cluster> {
    let source = state.clusters.get(source_id).await?;
    let global = state.settings.read().global_game_settings.clone();

    let mut options = CreateClusterOptions::new(name, source.mc_version.clone(), source.mc_loader)
        .dedicated(dedicated);
    options.mc_loader_version = source.mc_loader_version.clone();

    let clone = state.clusters.create(&global, options).await?;

    if let Err(err) = copy_content(state, source_id, clone.id).await {
        tracing::warn!(
            source_id,
            cluster_id = clone.id,
            error = %err,
            "duplicated cluster is missing some content; removing the incomplete copy"
        );
        let _ = state.clusters.delete(clone.id, true).await;
        return Err(err);
    }

    tracing::info!(
        source_id,
        cluster_id = clone.id,
        name = %clone.name,
        "duplicated cluster"
    );

    Ok(clone)
}

async fn copy_content(
    state: &LauncherState,
    source_id: i64,
    target_id: i64,
) -> LauncherResult<()> {
    let db = &state.services.db;

    for link in artifact_dao::list_cluster_artifacts(db, source_id).await? {
        artifact_dao::link_cluster_artifact(db, target_id, &link.hash, &link.cluster_file_name)
            .await?;

        if link.enabled == 0 {
            artifact_dao::update_cluster_artifact(
                db,
                target_id,
                &link.hash,
                &link.cluster_file_name,
                0,
            )
            .await?;
        }
    }

    for tracked in cluster_bundle_dao::list_bundle_tracked(db, source_id).await? {
        let (Some(bundle_name), Some(bundle_version_id), Some(package_id)) = (
            tracked.bundle_name.as_deref(),
            tracked.bundle_version_id.as_deref(),
            tracked.package_id.as_deref(),
        ) else {
            continue;
        };

        cluster_bundle_dao::track_bundle_artifact(
            db,
            target_id,
            &tracked.hash,
            bundle_name,
            bundle_version_id,
            package_id,
        )
        .await?;
    }

    for override_row in cluster_bundle_dao::list_overrides(db, source_id).await? {
        let Some(override_type) = OverrideType::parse(&override_row.override_type) else {
            continue;
        };

        cluster_bundle_dao::save_override(
            db,
            target_id,
            &override_row.bundle_name,
            &override_row.package_id,
            override_type,
        )
        .await?;
    }

    Ok(())
}
