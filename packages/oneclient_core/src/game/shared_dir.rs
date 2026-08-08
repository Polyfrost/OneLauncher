use std::collections::HashSet;
use std::fs::FileType;
use std::path::Path;

use oneclient_db::dao::artifact as artifact_dao;

use crate::LauncherResult;
use crate::clusters::Cluster;
use oneclient_common::domain::ContentType;
use oneclient_content::packages::store::manifest::{
    self, ManifestEntry, MaterializedManifest,
};
use oneclient_content::packages::store::{artifact_absolute_path, link_or_copy, remove_entry};
use oneclient_content::packages::PackageStore;
use crate::state::LauncherServices;

const REDIRECTED_DIRS: [&str; 2] = ["logs", "crash-reports"];

const SWAP_TYPES: [ContentType; 3] = [
    ContentType::Mod,
    ContentType::ResourcePack,
    ContentType::Shader,
];

const FABRIC_DEP_OVERRIDES: &str = "config/fabric_loader_dependencies.json";

struct Desired {
    content_type: ContentType,
    file_name: String,
    hash: String,
    /// Where it lives in the artifact cache
    src: std::path::PathBuf,
}

impl Desired {
    fn relative_path(&self) -> String {
        manifest::entry_path(self.content_type.folder_name(), &self.file_name)
    }
}

/// Safe to run over a directory left by a crashed session another cluster or
/// a launcher version predating the manifest
#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id, game_dir = %game_dir.display()), level = "debug")]
pub async fn materialize_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
) -> LauncherResult<()> {
    let dedicated = cluster.uses_dedicated_dir();
    polyio::create_dir_all(game_dir).await.ok();

    crate::game::heal::clear_zeroed_files(game_dir).await;

    // In the shared directory this often belongs to another cluster so every
    // use of it checks the id
    let previous = manifest::load(game_dir).await;

    import_manual_content(services, cluster, game_dir).await;

    // Before the folder is built not after handing the game several enabled
    // versions of one mod is a classloader conflict
    if let Err(err) = oneclient_content::bundles::reconcile_duplicate_activity(
        cluster.id,
        &services.content(),
    )
    .await
    {
        // Not worth blocking a launch the duplicates were already there
        tracing::warn!(cluster_id = cluster.id, %err, "failed to resolve duplicate package versions");
    }

    let desired = desired_content(services, cluster).await?;
    let desired_paths: HashSet<String> = desired.iter().map(Desired::relative_path).collect();

    // While the game is still closed this is what lands a package removed
    // mid-session and clears another cluster's content from the shared dir
    prune_previous(game_dir, previous.as_ref(), &desired_paths).await;

    if !dedicated {
        let linked = PackageStore::list_linked_artifacts(cluster.id, &services.content())
            .await
            .unwrap_or_default();

        for content_type in SWAP_TYPES {
            let dir = game_dir.join(content_type.folder_name());
            let stash = cluster.dir()?.join(content_type.folder_name());
            polyio::create_dir_all(&dir).await.ok();

            // Whatever is still here belongs to whoever played (or crashed)
            // last so take it into this cluster rather than deleting it
            let ours = ours_in_folder(content_type, &linked, previous.as_ref());
            stash_content_files(&dir, &stash, &ours).await;
            ensure_note(&dir, content_type).await;
            restore_stashed(&stash, &dir, content_type, &ours).await;
        }
    }

    let entries = link_desired(game_dir, &desired).await;
    manifest::save(game_dir, &MaterializedManifest::new(cluster.id, entries)).await;

    sync_fabric_dep_overrides(cluster, game_dir).await?;

    Ok(())
}

/// Only for shared directories a dedicated cluster's content stays put between
/// sessions and [`materialize_content`] reconciles it at the next launch
#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id), level = "debug")]
pub async fn dematerialize_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
) -> LauncherResult<()> {
    // Runs first so anything dropped in during the session is a tracked artifact
    // by now and gets dropped rather than stashed as a loose file
    import_manual_content(services, cluster, game_dir).await;

    let current = manifest::load(game_dir).await;
    let linked = PackageStore::list_linked_artifacts(cluster.id, &services.content())
        .await
        .unwrap_or_default();

    for content_type in SWAP_TYPES {
        let dir = game_dir.join(content_type.folder_name());
        let stash = cluster.dir()?.join(content_type.folder_name());
        polyio::create_dir_all(&dir).await.ok();

        let ours = ours_in_folder(content_type, &linked, current.as_ref());
        stash_content_files(&dir, &stash, &ours).await;
        ensure_note(&dir, content_type).await;
    }

    manifest::clear(game_dir).await;
    Ok(())
}

