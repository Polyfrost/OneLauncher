//! Per-world snapshots that live next to the instance.
//!
//! A world is zipped into `backups/worlds/<world>/<timestamp>-<reason>.zip`.
//! Before a launch the launcher backs up worlds that changed since the last
//! snapshot, so a crash or a bad mod cannot destroy hours of building.

use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use oneclient_cluster::Cluster;

const SAVES_DIR: &str = "saves";
const LEVEL_DAT: &str = "level.dat";
const SESSION_LOCK: &str = "session.lock";
const STATE_FILE: &str = "state.json";
const TEMP_PREFIX: &str = ".tmp-";

/// A world backup can be large; never make the player wait forever before a
/// launch. The partial archive is removed by the next backup.
const BACKUP_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Error)]
pub enum WorldBackupError {
    #[error("not a plain world name: {0}")]
    InvalidName(String),
    #[error("world '{0}' was not found")]
    NotFound(String),
    #[error("world backup failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not read or write the world archive: {0}")]
    Zip(String),
    #[error("could not resolve the instance folder: {0}")]
    Cluster(String),
}

pub type WorldBackupResult<T> = Result<T, WorldBackupError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldBackupInfo {
    pub world: String,
    pub file_name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldBackupReport {
    pub created: Vec<WorldBackupInfo>,
    pub skipped: usize,
    pub failures: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct WorldBackupState {
    signature: u64,
}

fn plain_name(name: &str) -> WorldBackupResult<&str> {
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(name),
        _ => Err(WorldBackupError::InvalidName(name.to_string())),
    }
}

fn saves_dir(cluster: &Cluster) -> WorldBackupResult<PathBuf> {
    cluster
        .game_dir()
        .map(|dir| dir.join(SAVES_DIR))
        .map_err(|err| WorldBackupError::Cluster(err.to_string()))
}

fn world_dir(cluster: &Cluster, world: &str) -> WorldBackupResult<PathBuf> {
    Ok(saves_dir(cluster)?.join(plain_name(world)?))
}

fn backups_root(cluster: &Cluster) -> WorldBackupResult<PathBuf> {
    cluster
        .dir()
        .map(|dir| dir.join("backups").join("worlds"))
        .map_err(|err| WorldBackupError::Cluster(err.to_string()))
}

fn world_backups_dir(cluster: &Cluster, world: &str) -> WorldBackupResult<PathBuf> {
    Ok(backups_root(cluster)?.join(plain_name(world)?))
}

fn stamp() -> String {
    Utc::now().format("%Y%m%d-%H%M%S-%9f").to_string()
}

fn safe_reason(reason: &str) -> String {
    let cleaned: String = reason
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(24)
        .collect();
    if cleaned.is_empty() {
        "manual".to_string()
    } else {
        cleaned
    }
}

fn modified_seconds(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

async fn world_signature(world_dir: &Path) -> std::io::Result<u64> {
    let mut items: Vec<(String, u64, u64)> = Vec::new();
    let mut stack = vec![(world_dir.to_path_buf(), String::new())];

    while let Some((current, prefix)) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&current).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == SESSION_LOCK {
                continue;
            }
            let relative = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                stack.push((entry.path(), relative));
            } else if file_type.is_file() {
                let meta = entry.metadata().await?;
                items.push((relative, meta.len(), modified_seconds(&meta)));
            }
        }
    }

    items.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    items.hash(&mut hasher);
    Ok(hasher.finish())
}

async fn write_zip(world_dir: &Path, dest: &Path) -> WorldBackupResult<u64> {
    let file = tokio::fs::File::create(dest).await?;
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    let mut stack = vec![(world_dir.to_path_buf(), String::new())];

    while let Some((current, prefix)) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&current).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == SESSION_LOCK {
                continue;
            }
            let relative = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                stack.push((entry.path(), relative));
            } else if file_type.is_file() {
                let bytes = tokio::fs::read(entry.path()).await?;
                let builder = async_zip::ZipEntryBuilder::new(
                    relative.into(),
                    async_zip::Compression::Deflate,
                );
                writer
                    .write_entry_whole(builder, &bytes)
                    .await
                    .map_err(|err| WorldBackupError::Zip(err.to_string()))?;
            }
        }
    }

    writer
        .close()
        .await
        .map_err(|err| WorldBackupError::Zip(err.to_string()))?;
    Ok(tokio::fs::metadata(dest).await?.len())
}

