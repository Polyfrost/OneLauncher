use std::sync::Arc;

use oneclient_common::domain::GameLoader;
use oneclient_content::modpacks::is_newer_version;
use oneclient_db::dao::applied_migration as migration_dao;
use oneclient_events::Signal;

use crate::LauncherResult;
use crate::clusters::{Cluster, ClusterUpdate};
use crate::game::get_loader_versions;
use crate::state::LauncherState;

const LOADER_UPDATE_ALWAYS: &str = "loader-update-always";
const LOADER_UPDATE_NEVER: &str = "loader-update-never";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderUpdatePolicy {
    Ask,
    Always,
    Never,
}

pub async fn loader_update_policy(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<LoaderUpdatePolicy> {
    if cluster.uses_bundles() {
        return Ok(LoaderUpdatePolicy::Always);
    }

    let db = &state.services.db;
    if migration_dao::is_applied(db, &format!("{LOADER_UPDATE_ALWAYS}:{}", cluster.id)).await? {
        return Ok(LoaderUpdatePolicy::Always);
    }
    if migration_dao::is_applied(db, &format!("{LOADER_UPDATE_NEVER}:{}", cluster.id)).await? {
        return Ok(LoaderUpdatePolicy::Never);
    }
    Ok(LoaderUpdatePolicy::Ask)
}

pub async fn set_loader_update_policy(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    policy: LoaderUpdatePolicy,
) -> LauncherResult<()> {
    let db = &state.services.db;
    let always = format!("{LOADER_UPDATE_ALWAYS}:{cluster_id}");
    let never = format!("{LOADER_UPDATE_NEVER}:{cluster_id}");

    migration_dao::forget(db, &always).await?;
    migration_dao::forget(db, &never).await?;
    match policy {
        LoaderUpdatePolicy::Ask => {}
        LoaderUpdatePolicy::Always => migration_dao::mark_applied(db, &always).await?,
        LoaderUpdatePolicy::Never => migration_dao::mark_applied(db, &never).await?,
    }
    Ok(())
}

pub async fn available_loader_update(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<Option<String>> {
    if !matches!(cluster.mc_loader, GameLoader::Fabric | GameLoader::Ornithe) {
        return Ok(None);
    }

    let mc_version = oneclient_common::version::normalize_mc_version_input(&cluster.mc_version);
    let newest = {
        let mut metadata = state.metadata.lock().await;
        get_loader_versions(
            &mut metadata,
            &state.services.mc(),
            &mc_version,
            cluster.mc_loader,
        )
        .await?
        .into_iter()
        .next()
    };

    Ok(newest.filter(|newest| match &cluster.mc_loader_version {
        Some(pinned) => is_newer_version(newest, pinned),
        None => cluster.uses_bundles(),
    }))
}

pub async fn apply_loader_update(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    version: String,
) -> LauncherResult<Cluster> {
    tracing::info!(cluster_id, %version, "updating the cluster's loader");
    let cluster = state
        .clusters
        .update(cluster_id, ClusterUpdate::default().loader_version(version))
        .await?;
    state.services.events.signal(Signal::ClustersChanged);
    Ok(cluster)
}

pub async fn auto_update_loader(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<Option<Cluster>> {
    if loader_update_policy(state, cluster).await? != LoaderUpdatePolicy::Always {
        return Ok(None);
    }
    let Some(version) = available_loader_update(state, cluster).await? else {
        return Ok(None);
    };
    apply_loader_update(state, cluster.id, version)
        .await
        .map(Some)
}