async fn desired_content(
    services: &LauncherServices,
    cluster: &Cluster,
) -> LauncherResult<Vec<Desired>> {
    let linked = PackageStore::list_linked_artifacts(cluster.id, &services.content()).await?;
    let mut desired = Vec::with_capacity(linked.len());

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
            tracing::warn!(hash = %link.hash, "cached artifact missing; skipping");
            continue;
        }

        desired.push(Desired {
            content_type: link.content_type,
            file_name: link.cluster_file_name,
            hash: link.hash,
            src,
        });
    }

    Ok(desired)
}

/// Only files that made it are recorded so a failed link is never later
/// mistaken for ours and deleted out from under the user
async fn link_desired(game_dir: &Path, desired: &[Desired]) -> Vec<ManifestEntry> {
    let mut entries = Vec::with_capacity(desired.len());

    for item in desired {
        let dest = game_dir
            .join(item.content_type.folder_name())
            .join(&item.file_name);

        match link_or_copy(&item.src, &dest).await {
            Ok(()) => entries.push(ManifestEntry {
                path: item.relative_path(),
                hash: item.hash.clone(),
            }),
            Err(err) => tracing::warn!(
                file = %item.file_name,
                error = %err,
                "failed to materialize content into the game directory"
            ),
        }
    }

    entries
}

/// Entries are keyed by path so a package whose file name is unchanged is left
/// in place rather than being deleted and relinked on every launch
async fn prune_previous(
    game_dir: &Path,
    previous: Option<&MaterializedManifest>,
    keep: &HashSet<String>,
) {
    let Some(previous) = previous else {
        return;
    };

    for entry in &previous.entries {
        if keep.contains(&entry.path) {
            continue;
        }

        let path = game_dir.join(&entry.path);
        if let Err(err) = remove_entry(&path).await {
            tracing::warn!(
                file = %entry.path,
                error = %err,
                "failed to clear stale materialized content"
            );
        }
    }
}

/// Names in one folder that are the launcher's not the user's what we
/// materialized last plus what the database tracks (covering files just
/// adopted by [`import_manual_content`])
fn ours_in_folder(
    content_type: ContentType,
    linked: &[oneclient_content::packages::LinkedArtifactInfo],
    manifest: Option<&MaterializedManifest>,
) -> HashSet<String> {
    let mut names: HashSet<String> = linked
        .iter()
        .filter(|link| link.content_type == content_type)
        .map(|link| link.cluster_file_name.clone())
        .collect();

    let folder = content_type.folder_name();
    if let Some(manifest) = manifest {
        names.extend(
            manifest
                .entries
                .iter()
                .filter_map(|entry| entry.path.strip_prefix(&format!("{folder}/")))
                .map(str::to_owned),
        );
    }

    names
}

#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id), level = "debug")]
pub async fn import_manual_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
) {
    let linked = match PackageStore::list_linked_artifacts(cluster.id, &services.content()).await {
        Ok(linked) => linked,
        Err(err) => {
            tracing::warn!(error = %err, "failed to list links; skipping manual-content import");
            return;
        }
    };

    let manifest = manifest::load(game_dir).await;

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

            // Ours from a previous launch `prune_previous` handles it
            // Taking it as a user drop-in would reinstall a just-deleted package
            let relative = manifest::entry_path(content_type.folder_name(), name);
            if manifest.as_ref().is_some_and(|m| m.contains(&relative)) {
                continue;
            }

            // No manifest a directory from a launcher version predating it
            // If the cache holds this exact file the launcher put it here
            if manifest.is_none() && is_cached_artifact(services, &path).await {
                tracing::debug!(file = name, "discarding stale launcher content in game dir");
                if let Err(err) = polyio::remove_file(&path).await {
                    tracing::warn!(file = name, error = %err, "failed to discard stale content");
                }
                continue;
            }

            match PackageStore::import_local_file(&path, content_type, cluster.id, &services.content()).await {
                Ok(_) => {
                    tracing::debug!(file = name, "registered manually-added content")
                }
                Err(err) => tracing::warn!(
                    file = name,
                    error = %err,
                    "failed to register manually-added content"
                ),
            }
        }
    }
}

/// Already in the artifact cache i.e. the launcher put it in the game dir
/// rather than the user dropping it there
async fn is_cached_artifact(services: &LauncherServices, path: &Path) -> bool {
    let Ok(hash) = polyio::sha1_file(path).await else {
        return false;
    };

    artifact_dao::get_artifact_by_hash(&services.db, &polyio::normalize_hash(&hash))
        .await
        .ok()
        .flatten()
        .is_some()
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
    let root = oneclient_common::paths::launcher_dir()?;
    let base = polyio::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let sep = std::path::MAIN_SEPARATOR;

    let body = format!("[prefix]{}{}", base.to_string_lossy(), sep);

    polyio::write(game_dir.join(ALLOWED_SYMLINKS_NAME), body).await?;
    Ok(())
}