async fn read_state(path: &Path) -> Option<WorldBackupState> {
    let bytes = tokio::fs::read(path).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

async fn write_state(path: &Path, signature: u64) -> std::io::Result<()> {
    let state = WorldBackupState { signature };
    let bytes = serde_json::to_vec(&state).map_err(std::io::Error::other)?;
    tokio::fs::write(path, bytes).await
}

async fn cleanup_temp(dir: &Path) {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(TEMP_PREFIX) {
            let _ = tokio::fs::remove_file(entry.path()).await;
        } else if name.starts_with(".restore-") || name.starts_with(".previous-") {
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

async fn list_backups_at(dir: &Path, world: &str) -> WorldBackupResult<Vec<WorldBackupInfo>> {
    let mut out = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return Ok(out);
    };

    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.starts_with('.') || !file_name.ends_with(".zip") {
            continue;
        }
        let meta = entry.metadata().await?;
        if !meta.is_file() {
            continue;
        }
        let created_at = meta
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        out.push(WorldBackupInfo {
            world: world.to_string(),
            file_name,
            path: entry.path(),
            created_at,
            size_bytes: meta.len(),
        });
    }

    out.sort_by_key(|backup| std::cmp::Reverse(backup.created_at));
    Ok(out)
}

async fn prune(dir: &Path, world: &str, retention: usize) -> WorldBackupResult<()> {
    let mut backups = list_backups_at(dir, world).await?;
    let keep = retention.max(1);
    if backups.len() <= keep {
        return Ok(());
    }
    for backup in backups.drain(keep..) {
        if let Err(err) = tokio::fs::remove_file(&backup.path).await {
            tracing::warn!(path = %backup.path.display(), "could not prune a world backup: {err}");
        }
    }
    Ok(())
}

async fn backup_at(
    world_dir: &Path,
    backups: &Path,
    world: &str,
    reason: &str,
    retention: usize,
    skip_unchanged: bool,
) -> WorldBackupResult<Option<WorldBackupInfo>> {
    if !world_dir.is_dir() {
        return Err(WorldBackupError::NotFound(world.to_string()));
    }
    tokio::fs::create_dir_all(backups).await?;
    cleanup_temp(backups).await;

    let signature = world_signature(world_dir).await?;
    let state_path = backups.join(STATE_FILE);
    if skip_unchanged
        && read_state(&state_path)
            .await
            .is_some_and(|s| s.signature == signature)
    {
        return Ok(None);
    }

    let file_name = format!("{}-{}.zip", stamp(), safe_reason(reason));
    let final_path = backups.join(&file_name);
    let temp_path = backups.join(format!("{TEMP_PREFIX}{file_name}"));
    let size_bytes = match write_zip(world_dir, &temp_path).await {
        Ok(size) => size,
        Err(err) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(err);
        }
    };
    if let Err(err) = tokio::fs::rename(&temp_path, &final_path).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(err.into());
    }

    if let Err(err) = write_state(&state_path, signature).await {
        tracing::warn!(path = %state_path.display(), "could not record the world backup state: {err}");
    }
    prune(backups, world, retention).await?;

    let created_at = tokio::fs::metadata(&final_path)
        .await
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(Utc::now);

    Ok(Some(WorldBackupInfo {
        world: world.to_string(),
        file_name,
        path: final_path,
        created_at,
        size_bytes,
    }))
}

/// Backs up a single world. `Ok(None)` means the world has not changed since
/// the last snapshot and `skip_unchanged` was set.
pub async fn backup_world(
    cluster: &Cluster,
    world: &str,
    reason: &str,
    retention: usize,
    skip_unchanged: bool,
) -> WorldBackupResult<Option<WorldBackupInfo>> {
    let dir = world_dir(cluster, world)?;
    let backups = world_backups_dir(cluster, world)?;
    backup_at(&dir, &backups, world, reason, retention, skip_unchanged).await
}

/// Backs up every world of the instance. Used before a launch.
pub async fn backup_worlds(
    cluster: &Cluster,
    reason: &str,
    retention: usize,
    skip_unchanged: bool,
) -> WorldBackupReport {
    let mut report = WorldBackupReport::default();
    let saves = match saves_dir(cluster) {
        Ok(dir) => dir,
        Err(err) => {
            report.failures.push(err.to_string());
            return report;
        }
    };

    let Ok(mut entries) = tokio::fs::read_dir(&saves).await else {
        return report;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        if !file_type.is_dir() || !entry.path().join(LEVEL_DAT).is_file() {
            continue;
        }
        let world = entry.file_name().to_string_lossy().to_string();
        let backups = match world_backups_dir(cluster, &world) {
            Ok(dir) => dir,
            Err(err) => {
                report.failures.push(format!("{world}: {err}"));
                continue;
            }
        };
        match backup_at(
            &entry.path(),
            &backups,
            &world,
            reason,
            retention,
            skip_unchanged,
        )
        .await
        {
            Ok(Some(info)) => report.created.push(info),
            Ok(None) => report.skipped += 1,
            Err(err) => report.failures.push(format!("{world}: {err}")),
        }
    }

    report
}

pub async fn list_world_backups(
    cluster: &Cluster,
    world: &str,
) -> WorldBackupResult<Vec<WorldBackupInfo>> {
    let dir = world_backups_dir(cluster, world)?;
    list_backups_at(&dir, world).await
}

/// Replaces the world with the selected snapshot. The current world is kept as
/// a fresh backup first, so a restore can itself be undone.
pub async fn restore_world_backup(
    cluster: &Cluster,
    world: &str,
    file_name: &str,
) -> WorldBackupResult<()> {
    let world_name = plain_name(world)?;
    let world_dir = world_dir(cluster, world_name)?;
    let backups = world_backups_dir(cluster, world_name)?;
    restore_at(&world_dir, &backups, world_name, file_name).await
}

