use std::path::{Component, Path, PathBuf};

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

const DIMENSION_SUFFIXES: [&str; 3] = ["", "_nether", "_the_end"];

pub async fn world_folders(dir: &Path) -> LauncherResult<Vec<String>> {
    let name = world_name(dir).await;
    checked_world(&name)?;
    Ok(DIMENSION_SUFFIXES
        .iter()
        .map(|suffix| format!("{name}{suffix}"))
        .collect())
}

const RESERVED_FOLDERS: [&str; 9] = [
    BACKUPS_DIR,
    "mods",
    "plugins",
    "config",
    "libraries",
    "logs",
    "cache",
    "versions",
    "crash-reports",
];

fn checked_world(name: &str) -> LauncherResult<()> {
    let mut parts = Path::new(name).components();
    let plain = matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(_)), None)
    );
    let reserved = name.starts_with('.')
        || RESERVED_FOLDERS
            .iter()
            .any(|folder| folder.eq_ignore_ascii_case(name));
    if plain && !reserved {
        Ok(())
    } else {
        Err(LauncherError::Server(format!(
            "level-name {name:?} must be a plain folder name the server does not use for anything else"
        )))
    }
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
    let folders = world_folders(dir).await?;
    if !tokio::fs::try_exists(dir.join(&folders[0]))
        .await
        .unwrap_or(false)
    {
        return Err(LauncherError::Server(
            "there is no world to back up yet".into(),
        ));
    }
    let sources: Vec<PathBuf> = folders.iter().map(|f| dir.join(f)).collect();
    let files = tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        for source in sources.iter().filter(|s| s.is_dir()) {
            collect(source, &mut files)?;
        }
        Ok::<_, std::io::Error>(files)
    })
    .await
    .map_err(|e| LauncherError::Server(e.to_string()))??;

    let backups = dir.join(BACKUPS_DIR);
    tokio::fs::create_dir_all(&backups).await?;
    let (target, tmp, file) = reserve_target(
        &backups,
        &Local::now().format("%Y-%m-%d_%H-%M-%S").to_string(),
    )
    .await?;
    if let Err(err) = write_zip(dir, files, file).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(err);
    }
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

