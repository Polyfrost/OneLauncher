use std::path::{Path, PathBuf};

use tokio::io::AsyncReadExt;

const PROBE_LEN: usize = 8 * 1024;

#[tracing::instrument(level = "debug")]
pub async fn clear_zeroed_files(game_dir: &Path) -> usize {
    let mut cleared = 0usize;
    let mut stack: Vec<PathBuf> = vec![game_dir.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(mut entries) = polyio::read_dir(&dir).await else {
            continue;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            let path = entry.path();

            if file_type.is_dir() {
                stack.push(path);
                continue;
            }

            if !file_type.is_file() || !is_zeroed(&path).await {
                continue;
            }

            match polyio::remove_file(&path).await {
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

    if cleared > 0 {
        tracing::info!(
            cleared,
            game_dir = %game_dir.display(),
            "cleared zero-filled files; affected mods will regenerate defaults"
        );
    }

    cleared
}

async fn is_zeroed(path: &Path) -> bool {
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return false;
    };

    let mut buf = vec![0u8; PROBE_LEN];
    let mut seen_any = false;

    loop {
        let read = match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return false,
        };

        if buf[..read].iter().any(|&b| b != 0) {
            return false;
        }

        seen_any = true;
    }

    seen_any
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[tokio::test]
    async fn clears_only_non_empty_all_nul_files() {
        let dir = polyio::testing::ScratchDir::new("heal-zeroed");
        let root = dir.join("game");

        let zeroed = root.join("config").join("sodium-options.json");
        let partly = root.join("config").join("half.json");
        let good = root.join("options.txt");
        let empty = root.join(".dedicated_directory");
        let nested = root.join("config").join("deep").join("nested.toml");

        write(&zeroed, &[0u8; 818]);
        write(&partly, &[&[0u8; 4096][..], b"{}"].concat());
        write(&good, b"fov:70\n");
        write(&empty, b"");
        write(&nested, &[0u8; 3]);

        assert_eq!(clear_zeroed_files(&root).await, 2);

        assert!(!zeroed.exists());
        assert!(!nested.exists(), "recursion must reach nested directories");
        assert!(partly.exists());
        assert!(good.exists());
        assert!(empty.exists(), "empty marker files are not corruption");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn does_not_descend_linked_directories() {
        let dir = polyio::testing::ScratchDir::new("heal-symlink");
        let root = dir.join("game");
        let outside = dir.join("launcher-logs");

        let victim = outside.join("latest.log");
        write(&victim, &[0u8; 64]);
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("logs")).unwrap();

        assert_eq!(clear_zeroed_files(&root).await, 0);
        assert!(victim.exists());
    }

    #[tokio::test]
    async fn missing_directory_is_not_an_error() {
        let dir = polyio::testing::ScratchDir::new("heal-missing");
        assert_eq!(clear_zeroed_files(&dir.join("nope")).await, 0);
    }
}