async fn restore_at(
    world_dir: &Path,
    backups: &Path,
    world: &str,
    file_name: &str,
) -> WorldBackupResult<()> {
    let backup_name = plain_name(file_name)?;
    if !backup_name.ends_with(".zip") {
        return Err(WorldBackupError::InvalidName(file_name.to_string()));
    }

    let backup_path = backups.join(backup_name);
    if !backup_path.is_file() {
        return Err(WorldBackupError::NotFound(file_name.to_string()));
    }

    // Keep the current state before touching it.
    if world_dir.is_dir() {
        let _ = backup_at(world_dir, backups, world, "before-restore", 10, false).await;
    }

    tokio::fs::create_dir_all(backups).await?;
    let temp = backups.join(format!(".restore-{}-{}.tmp", stamp(), std::process::id()));
    let _ = tokio::fs::remove_dir_all(&temp).await;

    if let Err(err) = polyio::extract_zip(&backup_path, &temp).await {
        let _ = tokio::fs::remove_dir_all(&temp).await;
        return Err(WorldBackupError::Zip(err.to_string()));
    }
    if !temp.join(LEVEL_DAT).is_file() {
        let _ = tokio::fs::remove_dir_all(&temp).await;
        return Err(WorldBackupError::Zip(
            "the snapshot does not contain a level.dat".to_string(),
        ));
    }

    let previous = backups.join(format!(".previous-{}-{}.tmp", stamp(), std::process::id()));
    let _ = tokio::fs::remove_dir_all(&previous).await;
    if world_dir.is_dir() {
        tokio::fs::rename(world_dir, &previous).await?;
    }

    match tokio::fs::rename(&temp, world_dir).await {
        Ok(()) => {
            let _ = tokio::fs::remove_dir_all(&previous).await;
            if let Ok(signature) = world_signature(world_dir).await {
                let _ = write_state(&backups.join(STATE_FILE), signature).await;
            }
            Ok(())
        }
        Err(err) => {
            if previous.is_dir() {
                let _ = tokio::fs::rename(&previous, world_dir).await;
            }
            let _ = tokio::fs::remove_dir_all(&temp).await;
            Err(err.into())
        }
    }
}

/// Called by the launch path. A backup failure never blocks the launch; the
/// caller reports it.
pub async fn backup_before_launch(cluster: &Cluster, retention: usize) -> WorldBackupReport {
    match tokio::time::timeout(
        BACKUP_TIMEOUT,
        backup_worlds(cluster, "before-launch", retention.max(1), true),
    )
    .await
    {
        Ok(report) => report,
        Err(_) => WorldBackupReport {
            failures: vec!["the backup took too long and was skipped".to_string()],
            ..WorldBackupReport::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("remixclient-world-backup-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    async fn seed_world(world: &Path, seed: &[u8]) {
        tokio::fs::create_dir_all(world.join("region"))
            .await
            .unwrap();
        tokio::fs::write(world.join(LEVEL_DAT), b"level")
            .await
            .unwrap();
        tokio::fs::write(world.join("region/r.0.0.mca"), seed)
            .await
            .unwrap();
        tokio::fs::write(world.join(SESSION_LOCK), b"lock")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_backup_round_trips_the_world() {
        let root = temp_root();
        let world = root.join("saves/World");
        let backups = root.join("backups/worlds/World");
        seed_world(&world, b"original").await;

        let info = backup_at(&world, &backups, "World", "test", 5, false)
            .await
            .unwrap()
            .expect("a backup should be created");
        assert!(info.path.is_file());
        assert!(info.size_bytes > 0);

        tokio::fs::write(world.join("region/r.0.0.mca"), b"changed")
            .await
            .unwrap();
        restore_at(&world, &backups, "World", &info.file_name)
            .await
            .unwrap();

        assert_eq!(
            tokio::fs::read(world.join("region/r.0.0.mca"))
                .await
                .unwrap(),
            b"original"
        );

        let _ = tokio::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn an_unchanged_world_is_skipped() {
        let root = temp_root();
        let world = root.join("saves/World");
        let backups = root.join("backups/worlds/World");
        seed_world(&world, b"original").await;

        assert!(
            backup_at(&world, &backups, "World", "test", 5, true)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            backup_at(&world, &backups, "World", "test", 5, true)
                .await
                .unwrap()
                .is_none()
        );

        let _ = tokio::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn retention_keeps_the_newest_backups() {
        let root = temp_root();
        let world = root.join("saves/World");
        let backups = root.join("backups/worlds/World");
        seed_world(&world, b"one").await;

        for i in 0..4 {
            tokio::fs::write(world.join("region/r.0.0.mca"), [i as u8])
                .await
                .unwrap();
            backup_at(&world, &backups, "World", "test", 2, false)
                .await
                .unwrap()
                .expect("a changed world is backed up");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        let listed = list_backups_at(&backups, "World").await.unwrap();
        assert_eq!(listed.len(), 2);

        let _ = tokio::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn a_path_in_a_world_name_is_refused() {
        assert!(plain_name("../escape").is_err());
        assert!(plain_name("a/b").is_err());
        assert!(plain_name("World").is_ok());
    }
}
