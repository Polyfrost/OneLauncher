use std::path::{Path, PathBuf};

use super::{is_safe_relative_path, tracked_content_type};
use crate::error::ContentResult;

pub const MAX_SETTING_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_SETTINGS_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_SETTING_FILES: usize = 4000;

const SETTING_DIRS: &[&str] = &["config", "oneconfig"];
const SETTING_FILES: &[&str] = &["options.txt", "optionsof.txt", "optionsshaders.txt"];
const SKIPPED_DIRS: &[&str] = &[
    "oneconfig/backup",
    "oneconfig/translations",
    "oneconfig/sounds",
];
const SKIPPED_EXTENSIONS: &[&str] = &[
    "jar", "zip", "gz", "7z", "rar", "log", "tmp", "lock", "bak", "old", "dll", "so", "dylib",
    "exe", "db", "sqlite", "mca", "dat_old",
];
const PRIVATE_MARKERS: &[&str] = &[
    "token",
    "secret",
    "password",
    "passwd",
    "credential",
    "session",
    "auth",
    "apikey",
    "api_key",
    "api-key",
    "account",
    "cookie",
    "private",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingFile {
    pub path: String,
    pub source: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsScan {
    pub files: Vec<SettingFile>,
    pub skipped: Vec<String>,
    pub bytes: u64,
}

#[must_use]
pub fn is_setting_path(path: &str) -> bool {
    let path = path.replace('\\', "/");
    if !is_safe_relative_path(&path) || tracked_content_type(&path).is_some() {
        return false;
    }
    let lower = path.to_ascii_lowercase();
    if SETTING_FILES.contains(&lower.as_str()) {
        return true;
    }
    let Some((root, _)) = lower.split_once('/') else {
        return false;
    };
    if !SETTING_DIRS.contains(&root) {
        return false;
    }
    if SKIPPED_DIRS
        .iter()
        .any(|dir| lower == *dir || lower.starts_with(&format!("{dir}/")))
    {
        return false;
    }
    if lower.split('/').any(|segment| segment.starts_with('.')) {
        return false;
    }
    let name = lower.rsplit('/').next().unwrap_or_default();
    if PRIVATE_MARKERS.iter().any(|marker| name.contains(marker)) {
        return false;
    }
    !name
        .rsplit_once('.')
        .is_some_and(|(_, ext)| SKIPPED_EXTENSIONS.contains(&ext))
}

#[tracing::instrument(level = "debug")]
pub async fn collect_settings(game_dir: &Path) -> ContentResult<SettingsScan> {
    let game_dir = game_dir.to_path_buf();
    let scan = tokio::task::spawn_blocking(move || scan_settings(&game_dir))
        .await
        .map_err(|err| crate::error::ContentError::InvalidData {
            reason: format!("could not read the instance settings: {err}"),
        })?;
    tracing::debug!(
        files = scan.files.len(),
        skipped = scan.skipped.len(),
        bytes = scan.bytes,
        "collected instance settings"
    );
    Ok(scan)
}

fn scan_settings(game_dir: &Path) -> SettingsScan {
    let mut found = Vec::new();
    for name in SETTING_FILES {
        found.push((name.to_string(), game_dir.join(name)));
    }
    for dir in SETTING_DIRS {
        walk(&game_dir.join(dir), dir, &mut found);
    }

    let mut scan = SettingsScan::default();
    for (rel, source) in found {
        let Ok(meta) = std::fs::symlink_metadata(&source) else {
            continue;
        };
        if !meta.is_file() || !is_setting_path(&rel) {
            continue;
        }
        let size = meta.len();
        if size > MAX_SETTING_FILE_BYTES
            || scan.bytes + size > MAX_SETTINGS_BYTES
            || scan.files.len() >= MAX_SETTING_FILES
        {
            scan.skipped.push(rel);
            continue;
        }
        scan.bytes += size;
        scan.files.push(SettingFile {
            path: rel,
            source,
            size,
        });
    }
    scan.files.sort_by(|a, b| a.path.cmp(&b.path));
    scan
}

fn walk(dir: &Path, rel: &str, found: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let child = format!("{rel}/{name}");
        if kind.is_dir() {
            if is_setting_dir(&child) {
                walk(&entry.path(), &child, found);
            }
        } else if kind.is_file() {
            found.push((child, entry.path()));
        }
    }
}

fn is_setting_dir(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    !lower
        .rsplit('/')
        .next()
        .is_some_and(|name| name.starts_with('.'))
        && !SKIPPED_DIRS.contains(&lower.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_and_oneconfig_files_count_as_settings() {
        assert!(is_setting_path("config/sodium-options.json"));
        assert!(is_setting_path(
            "oneconfig/profiles/default/polysprint.json"
        ));
        assert!(is_setting_path("options.txt"));
        assert!(is_setting_path("config\\iris.properties"));
    }

    #[test]
    fn content_caches_and_private_files_are_left_out() {
        assert!(!is_setting_path("mods/sodium.jar"));
        assert!(!is_setting_path("saves/world/level.dat"));
        assert!(!is_setting_path("config/hytils/api_key.txt"));
        assert!(!is_setting_path("config/discord-token.json"));
        assert!(!is_setting_path("oneconfig/backup/old.json"));
        assert!(!is_setting_path("oneconfig/.skiko/skiko.dll"));
        assert!(!is_setting_path("config/somemod/library.jar"));
        assert!(!is_setting_path("config/../options.txt"));
        assert!(!is_setting_path("servers.dat"));
    }

    #[tokio::test]
    async fn a_scan_keeps_settings_and_respects_the_size_limit() {
        let root = std::env::temp_dir().join(format!("oc-settings-{}", std::process::id()));
        std::fs::create_dir_all(root.join("config/sub")).unwrap();
        std::fs::create_dir_all(root.join("oneconfig/backup")).unwrap();
        std::fs::write(root.join("options.txt"), b"fov:90").unwrap();
        std::fs::write(root.join("config/sub/a.json"), b"{}").unwrap();
        std::fs::write(root.join("config/session.json"), b"{}").unwrap();
        std::fs::write(root.join("oneconfig/backup/b.json"), b"{}").unwrap();
        std::fs::write(
            root.join("config/huge.json"),
            vec![b' '; (MAX_SETTING_FILE_BYTES + 1) as usize],
        )
        .unwrap();

        let scan = collect_settings(&root).await.unwrap();
        let paths: Vec<&str> = scan.files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, vec!["config/sub/a.json", "options.txt"]);
        assert_eq!(scan.skipped, vec!["config/huge.json".to_string()]);
        std::fs::remove_dir_all(&root).ok();
    }
}
