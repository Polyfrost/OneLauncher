use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use oneclient_cluster::{Cluster, ClusterError, ClusterUpdate};
use oneclient_common::domain::ContentType;
use oneclient_common::patch::Patch;
use oneclient_common::paths;
use oneclient_content::packages::store::manifest::{self, MaterializedManifest};
use oneclient_db::dao::applied_migration as migration_dao;
use oneclient_db::dao::cluster as cluster_dao;
use oneclient_events::GroupedProgressSession;

use crate::LauncherResult;
use crate::clusters::java_override::BUNDLE_JAVA_OVERRIDE;
use crate::clusters::modpack::{linked_modpack_archive, repair_modpack_cluster};
use crate::state::LauncherState;

const KEPT_DIRS: [&str; 4] = ["saves", "screenshots", "logs", "crash-reports"];

const GLOBAL_TYPES: [ContentType; 2] = [ContentType::ResourcePack, ContentType::Shader];

const OVERRIDE_LOCK: &str = ".oneclient/bundle_overrides.json";

/// Set from the first destructive step until the content is back, so a reset that
/// failed or was cut short by a crash still keeps the half-emptied cluster from launching
const RESET_UNFINISHED: &str = "reset-unfinished";

static RESETTING: Mutex<BTreeSet<i64>> = Mutex::new(BTreeSet::new());

#[derive(Debug, Clone, Default)]
pub struct ResetReport {
    pub backup_dir: Option<PathBuf>,
    pub kept_shared_configs: bool,
    pub not_moved: Vec<String>,
    /// Why reinstalling the content failed, the cluster stays blocked from launching
    /// until a later reset finishes
    pub reinstall_error: Option<String>,
}

/// Marks a cluster as resetting until dropped. Callers take it before handing the reset to a
/// background task so the cluster never looks free in between
pub struct ResetClaim(i64);

impl ResetClaim {
    #[must_use]
    pub fn take(cluster_id: i64) -> Option<Self> {
        RESETTING
            .lock()
            .unwrap()
            .insert(cluster_id)
            .then_some(Self(cluster_id))
    }

    #[must_use]
    pub fn cluster_id(&self) -> i64 {
        self.0
    }
}

impl Drop for ResetClaim {
    fn drop(&mut self) {
        RESETTING.lock().unwrap().remove(&self.0);
    }
}

#[must_use]
pub fn is_resetting(cluster_id: i64) -> bool {
    RESETTING.lock().unwrap().contains(&cluster_id)
}

pub async fn reset_unfinished(state: &LauncherState, cluster_id: i64) -> LauncherResult<bool> {
    Ok(migration_dao::is_applied(&state.services.db, &unfinished_marker(cluster_id)).await?)
}

fn unfinished_marker(cluster_id: i64) -> String {
    format!("{RESET_UNFINISHED}:{cluster_id}")
}

