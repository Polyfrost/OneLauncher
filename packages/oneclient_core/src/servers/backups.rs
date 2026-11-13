use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

use super::properties::Properties;
use super::store::BACKUPS_DIR;
use crate::{LauncherError, LauncherResult};

#[derive(Debug, Clone, PartialEq)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub created: DateTime<Local>,
}

pub async fn world_name(dir: &Path) -> String {
    Properties::read(dir).await.get_or("level-name", "world")
}

pub async fn list_backups(dir: &Path) -> Vec<BackupInfo> {
    let mut out = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir.join(BACKUPS_DIR)).await else {
        return out;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "zip") {
            continue;
        }
        let Ok(meta) = entry.metadata().await else {
            continue;
        };
        out.push(BackupInfo {
            name: entry
                .file_name()
                .to_string_lossy()
                .trim_end_matches(".zip")
                .to_string(),
            size: meta.len(),
            created: meta
                .modified()
                .map(DateTime::<Local>::from)
                .unwrap_or_else(|_| Local::now()),
            path,
        });
    }
    out.sort_by_key(|b| std::cmp::Reverse(b.created));
    out
}

/// Zips the world folder, then drops the oldest backups past `keep`
pub async fn create_backup(dir: &Path, keep: u32) -> LauncherResult<PathBuf> {
    let world = world_name(dir).await;
    let source = dir.join(&world);
    if !tokio::fs::try_exists(&source).await.unwrap_or(false) {
        return Err(LauncherError::Server(
            "there is no world to back up yet".into(),
        ));
    }
    let backups = dir.join(BACKUPS_DIR);
    tokio::fs::create_dir_all(&backups).await?;
    let target = backups.join(format!("{}.zip", Local::now().format("%Y-%m-%d_%H-%M-%S")));
    let tmp = target.with_extension("zip.tmp");

    let files = {
        let source = source.clone();
        tokio::task::spawn_blocking(move || {
            let mut files = Vec::new();
            collect(&source, &mut files);
            files
        })
        .await
        .unwrap_or_default()
    };

    let file = tokio::fs::File::create(&tmp).await?;
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    for path in files {
        // The server holds session.lock open on Windows, and it is meaningless in a copy
        if path.file_name().is_some_and(|n| n == "session.lock") {
            continue;
        }
        let Ok(bytes) = tokio::fs::read(&path).await else {
            continue;
        };
        let relative = path
            .strip_prefix(dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let builder =
            async_zip::ZipEntryBuilder::new(relative.into(), async_zip::Compression::Deflate);
        writer
            .write_entry_whole(builder, &bytes)
            .await
            .map_err(|e| LauncherError::Server(e.to_string()))?;
    }
    writer
        .close()
        .await
        .map_err(|e| LauncherError::Server(e.to_string()))?;
    tokio::fs::rename(&tmp, &target).await?;

    for old in list_backups(dir)
        .await
        .into_iter()
        .skip(keep.max(1) as usize)
    {
        let _ = tokio::fs::remove_file(old.path).await;
    }
    Ok(target)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Replaces the world with the one in the backup, only while the server is stopped
pub async fn restore_backup(dir: &Path, backup: &Path) -> LauncherResult<()> {
    let world = dir.join(world_name(dir).await);
    if tokio::fs::try_exists(&world).await.unwrap_or(false) {
        tokio::fs::remove_dir_all(&world).await?;
    }
    polyio::extract_zip(backup, dir).await?;
    Ok(())
}

/// Backs the world up then deletes it so the next start generates a new one
pub async fn reset_world(dir: &Path, keep: u32) -> LauncherResult<()> {
    let world = dir.join(world_name(dir).await);
    if !tokio::fs::try_exists(&world).await.unwrap_or(false) {
        return Ok(());
    }
    create_backup(dir, keep).await?;
    tokio::fs::remove_dir_all(&world).await?;
    Ok(())
}