const EMPTY_NOTE_NAME: &str = "WHY_NOTHING_HERE.txt";

/// Launcher-owned content is dropped (the cache has it) everything else
/// (sidecars unzipped packs stray configs) is *moved* into the cluster folder
/// so it stays attached to that cluster and [`restore_stashed`] links it back
async fn stash_content_files(dir: &Path, stash: &Path, ours: &HashSet<String>) {
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

        // Ours and the cache has it covered
        // On Windows `symlink_file` hard-links so ours is not always a symlink
        if file_type.is_symlink() || ours.contains(&name) {
            remove_dir_or_file(&path, file_type).await;
            continue;
        }

        let dest = stash.join(&name);
        if let Err(err) = move_entry(&path, &dest).await {
            tracing::warn!(
                file = %name,
                error = %err,
                "failed to stash content into cluster; leaving it in place"
            );
        }
    }
}

/// Links stashed leftovers back so the game writes straight through into the
/// cluster folder even if we never get to run on exit
async fn restore_stashed(
    stash: &Path,
    dir: &Path,
    content_type: ContentType,
    ours: &HashSet<String>,
) {
    let Ok(mut entries) = polyio::read_dir(stash).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if name == EMPTY_NOTE_NAME || name.starts_with('.') || ours.contains(&name) {
            continue;
        }

        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        // A link into the artifact cache managed content is materialized from
        // the database not from here
        if file_type.is_symlink() {
            continue;
        }

        // A bare jar in the stash is a leftover from the old scheme often with
        // its package already removed restoring it would put a deleted mod back
        // every launch
        // Left on disk for the Storage settings page to clear
        if file_type.is_file() && has_content_extension(content_type, &name) {
            tracing::debug!(
                file = %name,
                "ignoring stray content in cluster folder; use the Storage settings page to clear it"
            );
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
                "failed to restore stashed content into game dir"
            );
        }
    }
}

async fn remove_dir_or_file(path: &Path, file_type: FileType) {
    if file_type.is_dir() {
        polyio::remove_dir_all(path).await.ok();
    } else if polyio::remove_file(path).await.is_err() {
        // A Windows junction has to go through `remove_dir`
        polyio::remove_symlink_dir(path).await.ok();
    }
}

