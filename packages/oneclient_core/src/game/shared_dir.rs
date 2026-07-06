
use std::collections::HashSet;
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

    import_manual_content(services, cluster, game_dir).await;

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

pub async fn import_manual_content(services: &LauncherServices, cluster: &Cluster, game_dir: &Path) {
    let linked = match PackageStore::list_linked_artifacts(cluster.id, services).await {
        Ok(linked) => linked,
        Err(err) => {
            tracing::warn!(error = %err, "failed to list links; skipping manual-content import");
            return;
        }
    };

    for content_type in SWAP_TYPES {
        let dir = game_dir.join(content_type.folder_name());
        let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
            continue;
        };

        let known: HashSet<&str> = linked
            .iter()
            .filter(|link| link.content_type == content_type)
            .map(|link| link.cluster_file_name.as_str())
            .collect();

        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }

            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.starts_with('.') || !has_content_extension(content_type, name) {
                continue;
            }
            if known.contains(name) {
                continue;
            }

            match PackageStore::import_local_file(&path, content_type, cluster.id, services).await {
                Ok(_) => {
                    tracing::info!(file = name, "registered manually-added shared content")
                }
                Err(err) => tracing::warn!(
                    file = name,
                    error = %err,
                    "failed to register manually-added shared content"
                ),
            }
        }
    }
}

fn has_content_extension(content_type: ContentType, name: &str) -> bool {
    let lower = name.to_lowercase();
    match content_type {
        ContentType::Mod => lower.ends_with(".jar"),
        ContentType::ResourcePack | ContentType::Shader => lower.ends_with(".zip"),
        _ => false,
    }
}

const EMPTY_NOTE_NAME: &str = "WHY_IS_THIS_EMPTY.txt";

fn empty_note_body(content_type: ContentType) -> String {
    let noun = match content_type {
        ContentType::ResourcePack => "resource packs",
        ContentType::Shader => "shader packs",
        _ => "mods",
    };
    format!(
        "It's empty here, but nothing is broken!\n\
        \n\
        OneClient keeps your {noun} safe somewhere else and only puts them here \
        while you play. When you close the game, it tidies them away again.\n\
        \n\
        Want to add {noun}? The easy way is right inside OneClient. Or you can drop \
        files in this folder, and OneClient will pick them up the next time you play.\n"
    )
}

pub async fn clear_shared_content(game_dir: &Path) -> LauncherResult<()> {
    for content_type in SWAP_TYPES {
        let dir = game_dir.join(content_type.folder_name());
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await.ok();
        }
        tokio::fs::create_dir_all(&dir).await.ok();
        tokio::fs::write(dir.join(EMPTY_NOTE_NAME), empty_note_body(content_type))
            .await
            .ok();
    }
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
