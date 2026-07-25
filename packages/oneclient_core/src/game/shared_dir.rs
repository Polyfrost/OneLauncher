use std::collections::HashSet;
use std::fs::FileType;
use std::path::Path;

use oneclient_db::dao::artifact as artifact_dao;

use crate::LauncherResult;
use crate::clusters::Cluster;
use crate::packages::domain::ContentType;
use crate::packages::store::{artifact_absolute_path, link_or_copy};
use crate::packages::{LinkedArtifactInfo, PackageStore};
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

    let linked = PackageStore::list_linked_artifacts(cluster.id, services).await?;

    for content_type in SWAP_TYPES {
        let dir = game_dir.join(content_type.folder_name());
        let stash = cluster.dir()?.join(content_type.folder_name());
        polyio::create_dir_all(&dir).await.ok();

        let managed = managed_names(&linked, content_type);

        // Anything still here belongs to whoever played last (or crashed last)
        // — take it into this cluster rather than deleting it.
        stash_content_files(&dir, &stash, &managed).await;
        ensure_note(&dir, content_type).await;
        restore_stashed(&stash, &dir, &managed).await;
    }

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

/// The file names the package store owns inside a content folder. Everything
/// else in there was put there by the game or the user.
fn managed_names(linked: &[LinkedArtifactInfo], content_type: ContentType) -> HashSet<String> {
    linked
        .iter()
        .filter(|link| link.content_type == content_type)
        .map(|link| link.cluster_file_name.clone())
        .collect()
}

/// Empty a shared content folder without losing anything the game wrote into
/// it.
///
/// Managed content is dropped — the cluster folder already holds an equivalent
/// link into the artifact cache. Everything else (a shaderpack's settings
/// sidecar, an unzipped pack, a stray config) is *moved* into the cluster's own
/// copy of the folder, and [`restore_stashed`] links it back on the next
/// launch. That is what keeps shader configs attached to the cluster they were
/// tuned in instead of being wiped by the next launch of any cluster.
async fn stash_content_files(dir: &Path, stash: &Path, managed: &HashSet<String>) {
    let Ok(mut entries) = polyio::read_dir(dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if name == EMPTY_NOTE_NAME || name.starts_with('.') {
            continue;
        }

        let Ok(file_type) = entry.file_type().await else {
            continue;
        };

        // Ours: either a link we made this launch, or a name the store tracks
        // (on Windows `symlink_file` hard-links, so ours is not always a
        // symlink). Either way the cluster folder has it covered.
        if file_type.is_symlink() || managed.contains(&name) {
            remove_entry(&path, file_type).await;
            continue;
        }

        let dest = stash.join(&name);
        if let Err(err) = move_entry(&path, &dest).await {
            tracing::warn!(
                file = %name,
                error = %err,
                "failed to stash shared content into cluster; leaving it in place"
            );
        }
    }
}

/// Link the cluster's stashed leftovers back into the shared folder so the
/// game finds them where it left them. The game writes straight through the
/// link, so edits land in the cluster folder even if we never get to run on
/// exit.
async fn restore_stashed(stash: &Path, dir: &Path, managed: &HashSet<String>) {
    let Ok(mut entries) = polyio::read_dir(stash).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if name == EMPTY_NOTE_NAME || name.starts_with('.') || managed.contains(&name) {
            continue;
        }

        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        // A link into the artifact cache — managed content is linked from the
        // database further down, not from here.
        if file_type.is_symlink() {
            continue;
        }

        let dest = dir.join(&name);
        let result = if file_type.is_dir() {
            polyio::symlink_dir(&path, &dest).await.map_err(Into::into)
        } else {
            link_or_copy(&path, &dest).await
        };

        if let Err(err) = result {
            tracing::warn!(
                file = %name,
                error = %err,
                "failed to restore stashed content into shared dir"
            );
        }
    }
}

async fn remove_entry(path: &Path, file_type: FileType) {
    if file_type.is_dir() {
        polyio::remove_dir_all(path).await.ok();
    } else if polyio::remove_file(path).await.is_err() {
        // A Windows junction has to go through `remove_dir`.
        polyio::remove_symlink_dir(path).await.ok();
    }
}