async fn move_entry(src: &Path, dest: &Path) -> LauncherResult<()> {
    if let Some(parent) = dest.parent() {
        polyio::create_dir_all(parent).await.ok();
    }

    // A stash from an earlier session is older than what the game just wrote
    // so it loses
    if let Ok(meta) = polyio::symlink_metadata(dest).await {
        remove_dir_or_file(dest, meta.file_type()).await;
    }

    if polyio::rename(src, dest).await.is_ok() {
        return Ok(());
    }

    // `rename` cannot cross devices fall back to a copy for plain files
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
        Ok(meta) if meta.file_type().is_symlink() => {
            polyio::remove_symlink_dir(shared).await?;
        }

        // A real directory likely the user's so salvage its contents into the
        // cluster folder instead of deleting it
        Ok(meta) if meta.is_dir() => {
            move_dir_contents(shared, target).await;
            polyio::remove_dir_all(shared).await.ok();
        }

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

    if src == dest {
        return Ok(());
    }

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

    use super::*;

    fn names(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| (*n).to_string()).collect()
    }

    fn manifest_of(cluster_id: i64, paths: &[&str]) -> MaterializedManifest {
        MaterializedManifest::new(
            cluster_id,
            paths
                .iter()
                .map(|p| ManifestEntry {
                    path: (*p).to_string(),
                    hash: "hash".into(),
                })
                .collect(),
        )
    }

    /// A shaderpack's settings sidecar has to survive a launch/exit cycle and
    /// end up in the cluster rather than the shared dir
    #[tokio::test]
    async fn shader_settings_survive_a_session() {
        let root = polyio::testing::ScratchDir::new("shader_settings");
        let shared = root.join("shared").join("shaderpacks");
        let stash = root.join("cluster").join("shaderpacks");
        polyio::create_dir_all(&shared).await.unwrap();

        let ours = names(&["bsl.zip"]);

        polyio::write(shared.join("bsl.zip"), b"pack".as_slice())
            .await
            .unwrap();
        polyio::write(shared.join("bsl.zip.txt"), b"BLOOM=off".as_slice())
            .await
            .unwrap();

        stash_content_files(&shared, &stash, &ours).await;
        assert!(!shared.join("bsl.zip").exists(), "managed pack left behind");
        assert!(!shared.join("bsl.zip.txt").exists(), "sidecar left behind");
        assert_eq!(
            polyio::read_to_string(stash.join("bsl.zip.txt")).await.unwrap(),
            "BLOOM=off"
        );

        restore_stashed(&stash, &shared, ContentType::Shader, &ours).await;
        assert_eq!(
            polyio::read_to_string(shared.join("bsl.zip.txt")).await.unwrap(),
            "BLOOM=off"
        );

        polyio::write(stash.join("bsl.zip.txt"), b"BLOOM=on".as_slice())
            .await
            .unwrap();
        assert_eq!(
            polyio::read_to_string(shared.join("bsl.zip.txt")).await.unwrap(),
            "BLOOM=on"
        );

        std::fs::remove_dir_all(root.path()).ok();
    }

    #[tokio::test]
    async fn unpacked_dirs_move_into_the_cluster() {
        let root = polyio::testing::ScratchDir::new("unpacked");
        let shared = root.join("shared").join("shaderpacks");
        let stash = root.join("cluster").join("shaderpacks");
        polyio::create_dir_all(shared.join("Loose/shaders")).await.unwrap();
        polyio::write(shared.join("Loose/shaders/final.fsh"), b"void main".as_slice())
            .await
            .unwrap();

        stash_content_files(&shared, &stash, &HashSet::new()).await;
        assert!(!shared.join("Loose").exists());
        assert!(stash.join("Loose/shaders/final.fsh").exists());

        restore_stashed(&stash, &shared, ContentType::Shader, &HashSet::new()).await;
        assert!(shared.join("Loose/shaders/final.fsh").exists());

        // Only the link goes never the stashed original
        stash_content_files(&shared, &stash, &HashSet::new()).await;
        assert!(!shared.join("Loose").exists());
        assert!(stash.join("Loose/shaders/final.fsh").exists());

        std::fs::remove_dir_all(root.path()).ok();
    }

    #[tokio::test]
    async fn note_is_never_stashed() {
        let root = polyio::testing::ScratchDir::new("note");
        let shared = root.join("shared").join("mods");
        let stash = root.join("cluster").join("mods");
        polyio::create_dir_all(&shared).await.unwrap();
        polyio::write(shared.join(EMPTY_NOTE_NAME), b"hi".as_slice())
            .await
            .unwrap();

        stash_content_files(&shared, &stash, &HashSet::new()).await;
        assert!(shared.join(EMPTY_NOTE_NAME).exists());
        assert!(!stash.join(EMPTY_NOTE_NAME).exists());

        std::fs::remove_dir_all(root.path()).ok();
    }

    /// The reported bug a package removed while the game held it open stays in
    /// the folder so the next launch must clear it
    /// An ordinary file is used
    /// because that is what a managed file looks like on Windows
    #[tokio::test]
    async fn removed_package_is_pruned_at_the_next_launch() {
        let root = polyio::testing::ScratchDir::new("prune_removed");
        let game_dir = root.path();
        polyio::create_dir_all(game_dir.join("mods")).await.unwrap();

        let jar = game_dir.join("mods").join("removed.jar");
        polyio::write(&jar, b"jar".as_slice()).await.unwrap();

        let previous = manifest_of(1, &["mods/removed.jar", "mods/kept.jar"]);
        let keep: HashSet<String> = ["mods/kept.jar".to_string()].into_iter().collect();

        prune_previous(game_dir, Some(&previous), &keep).await;

        assert!(
            polyio::symlink_metadata(&jar).await.is_err(),
            "a removed package must not survive into the next session"
        );

        std::fs::remove_dir_all(root.path()).ok();
    }

    /// Whatever a previous cluster materialized in the shared dir is ours to
    /// clear not the user's to keep
    #[tokio::test]
    async fn another_clusters_content_is_cleared_not_stashed() {
        let root = polyio::testing::ScratchDir::new("cross_cluster");
        let game_dir = root.path();
        polyio::create_dir_all(game_dir.join("mods")).await.unwrap();

        let jar = game_dir.join("mods").join("theirs.jar");
        polyio::write(&jar, b"jar".as_slice()).await.unwrap();

        prune_previous(game_dir, Some(&manifest_of(1, &["mods/theirs.jar"])), &HashSet::new()).await;

        assert!(polyio::symlink_metadata(&jar).await.is_err());

        std::fs::remove_dir_all(root.path()).ok();
    }

    /// A hand-dropped file is in no manifest so it is stashed into the cluster
    /// rather than deleted
    #[tokio::test]
    async fn user_files_are_never_pruned() {
        let root = polyio::testing::ScratchDir::new("user_file");
        let game_dir = root.path();
        polyio::create_dir_all(game_dir.join("mods")).await.unwrap();

        let mine = game_dir.join("mods").join("handmade.jar");
        polyio::write(&mine, b"jar".as_slice()).await.unwrap();

        prune_previous(game_dir, Some(&manifest_of(1, &["mods/ours.jar"])), &HashSet::new()).await;

        assert!(mine.exists(), "a file we never materialized is not ours to delete");

        std::fs::remove_dir_all(root.path()).ok();
    }

    /// A jar left in a cluster folder by an older launcher whose package has
    /// since been removed must not be linked back into the game
    #[tokio::test]
    async fn stray_content_in_the_stash_is_not_restored() {
        let root = polyio::testing::ScratchDir::new("stray_content");
        let shared = root.join("shared").join("mods");
        let stash = root.join("cluster").join("mods");
        polyio::create_dir_all(&shared).await.unwrap();
        polyio::create_dir_all(&stash).await.unwrap();

        // A leftover from the old scheme an ordinary file (which is what a
        // Windows hard link looks like) with no database row to explain it
        polyio::write(stash.join("removed.jar"), b"jar".as_slice())
            .await
            .unwrap();
        polyio::write(stash.join("options.txt"), b"k=v".as_slice())
            .await
            .unwrap();

        restore_stashed(&stash, &shared, ContentType::Mod, &HashSet::new()).await;

        assert!(
            !shared.join("removed.jar").exists(),
            "a removed package must not be restored into the game"
        );
        assert!(
            stash.join("removed.jar").exists(),
            "and it must not be silently deleted either"
        );
        assert!(shared.join("options.txt").exists(), "sidecars still restore");

        std::fs::remove_dir_all(root.path()).ok();
    }

    /// Pinned because the halves are easy to swap `stash` reads the game dir
    /// and writes the cluster `restore` the reverse and both take two
    /// same-typed `&Path`s a swap would quietly delete a user's files
    #[tokio::test]
    async fn a_session_never_removes_anything_from_the_cluster_folder() {
        let root = polyio::testing::ScratchDir::new("stash_is_sacred");
        let shared = root.join("shared").join("mods");
        let stash = root.join("cluster").join("mods");
        polyio::create_dir_all(&shared).await.unwrap();
        polyio::create_dir_all(&stash).await.unwrap();

        polyio::write(stash.join("options.txt"), b"k=v".as_slice())
            .await
            .unwrap();
        polyio::write(stash.join("leftover.jar"), b"jar".as_slice())
            .await
            .unwrap();
        polyio::create_dir_all(stash.join("unpacked")).await.unwrap();
        polyio::write(stash.join("unpacked").join("inner.txt"), b"x".as_slice())
            .await
            .unwrap();

        polyio::write(shared.join("managed.jar"), b"jar".as_slice())
            .await
            .unwrap();

        let ours = names(&["managed.jar"]);

        let before = dir_entries(&stash).await;

        stash_content_files(&shared, &stash, &ours).await;
        restore_stashed(&stash, &shared, ContentType::Mod, &ours).await;
        stash_content_files(&shared, &stash, &ours).await;
        restore_stashed(&stash, &shared, ContentType::Mod, &ours).await;

        let after = dir_entries(&stash).await;

        assert_eq!(
            before, after,
            "a session must not add to or remove from the cluster folder"
        );
        assert!(
            stash.join("unpacked").join("inner.txt").exists(),
            "the contents of a stashed directory survive too"
        );

        std::fs::remove_dir_all(root.path()).ok();
    }

    async fn dir_entries(dir: &Path) -> Vec<String> {
        let mut names = Vec::new();
        let Ok(mut entries) = polyio::read_dir(dir).await else {
            return names;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        names
    }

    #[test]
    fn ownership_spans_the_manifest_and_the_database() {
        let manifest = manifest_of(1, &["mods/from_manifest.jar", "shaderpacks/bsl.zip"]);
        let linked = Vec::new();

        let ours = ours_in_folder(ContentType::Mod, &linked, Some(&manifest));

        assert!(ours.contains("from_manifest.jar"));
        // Scoped to the folder a shaderpack entry must not make a mod of the
        // same name look managed
        assert!(!ours.contains("bsl.zip"));
    }
}
