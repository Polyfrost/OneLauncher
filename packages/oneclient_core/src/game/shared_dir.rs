
use std::path::Path;

use oneclient_db::dao::artifact as artifact_dao;

use crate::clusters::Cluster;
use crate::packages::domain::ContentType;
use crate::packages::store::{artifact_absolute_path, link_or_copy};
use crate::packages::PackageStore;
use crate::state::LauncherServices;
use crate::LauncherResult;

const SWAP_TYPES: [ContentType; 3] =
    [ContentType::Mod, ContentType::ResourcePack, ContentType::Shader];

const FABRIC_DEP_OVERRIDES: &str = "config/fabric_loader_dependencies.json";

pub async fn sync_shared_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
) -> LauncherResult<()> {
    tokio::fs::create_dir_all(game_dir).await.ok();

    for content_type in SWAP_TYPES {
        let dir = game_dir.join(content_type.folder_name());
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await.ok();
        }
        tokio::fs::create_dir_all(&dir).await.ok();
    }

    let linked = PackageStore::list_linked_artifacts(cluster.id, services).await?;
    for link in linked {
        if !link.enabled || !SWAP_TYPES.contains(&link.content_type) {
            continue;
        }

        let Some(artifact) =
            artifact_dao::get_artifact_by_hash(&services.db, &link.hash).await?
        else {
            continue;
        };

        let src = artifact_absolute_path(&artifact.path)?;
        if !src.exists() {
            tracing::warn!(hash = %link.hash, "cached artifact missing; skipping shared link");
            continue;
        }

        let dest = game_dir
            .join(link.content_type.folder_name())
            .join(&link.cluster_file_name);

        if let Err(err) = link_or_copy(&src, &dest).await {
            tracing::warn!(
                file = %link.cluster_file_name,
                error = %err,
                "failed to link content into shared directory"
            );
        }
    }

    sync_fabric_dep_overrides(cluster, game_dir).await?;

    Ok(())
}

async fn sync_fabric_dep_overrides(cluster: &Cluster, game_dir: &Path) -> LauncherResult<()> {
    let src = cluster.dir()?.join(FABRIC_DEP_OVERRIDES);
    let dest = game_dir.join(FABRIC_DEP_OVERRIDES);

    if src.exists() {
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::copy(&src, &dest).await?;
    } else if dest.exists() {
        tokio::fs::remove_file(&dest).await.ok();
    }

    Ok(())
}
