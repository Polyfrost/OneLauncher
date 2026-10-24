use std::sync::Arc;

use oneclient_common::domain::{GameLoader, PackageUpdateMode};
use oneclient_content::modpacks::is_newer_version;
use oneclient_db::dao::cluster_flag::{self as flag_dao, ClusterFlag};
use oneclient_events::Signal;

use crate::LauncherResult;
use crate::clusters::{Cluster, ClusterUpdate};
use crate::game::get_loader_versions;
use crate::state::LauncherState;

pub async fn loader_update_policy(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<PackageUpdateMode> {
    if cluster.uses_bundles() {
        return Ok(PackageUpdateMode::Automatic);
    }

    let mode = flag_dao::get(&state.services.db, cluster.id, ClusterFlag::LoaderUpdate).await?;
    Ok(mode
        .as_deref()
        .and_then(PackageUpdateMode::parse)
        .unwrap_or_default())
}

pub async fn set_loader_update_policy(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    mode: PackageUpdateMode,
) -> LauncherResult<()> {
    let value = Some(mode.as_str());
    Ok(flag_dao::set(
        &state.services.db,
        cluster_id,
        ClusterFlag::LoaderUpdate,
        value,
    )
    .await?)
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
    if loader_update_policy(state, cluster).await? != PackageUpdateMode::Automatic {
        return Ok(None);
    }
    let Some(version) = available_loader_update(state, cluster).await? else {
        return Ok(None);
    };
    apply_loader_update(state, cluster.id, version)
        .await
        .map(Some)
}