/// `claim` is held for the whole reset. It has to be taken before the activity check below,
/// while a launch marks itself active before checking for a reset, so whichever comes second
/// sees the other
#[tracing::instrument(skip_all, fields(cluster_id = claim.cluster_id()))]
pub async fn reset_cluster(
    state: &Arc<LauncherState>,
    claim: ResetClaim,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ResetReport> {
    let cluster_id = claim.cluster_id();
    let cluster = state.clusters.get(cluster_id).await?;
    if state.games.is_active(cluster_id) {
        return Err(ClusterError::AlreadyRunning(cluster_id).into());
    }
    if cluster.stage.is_busy() {
        return Err(ClusterError::Busy(cluster.stage).into());
    }

    let mut report = ResetReport {
        kept_shared_configs: !cluster.uses_dedicated_dir(),
        ..Default::default()
    };

    let modpack = if cluster.linked_modpack_hash.is_some() {
        Some(linked_modpack_archive(state, &cluster).await?.1)
    } else {
        None
    };

    {
        let _mods_sync = crate::game::lock_mods_sync(cluster_id).await;

        migration_dao::mark_applied(&state.services.db, &unfinished_marker(cluster_id)).await?;
        let kept_types: Vec<i64> = if cluster.is_isolated() {
            Vec::new()
        } else {
            GLOBAL_TYPES.iter().map(|kind| *kind as i64).collect()
        };
        cluster_dao::reset_content_state(&state.services.db, cluster_id, &kept_types).await?;
        migration_dao::forget_prefix(
            &state.services.db,
            &format!("{BUNDLE_JAVA_OVERRIDE}:{cluster_id}:"),
        )
        .await?;

        let global = state.settings.read().global_game_settings.clone();
        state
            .clusters
            .create_and_assign_profile(&global, cluster_id, &cluster.folder_name)
            .await?;

        let loader_version = match (&modpack, cluster.uses_bundles() && !cluster.user_created) {
            (Some(manifest), _) => Some(manifest.loader_version.clone()),
            (None, true) => Some(None),
            (None, false) => None,
        };
        if let Some(version) = loader_version {
            let update = ClusterUpdate {
                mc_loader_version: version.map_or(Patch::Clear, Patch::Set),
                ..Default::default()
            };
            state.clusters.update(cluster_id, update).await?;
        }

        move_cluster_files(&cluster, &mut report).await?;
    }

    tracing::info!(
        cluster_id,
        backup = ?report.backup_dir,
        not_moved = report.not_moved.len(),
        "reset cluster state; reinstalling its content"
    );

    if let Err(err) = reinstall(state, &cluster, modpack.is_some(), progress).await {
        tracing::error!(cluster_id, error = %err, "failed to reinstall the content of a reset cluster");
        report.reinstall_error = Some(err.to_string());
        return Ok(report);
    }

    migration_dao::forget(&state.services.db, &unfinished_marker(cluster_id)).await?;
    Ok(report)
}

async fn reinstall(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    modpack: bool,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<()> {
    let cluster_id = cluster.id;
    if modpack {
        repair_modpack_cluster(state, cluster_id, progress).await?;
    } else if cluster.uses_bundles() {
        oneclient_content::bundles::install_cluster_bundles(
            cluster_id,
            state.bundles.as_ref(),
            progress,
            &state.services.content(),
        )
        .await?;
        if let Err(err) =
            crate::clusters::apply_bundle_java_override(state, cluster_id, true, true, progress)
                .await
        {
            tracing::warn!(cluster_id, error = %err, "failed to apply the bundle java override after a reset");
        }
    }
    Ok(())
}

async fn move_cluster_files(cluster: &Cluster, report: &mut ResetReport) -> LauncherResult<()> {
    let cluster_dir = cluster.dir()?;
    let cover = cluster.cover_file().and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    });

    let mut moving: Vec<String> = Vec::new();
    if let Ok(mut entries) = polyio::read_dir(&cluster_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_kept(cluster, &name, cover.as_deref()) {
                continue;
            }
            moving.push(name);
        }
    }
    if polyio::symlink_metadata(cluster_dir.join(OVERRIDE_LOCK))
        .await
        .is_ok()
    {
        moving.push(OVERRIDE_LOCK.to_string());
    }

    {
        let _manifest = manifest::lock().await;
        manifest::clear(&cluster_dir, manifest::MANIFEST_NAME).await;
        if manifest::mods_live_in_cluster(&cluster_dir).await {
            manifest::save(
                &cluster_dir,
                manifest::MODS_MANIFEST_NAME,
                &MaterializedManifest::new(cluster.id, Vec::new()),
            )
            .await;
        }
    }

    if moving.is_empty() {
        return Ok(());
    }

    let backup = paths::cluster_reset_backups_dir()?.join(format!(
        "{}-{}-{}",
        cluster.id,
        cluster.folder_name,
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    polyio::create_dir_all(&backup).await?;

    for rel in moving {
        let from = cluster_dir.join(&rel);
        let to = backup.join(&rel);
        if let Err(err) = move_into_backup(&from, &to).await {
            tracing::warn!(cluster_id = cluster.id, entry = %rel, error = %err, "could not move an entry into the reset backup");
            report.not_moved.push(rel);
        }
    }

    polyio::create_dir_all(cluster_dir.join(ContentType::Mod.folder_name()))
        .await
        .ok();
    report.backup_dir = Some(backup);
    Ok(())
}

fn is_kept(cluster: &Cluster, name: &str, cover: Option<&str>) -> bool {
    name.starts_with('.')
        || KEPT_DIRS.contains(&name)
        || cover == Some(name)
        || (!cluster.is_isolated() && GLOBAL_TYPES.iter().any(|kind| kind.folder_name() == name))
}

async fn move_into_backup(from: &Path, to: &Path) -> Result<(), polyio::IOError> {
    if let Some(parent) = to.parent() {
        polyio::create_dir_all(parent).await?;
    }
    polyio::rename(from, to).await?;
    detach_symlinks(from, to).await;
    Ok(())
}

/// Content is symlinked in from the package store on Unix, and the store drops files nothing
/// links to on the next start, so the backup swaps each linked file for a copy of its own.
/// `original` is where `moved` used to live, to resolve relative links
async fn detach_symlinks(original: &Path, moved: &Path) {
    let mut pending = vec![(original.to_path_buf(), moved.to_path_buf())];
    while let Some((original, moved)) = pending.pop() {
        let Ok(meta) = polyio::symlink_metadata(&moved).await else {
            continue;
        };
        if meta.is_dir() {
            let Ok(mut entries) = polyio::read_dir(&moved).await else {
                continue;
            };
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name();
                pending.push((original.join(&name), moved.join(&name)));
            }
            continue;
        }
        if !meta.file_type().is_symlink() {
            continue;
        }

        let Ok(target) = polyio::read_link(&moved).await else {
            continue;
        };
        let target = match original.parent() {
            Some(parent) if target.is_relative() => parent.join(target),
            _ => target,
        };
        // Linked folders stay links, copying one could pull in a whole shared tree
        if !polyio::stat(&target).await.is_ok_and(|meta| meta.is_file()) {
            continue;
        }
        if let Err(err) = copy_over_link(&target, &moved).await {
            tracing::warn!(link = %moved.display(), error = %err, "could not keep a copy of a linked file in the reset backup");
        }
    }
}

async fn copy_over_link(target: &Path, link: &Path) -> Result<(), polyio::IOError> {
    let mut staging = link.as_os_str().to_owned();
    staging.push(".reset-copy");
    let staging = PathBuf::from(staging);

    polyio::copy(target, &staging).await?;
    polyio::remove_file(link).await?;
    polyio::rename(&staging, link).await
}