async fn reserve_target(
    backups: &Path,
    stamp: &str,
) -> std::io::Result<(PathBuf, PathBuf, tokio::fs::File)> {
    let mut n = 1;
    loop {
        let name = if n == 1 {
            stamp.to_string()
        } else {
            format!("{stamp}-{n}")
        };
        n += 1;
        let target = backups.join(format!("{name}.zip"));
        if tokio::fs::try_exists(&target).await? {
            continue;
        }
        let tmp = backups.join(format!("{name}.zip.tmp"));
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .await
        {
            Ok(file) => return Ok((target, tmp, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
    }
}

async fn write_zip(dir: &Path, files: Vec<PathBuf>, file: tokio::fs::File) -> LauncherResult<()> {
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    for path in files {
        // The server holds session.lock open on Windows, and it is meaningless in a copy
        if path.file_name().is_some_and(|n| n == "session.lock") {
            continue;
        }
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(LauncherError::Server(format!(
                    "could not read {}: {err}",
                    path.display()
                )));
            }
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
    Ok(())
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// Replaces the world with the one in the backup, only while the server is stopped
pub async fn restore_backup(dir: &Path, backup: &Path, keep: u32) -> LauncherResult<()> {
    let staging = dir.join(".restore");
    let _ = tokio::fs::remove_dir_all(&staging).await;
    let result = async {
        polyio::extract_zip(backup, &staging).await?;
        let current = dir.join(&world_folders(dir).await?[0]);
        if tokio::fs::try_exists(&current).await? {
            create_backup(dir, keep).await?;
        }
        swap_in_backup(dir, &staging).await
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result
}

fn backup_base(mut folders: Vec<String>) -> Option<String> {
    folders.sort();
    folders
        .iter()
        .find(|n| !n.ends_with("_nether") && !n.ends_with("_the_end"))
        .or(folders.first())
        .cloned()
}

async fn swap_in_backup(dir: &Path, staging: &Path) -> LauncherResult<()> {
    let mut found = Vec::new();
    let mut entries = tokio::fs::read_dir(staging).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            found.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    let base = backup_base(found)
        .ok_or_else(|| LauncherError::Server("the backup has no world folder in it".into()))?;

    let mut moves = Vec::new();
    for (suffix, target) in DIMENSION_SUFFIXES.iter().zip(world_folders(dir).await?) {
        let from = staging.join(format!("{base}{suffix}"));
        if tokio::fs::try_exists(&from).await.unwrap_or(false) {
            moves.push((from, dir.join(&target), dir.join(format!(".{target}-replaced"))));
        }
    }

    let mut set_aside = Vec::new();
    let mut moved_in = Vec::new();
    let result = async {
        for (_, to, aside) in &moves {
            if tokio::fs::try_exists(to).await? {
                let _ = tokio::fs::remove_dir_all(aside).await;
                tokio::fs::rename(to, aside).await?;
                set_aside.push((aside.clone(), to.clone()));
            }
        }
        for (from, to, _) in &moves {
            tokio::fs::rename(from, to).await?;
            moved_in.push(to.clone());
        }
        Ok::<_, std::io::Error>(())
    }
    .await;

    if let Err(err) = result {
        for to in &moved_in {
            let _ = tokio::fs::remove_dir_all(to).await;
        }
        for (aside, to) in &set_aside {
            let _ = tokio::fs::rename(aside, to).await;
        }
        return Err(err.into());
    }
    for (aside, _) in &set_aside {
        let _ = tokio::fs::remove_dir_all(aside).await;
    }
    Ok(())
}

/// Backs the world up then deletes it so the next start generates a new one
pub async fn reset_world(dir: &Path, keep: u32) -> LauncherResult<()> {
    let folders = world_folders(dir).await?;
    if !tokio::fs::try_exists(dir.join(&folders[0]))
        .await
        .unwrap_or(false)
    {
        return Ok(());
    }
    create_backup(dir, keep).await?;
    for folder in &folders {
        let path = dir.join(folder);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            tokio::fs::remove_dir_all(&path).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        backup_base, checked_world, create_backup, list_backups, reserve_target, restore_backup,
    };

    #[tokio::test]
    async fn backups_in_the_same_second_get_their_own_file() {
        let tmp = polyio::tempdir().await.expect("dir");
        let dir = tmp.dir_path();
        let stamp = "2026-11-15_13-00-00";
        let (first, first_tmp, _) = reserve_target(dir, stamp).await.unwrap();
        assert_eq!(first, dir.join(format!("{stamp}.zip")));
        let (second, second_tmp, _) = reserve_target(dir, stamp).await.unwrap();
        assert_eq!(second, dir.join(format!("{stamp}-2.zip")));
        assert_ne!(first_tmp, second_tmp);
        polyio::write(&dir.join(format!("{stamp}-3.zip")), b"zip")
            .await
            .unwrap();
        let (third, ..) = reserve_target(dir, stamp).await.unwrap();
        assert_eq!(third, dir.join(format!("{stamp}-4.zip")));
    }

    #[tokio::test]
    async fn restore_brings_the_world_back_and_backs_up_the_replaced_one() {
        let tmp = polyio::tempdir().await.expect("dir");
        let dir = tmp.dir_path();
        let level = dir.join("world").join("level.dat");
        tokio::fs::create_dir_all(dir.join("world")).await.unwrap();
        tokio::fs::write(&level, b"original").await.unwrap();
        let backup = create_backup(dir, 10).await.unwrap();

        tokio::fs::write(&level, b"modified").await.unwrap();
        restore_backup(dir, &backup, 10).await.unwrap();
        assert_eq!(tokio::fs::read(&level).await.unwrap(), b"original");

        let backups = list_backups(dir).await;
        assert_eq!(backups.len(), 2);
        let safety = backups.iter().find(|b| b.path != backup).unwrap();
        let check = dir.join("check");
        polyio::extract_zip(&safety.path, &check).await.unwrap();
        assert_eq!(
            tokio::fs::read(check.join("world").join("level.dat"))
                .await
                .unwrap(),
            b"modified"
        );
    }

    #[test]
    fn level_name_must_stay_inside_the_server_folder() {
        assert!(checked_world("world").is_ok());
        assert!(checked_world("my world").is_ok());
        assert!(checked_world("..").is_err());
        assert!(checked_world("a/b").is_err());
        assert!(checked_world("/etc").is_err());
        assert!(checked_world("").is_err());
        assert!(checked_world("backups").is_err());
        assert!(checked_world("Mods").is_err());
        assert!(checked_world("plugins").is_err());
        assert!(checked_world(".restore").is_err());
        assert!(checked_world(".hidden").is_err());
        assert!(checked_world("world_backups").is_ok());
    }

    #[test]
    fn restore_finds_the_overworld_beside_paper_dimensions() {
        let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            backup_base(names(&["world_the_end", "world_nether", "world"])).as_deref(),
            Some("world")
        );
        assert_eq!(backup_base(names(&["old"])).as_deref(), Some("old"));
        assert_eq!(backup_base(Vec::new()), None);
    }
}