async fn move_entry(src: &Path, dest: &Path) -> LauncherResult<()> {
    if let Some(parent) = dest.parent() {
        polyio::create_dir_all(parent).await.ok();
    }

    // A stash left from an earlier session is always older than what the game
    // just wrote, so it loses.
    if let Ok(meta) = polyio::symlink_metadata(dest).await {
        remove_entry(dest, meta.file_type()).await;
    }

    if polyio::rename(src, dest).await.is_ok() {
        return Ok(());
    }

    // `rename` cannot cross devices; fall back to a copy for plain files.
    polyio::copy(src, dest).await?;
    polyio::remove_file(src).await.ok();
    Ok(())
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

#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id), level = "debug")]
pub async fn clear_shared_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
) -> LauncherResult<()> {
    // Run after `import_manual_content`, so anything the user dropped in is
    // already a tracked artifact by now and gets dropped rather than stashed.
    let linked = PackageStore::list_linked_artifacts(cluster.id, services)
        .await
        .unwrap_or_default();

    for content_type in SWAP_TYPES {
        let dir = game_dir.join(content_type.folder_name());
        let stash = cluster.dir()?.join(content_type.folder_name());
        polyio::create_dir_all(&dir).await.ok();

        stash_content_files(&dir, &stash, &managed_names(&linked, content_type)).await;
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn tmp_root(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);

        let dir =
            std::env::temp_dir().join(format!("oneclient_shd_{}_{tag}_{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    fn managed(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| (*n).to_string()).collect()
    }

    /// A shaderpack's settings sidecar has to survive a launch/exit cycle, and
    /// end up in the cluster rather than the shared dir.
    #[tokio::test]
    async fn shader_settings_survive_a_session() {
        let root = tmp_root("shader_settings");
        let shared = root.join("shared").join("shaderpacks");
        let stash = root.join("cluster").join("shaderpacks");
        polyio::create_dir_all(&shared).await.unwrap();

        let names = managed(&["bsl.zip"]);

        // Play: the pack is linked in, the game writes its settings next to it.
        polyio::write(shared.join("bsl.zip"), b"pack".as_slice())
            .await
            .unwrap();
        polyio::write(shared.join("bsl.zip.txt"), b"BLOOM=off".as_slice())
            .await
            .unwrap();

        // Exit.
        stash_content_files(&shared, &stash, &names).await;
        assert!(!shared.join("bsl.zip").exists(), "managed pack left behind");
        assert!(!shared.join("bsl.zip.txt").exists(), "sidecar left behind");
        assert_eq!(
            polyio::read_to_string(stash.join("bsl.zip.txt")).await.unwrap(),
            "BLOOM=off"
        );

        // Next launch.
        restore_stashed(&stash, &shared, &names).await;
        assert_eq!(
            polyio::read_to_string(shared.join("bsl.zip.txt")).await.unwrap(),
            "BLOOM=off"
        );

        // The game edits its settings through the link we restored.
        polyio::write(stash.join("bsl.zip.txt"), b"BLOOM=on".as_slice())
            .await
            .unwrap();
        assert_eq!(
            polyio::read_to_string(shared.join("bsl.zip.txt")).await.unwrap(),
            "BLOOM=on"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// An unzipped pack is a directory; it moves out and comes back as a link.
    #[tokio::test]
    async fn unpacked_dirs_move_into_the_cluster() {
        let root = tmp_root("unpacked");
        let shared = root.join("shared").join("shaderpacks");
        let stash = root.join("cluster").join("shaderpacks");
        polyio::create_dir_all(shared.join("Loose/shaders")).await.unwrap();
        polyio::write(shared.join("Loose/shaders/final.fsh"), b"void main".as_slice())
            .await
            .unwrap();

        stash_content_files(&shared, &stash, &HashSet::new()).await;
        assert!(!shared.join("Loose").exists());
        assert!(stash.join("Loose/shaders/final.fsh").exists());

        restore_stashed(&stash, &shared, &HashSet::new()).await;
        assert!(shared.join("Loose/shaders/final.fsh").exists());

        // Only the link goes, never the stashed original.
        stash_content_files(&shared, &stash, &HashSet::new()).await;
        assert!(!shared.join("Loose").exists());
        assert!(stash.join("Loose/shaders/final.fsh").exists());

        std::fs::remove_dir_all(&root).ok();
    }

    /// The note explaining the empty folder is ours, and stays put.
    #[tokio::test]
    async fn note_is_never_stashed() {
        let root = tmp_root("note");
        let shared = root.join("shared").join("mods");
        let stash = root.join("cluster").join("mods");
        polyio::create_dir_all(&shared).await.unwrap();
        polyio::write(shared.join(EMPTY_NOTE_NAME), b"hi".as_slice())
            .await
            .unwrap();

        stash_content_files(&shared, &stash, &HashSet::new()).await;
        assert!(shared.join(EMPTY_NOTE_NAME).exists());
        assert!(!stash.join(EMPTY_NOTE_NAME).exists());

        std::fs::remove_dir_all(&root).ok();
    }
}
