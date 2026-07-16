use std::str::FromStr;

use oneclient_db::dao::{bundle as bundle_dao, cluster as cluster_dao};

use crate::packages::domain::GameLoader;
use crate::state::LauncherState;
use crate::version::format_mc_version;
use crate::LauncherResult;

use super::cluster::Cluster;
use super::manager::ClusterManager;
use super::options::CreateClusterOptions;

#[tracing::instrument(skip(state))]
pub async fn ensure_from_bundles(state: &LauncherState) -> LauncherResult<Vec<Cluster>> {
    let groups = bundle_dao::list_distinct_version_loaders(&state.services.db).await?;
    let mut created = Vec::new();

    for group in groups {
        let Some(loader) = GameLoader::from_repr(group.mc_loader as u8) else {
            tracing::warn!(
                mc_version = %group.mc_version,
                mc_loader = group.mc_loader,
                "skipping bundle group with unknown loader"
            );
            continue;
        };

        if cluster_dao::find_by_version_loader(
            &state.services.db,
            &group.mc_version,
            group.mc_loader,
        )
        .await?
        .is_some()
        {
            continue;
        }

        let mc_version = group.mc_version.clone();
        let name = format!("{mc_version} {loader}");
        match ClusterManager::create(
            state,
            CreateClusterOptions::new(name, mc_version.clone(), loader),
        )
        .await
        {
            Ok(cluster) => {
                tracing::info!(
                    cluster_id = cluster.id,
                    mc_version = %cluster.mc_version,
                    loader = %cluster.mc_loader,
                    "created cluster from bundle catalog"
                );
                created.push(cluster);
            }
            Err(err) => {
                tracing::warn!(
                    mc_version = %mc_version,
                    loader = %loader,
                    error = %err,
                    "failed to create cluster for bundle group"
                );
            }
        }
    }

    Ok(created)
}

#[tracing::instrument(skip(state))]
pub async fn ensure_from_versions(state: &LauncherState) -> LauncherResult<Vec<Cluster>> {
    let metadata = state.versions.metadata().await;
    let mut created = Vec::new();

    for entry in metadata {
        let Some(minor) = entry.minor_version else {
            continue;
        };
        let Some(loader_str) = entry.loader.as_deref() else {
            continue;
        };
        let Ok(loader) = GameLoader::from_str(loader_str) else {
            tracing::warn!(
                major = entry.major_version,
                minor,
                loader = loader_str,
                "skipping versions entry with unknown loader"
            );
            continue;
        };

        let mc_version = format_mc_version(entry.major_version, minor, entry.patch_version);

        if cluster_dao::find_by_version_loader(&state.services.db, &mc_version, loader as i64)
            .await?
            .is_some()
        {
            continue;
        }

        let name = format!("{mc_version} {loader}");
        match ClusterManager::create(
            state,
            CreateClusterOptions::new(name, mc_version.clone(), loader),
        )
        .await
        {
            Ok(cluster) => {
                tracing::info!(
                    cluster_id = cluster.id,
                    mc_version = %cluster.mc_version,
                    loader = %cluster.mc_loader,
                    "created cluster from versions manifest"
                );
                created.push(cluster);
            }
            Err(err) => {
                tracing::warn!(
                    mc_version = %mc_version,
                    loader = %loader,
                    error = %err,
                    "failed to create cluster for versions entry"
                );
            }
        }
    }

    Ok(created)
}
