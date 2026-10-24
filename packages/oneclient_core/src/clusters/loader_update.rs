use std::sync::Arc;

use oneclient_common::domain::GameLoader;
use oneclient_content::modpacks::is_newer_version;
use oneclient_db::dao::cluster_flag as flag_dao;
use oneclient_events::Signal;

use crate::LauncherResult;
use crate::clusters::{Cluster, ClusterUpdate};
use crate::game::get_loader_versions;
use crate::state::LauncherState;

const LOADER_UPDATE: &str = "loader-update";

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

    let policy = flag_dao::get(&state.services.db, cluster.id, LOADER_UPDATE).await?;
    Ok(match policy.as_deref() {
        Some("always") => LoaderUpdatePolicy::Always,
        Some("never") => LoaderUpdatePolicy::Never,
        _ => LoaderUpdatePolicy::Ask,
    })
}

pub async fn set_loader_update_policy(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    policy: LoaderUpdatePolicy,
) -> LauncherResult<()> {
    let db = &state.services.db;
    match policy {
        LoaderUpdatePolicy::Ask => flag_dao::clear(db, cluster_id, LOADER_UPDATE).await?,
        LoaderUpdatePolicy::Always => {
            flag_dao::set(db, cluster_id, LOADER_UPDATE, Some("always")).await?;
        }
        LoaderUpdatePolicy::Never => {
            flag_dao::set(db, cluster_id, LOADER_UPDATE, Some("never")).await?;
        }
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
