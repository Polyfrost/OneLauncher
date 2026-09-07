use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use oneclient_common::domain::ContentType;

const PROBE_LEN: usize = 4096;

const MAX_PROBE_BYTES: u64 = 32 * 1024 * 1024;

const BULK_NAMES: [&[u8]; 4] = [b"cache", b"repo", b"backup", b"logs"];

const NEVER_PROBED: [&str; 8] = [
    "db", "sqlite", "sqlite3", "wal", "shm", "journal", "lock", "idx",
];

const SWEPT_DIRS: [&str; 1] = ["config"];

const SWEPT_CONTENT: [(ContentType, bool); 3] = [
    (ContentType::Mod, true),
    (ContentType::ResourcePack, false),
    (ContentType::Shader, false),
];

#[tracing::instrument(level = "debug")]
pub async fn clear_zeroed_files(game_dir: &Path) -> usize {
    let game_dir = game_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let mut cleared = sweep_dir(&game_dir, false);

        for dir in SWEPT_DIRS {
            cleared += sweep_dir(&game_dir.join(dir), true);
        }

        for (content_type, recurse) in SWEPT_CONTENT {
            cleared += sweep_dir(&game_dir.join(content_type.folder_name()), recurse);
        }

        if cleared > 0 {
            tracing::info!(
                cleared,
                game_dir = %game_dir.display(),
                "cleared zero-filled files; affected mods will regenerate defaults"
            );
        }

        cleared
    })
    .await
    .unwrap_or(0)
}

fn sweep_dir(root: &Path, recurse: bool) -> usize {
    let mut cleared = 0usize;
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            let path = entry.path();

            if names_bulk_data(&path) {
                continue;
            }

            if file_type.is_dir() {
                if recurse {
                    stack.push(path);
                }
                continue;
            }

            if !file_type.is_file() || !worth_probing(&path) || !is_zeroed(&path) {
                continue;
            }

            match fs::remove_file(&path) {
                Ok(()) => {
                    cleared += 1;
                    tracing::warn!(
                        file = %path.display(),
                        "removed zero-filled file left by an unclean shutdown"
                    );
                }
                Err(err) => tracing::warn!(
                    file = %path.display(),
                    error = %err,
                    "failed to remove zero-filled file"
                ),
            }
        }
    }

    cleared
}

fn names_bulk_data(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let name = name.as_bytes();

            BULK_NAMES.iter().any(|needle| {
                name.windows(needle.len())
                    .any(|window| window.eq_ignore_ascii_case(needle))
            })
        })
}

fn worth_probing(path: &Path) -> bool {
    !path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| NEVER_PROBED.iter().any(|skip| ext.eq_ignore_ascii_case(skip)))
}

fn is_zeroed(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };

    let mut buf = [0u8; PROBE_LEN];
    let mut read_so_far = 0u64;

    loop {
        let read = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return false,
        };

        if buf[..read].iter().any(|&b| b != 0) {
            return false;
        }

        read_so_far += read as u64;

        if read_so_far > MAX_PROBE_BYTES {
            return false;
        }
    }

    read_so_far > 0
}
