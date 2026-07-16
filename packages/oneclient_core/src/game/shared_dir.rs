use std::collections::HashSet;
use std::path::Path;

use oneclient_db::dao::artifact as artifact_dao;

use crate::LauncherResult;
use crate::clusters::Cluster;
use crate::packages::PackageStore;
use crate::packages::domain::ContentType;
use crate::packages::store::{artifact_absolute_path, link_or_copy};
use crate::state::LauncherServices;

const REDIRECTED_DIRS: [&str; 2] = ["logs", "crash-reports"];

const SWAP_TYPES: [ContentType; 3] = [
    ContentType::Mod,
    ContentType::ResourcePack,
    ContentType::Shader,
];

const FABRIC_DEP_OVERRIDES: &str = "config/fabric_loader_dependencies.json";

#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id, game_dir = %game_dir.display()), level = "debug")]
pub async fn sync_shared_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
) -> LauncherResult<()> {
    polyio::create_dir_all(game_dir).await.ok();

    import_manual_content(services, cluster, game_dir).await;

    for content_type in SWAP_TYPES {
        let dir = game_dir.join(content_type.folder_name());
        polyio::create_dir_all(&dir).await.ok();
        clear_content_files(&dir).await;
        ensure_note(&dir, content_type).await;
    }

    let linked = PackageStore::list_linked_artifacts(cluster.id, services).await?;
    for link in linked {
        if !link.enabled || !SWAP_TYPES.contains(&link.content_type) {
            continue;
        }

        let Some(artifact) = artifact_dao::get_artifact_by_hash(&services.db, &link.hash).await?
        else {
            continue;
        };

        let src = artifact_absolute_path(&artifact.path)?;
        if !polyio::try_exists(&src).await.unwrap_or(false) {
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

#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id), level = "debug")]
pub async fn import_manual_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
) {
    let linked = match PackageStore::list_linked_artifacts(cluster.id, services).await {
        Ok(linked) => linked,
        Err(err) => {
            tracing::warn!(error = %err, "failed to list links; skipping manual-content import");
            return;
        }
    };

    for content_type in SWAP_TYPES {
        let dir = game_dir.join(content_type.folder_name());
        let Ok(mut entries) = polyio::read_dir(&dir).await else {
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
                    tracing::debug!(file = name, "registered manually-added shared content")
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

const ALLOWED_SYMLINKS_NAME: &str = "allowed_symlinks.txt";

#[tracing::instrument(level = "debug")]
pub async fn write_allowed_symlinks(game_dir: &Path) -> LauncherResult<()> {
    let root = crate::paths::launcher_dir()?;
    let base = polyio::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let sep = std::path::MAIN_SEPARATOR;

    let body = format!("[prefix]{}{}", base.to_string_lossy(), sep);

    polyio::write(game_dir.join(ALLOWED_SYMLINKS_NAME), body).await?;
    Ok(())
}

const EMPTY_NOTE_NAME: &str = "WHY_NOTHING_HERE.txt";

async fn clear_content_files(dir: &Path) {
    let Ok(mut entries) = polyio::read_dir(dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name == EMPTY_NOTE_NAME || name.starts_with('.') {
            continue;
        }

        let Ok(file_type) = entry.file_type().await else {
            continue;
        };

        if !file_type.is_file() && !file_type.is_symlink() {
            continue;
        }

        polyio::remove_file(&path).await.ok();
    }
}

async fn ensure_note(dir: &Path, content_type: ContentType) {
    let note = dir.join(EMPTY_NOTE_NAME);

    if polyio::try_exists(&note).await.unwrap_or(false) {
        return;
    }

    polyio::write(&note, {
        let noun = content_type.folder_name();

        format!(
            "It's empty here, but nothing is broken!\n\
        \n\
        OneClient keeps your {noun} safe somewhere else (specifically in the global launcher cache)\
		and only puts them here while you play. When you close the game, it tidies them away again.\n\
        \n\
        Want to add {noun}? The best way is to do it right inside OneClient. Or you can drop \
        files in this folder, and OneClient will pick them up the next time you play.\n"
        )
    })
    .await
    .ok();
}

#[tracing::instrument(level = "debug")]
pub async fn clear_shared_content(game_dir: &Path) -> LauncherResult<()> {
    for content_type in SWAP_TYPES {
        let dir = game_dir.join(content_type.folder_name());
        polyio::create_dir_all(&dir).await.ok();
        clear_content_files(&dir).await;
        ensure_note(&dir, content_type).await;
    }
    Ok(())
}

#[tracing::instrument(skip(cluster), fields(cluster_id = cluster.id), level = "debug")]
pub async fn link_cluster_logs(cluster: &Cluster, game_dir: &Path) {
    let cluster_dir = match cluster.dir() {
        Ok(dir) => dir,
        Err(err) => {
            tracing::warn!(error = %err, "cannot resolve cluster dir; skipping log redirect");
            return;
        }
    };

    for name in REDIRECTED_DIRS {
        let target = cluster_dir.join(name);
        let shared = game_dir.join(name);

        if let Err(err) = redirect_dir(&shared, &target).await {
            tracing::warn!(
                dir = name,
                error = %err,
                "failed to redirect shared game dir into cluster; logs may pool in shared dir"
            );
        }
    }
}

async fn redirect_dir(shared: &Path, target: &Path) -> LauncherResult<()> {
    polyio::create_dir_all(target).await.ok();

    match polyio::symlink_metadata(shared).await {
        // check if a symlink from a previous launch
        Ok(meta) if meta.file_type().is_symlink() => {
            polyio::remove_symlink_dir(shared).await?;
        }

        // real directory (most likely either done by the user or a tool)
        // so instead of deleting it we first move the contents of it into the designated
        // cluster folder
        Ok(meta) if meta.is_dir() => {
            move_dir_contents(shared, target).await;
            polyio::remove_dir_all(shared).await.ok();
        }

        // some file, so remove it so the link can take its place.
        Ok(_) => {
            polyio::remove_file(shared).await.ok();
        }

        Err(_) => {}
    }

    if let Some(parent) = shared.parent() {
        polyio::create_dir_all(parent).await.ok();
    }
    polyio::symlink_dir(target, shared).await?;
    Ok(())
}

#[tracing::instrument(level = "debug")]
pub async fn unlink_cluster_logs(game_dir: &Path) {
    for name in REDIRECTED_DIRS {
        let shared = game_dir.join(name);

        match polyio::symlink_metadata(&shared).await {
            Ok(meta) if meta.file_type().is_symlink() => {
                if let Err(err) = polyio::remove_symlink_dir(&shared).await {
                    tracing::warn!(dir = name, error = %err, "failed to unlink shared log dir");
                }
            }
            Ok(_) => {
                tracing::warn!(
                    dir = name,
                    "shared log dir is not our link; leaving as-is (next launch will salvage it)"
                );
            }
            Err(_) => {}
        }
    }
}

async fn move_dir_contents(from: &Path, to: &Path) {
    let Ok(mut entries) = polyio::read_dir(from).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let src = entry.path();
        let Some(name) = src.file_name() else {
            continue;
        };

        let dest = to.join(name);

        if polyio::rename(&src, &dest).await.is_err() {
            tracing::warn!(file = %src.display(), "failed to salvage leaked log file");
        }
    }
}

async fn sync_fabric_dep_overrides(cluster: &Cluster, game_dir: &Path) -> LauncherResult<()> {
    let src = cluster.dir()?.join(FABRIC_DEP_OVERRIDES);
    let dest = game_dir.join(FABRIC_DEP_OVERRIDES);

    if polyio::try_exists(&src).await.unwrap_or(false) {
        if let Some(parent) = dest.parent() {
            polyio::create_dir_all(parent).await.ok();
        }
        polyio::copy(&src, &dest).await?;
    } else if polyio::try_exists(&dest).await.unwrap_or(false) {
        polyio::remove_file(&dest).await.ok();
    }

    Ok(())
}
