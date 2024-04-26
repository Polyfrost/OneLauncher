//! OneLauncher cluster management

use crate::proxy::send::send_cluster;
use crate::proxy::ClusterPayloadType;
use crate::prelude::ClusterPath;
use crate::store::{State, PackagePath};
use crate::utils::io;
pub use crate::store::{JavaOptions, Cluster};

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;

pub mod create;
pub mod update;

/// get a cluster by its specified [`ClusterPath`].
#[tracing::instrument]
pub async fn get(path: &ClusterPath, clear: Option<bool>) -> crate::Result<Option<Cluster>> {
    let state = State::get().await?;
    let clusters = state.clusters.read().await;
    let mut cluster = clusters.0.get(path).cloned();

    if clear.unwrap_or(false) {
        if let Some(cluster) = &mut cluster {
            cluster.packages = HashMap::new();
        }
    }

    Ok(cluster)
}

/// remove a specified cluster from it's [`ClusterPath`].
#[tracing::instrument]
pub async fn remove(path: &ClusterPath) -> crate::Result<()> {
    let state = State::get().await?;
    let mut clusters = state.clusters.write().await;

    if let Some(cluster) = clusters.remove(path).await? {
        send_cluster(cluster.uuid, path, &cluster.meta.name, ClusterPayloadType::Deleted).await?;
    }

    Ok(())
}

/// get a cluster by it's [`uuid::Uuid`]
#[tracing::instrument]
pub async fn get_by_uuid(
    uuid: uuid::Uuid,
    clear: Option<bool>,
) -> crate::Result<Option<Cluster>> {
    let state = State::get().await?;
    let clusters = state.clusters.read().await;
    let mut cluster = clusters.0.values().find(|c| c.uuid == uuid).cloned();

    if clear.unwrap_or(false) {
        if let Some(cluster) = &mut cluster {
            cluster.packages = HashMap::new();
        }
    }

    Ok(cluster)
}

/// get a cluster's full path by it's [`ClusterPath`].
#[tracing::instrument]
pub async fn get_full_path(path: &ClusterPath) -> crate::Result<PathBuf> {
    let _ = get(path, Some(true)).await?.ok_or_else(|| anyhow::anyhow!("failed to get the full path of profile at path {}", path))?;
    let full_path = io::canonicalize(path.full_path().await?)?;

    Ok(full_path)
}

/// get a specific mod's full path in the filesystem by it's [`ClusterPath`] and [`PackagePath`].
#[tracing::instrument]
pub async fn get_mod_path(
    cluster_path: &ClusterPath,
    package_path: &PackagePath,
) -> crate::Result<PathBuf> {
    if get(cluster_path, Some(true)).await?.is_some() {
        let full_path = io::canonicalize(package_path.full_path(cluster_path.clone()).await?)?;

        return Ok(full_path);
    }

    Err(anyhow::anyhow!("failed to get the full path of a cluster at path {}", package_path.full_path(cluster_path.clone()).await?.display()).into())
}

/// edit a cluster with an async closure and it's [`ClusterPath`]
pub async fn edit<FutFn>(
    path: &ClusterPath,
    action: impl Fn(&mut Cluster) -> FutFn,
) -> crate::Result<()>
where
    FutFn: Future<Output = crate::Result<()>>,
{
    let state = State::get().await?;
    let mut clusters = state.clusters.write().await;

    match clusters.0.get_mut(path) {
        Some(ref mut cluster) => {
            action(cluster).await?;

            send_cluster(cluster.uuid, path, &cluster.meta.name, ClusterPayloadType::Edited).await?;

            Ok(())
        }
        None => Err(anyhow::anyhow!("unmanaged profile edited at {}", path.to_string()).into())
    }
}

/// Try to update a Cluster's playtime.
#[tracing::instrument]
#[onelauncher_debug::debugger]
pub async fn update_playtime(path: &ClusterPath) -> crate::Result<()> {
    let state = State::get().await?;
    let cluster = get(path, None).await?.ok_or_else(|| anyhow::anyhow!("failed to update playtime at path {}", path))?;
    let recent_playtime = cluster.meta.recently_played;
    
    /*
     * todo
     */

    let mut clusters = state.clusters.write().await;
    if let Some(cluster) = clusters.0.get_mut(path) {
        cluster.meta.overall_played += recent_playtime;
        cluster.meta.recently_played = 0;
    }

    State::sync().await?;

    Ok(())
}
