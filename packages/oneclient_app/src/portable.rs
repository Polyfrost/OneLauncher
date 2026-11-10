//! Portable mode keeps every launcher file next to the executable so the whole
//! app can live on a USB stick or in a single folder.
//!
//! It is enabled by `--portable`, `REMIXCLIENT_PORTABLE=1`, or an empty
//! `portable.txt` / `remixclient.portable` marker next to the executable.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ACTIVE: OnceLock<bool> = OnceLock::new();

const MARKERS: &[&str] = &["portable.txt", "remixclient.portable"];

const DATA_FOLDER: &str = "RemixClient-data";

/// Applies portable mode before any launcher path is read. Returns whether the
/// launcher is now running portable.
pub fn init(cli_flag: bool) -> bool {
    if !requested(cli_flag) {
        let _ = ACTIVE.set(false);
        return false;
    }

    let Some(dir) = data_dir() else {
        let _ = ACTIVE.set(false);
        return false;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        // The executable lives somewhere read-only (Program Files, a mounted
        // image). Fall back to the normal data folder instead of failing later.
        let _ = ACTIVE.set(false);
        return false;
    }

    oneclient_common::paths::set_launcher_dir(dir);
    let _ = ACTIVE.set(true);

    // Remember the choice so a plain double-click stays portable next time.
    if cli_flag {
        let _ = write_marker();
    }

    true
}

pub fn active() -> bool {
    *ACTIVE.get().unwrap_or(&false)
}

fn requested(cli_flag: bool) -> bool {
    cli_flag || env_requested() || marker_found()
}

fn env_requested() -> bool {
    std::env::var("REMIXCLIENT_PORTABLE")
        .ok()
        .as_deref()
        .is_some_and(truthy)
}

fn truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off"
    )
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

fn marker_found() -> bool {
    exe_dir().is_some_and(|dir| marker_in(&dir))
}

fn marker_in(dir: &Path) -> bool {
    MARKERS.iter().any(|marker| dir.join(marker).is_file())
}

fn data_dir() -> Option<PathBuf> {
    exe_dir().map(|dir| dir.join(DATA_FOLDER))
}

fn write_marker() -> std::io::Result<()> {
    let Some(dir) = exe_dir() else {
        return Ok(());
    };
    if marker_in(&dir) {
        return Ok(());
    }
    std::fs::write(dir.join(MARKERS[0]), b"RemixClient portable mode\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_values_are_read_like_a_human_wrote_them() {
        for value in ["1", "true", "yes", "on", "portable", " TRUE "] {
            assert!(truthy(value), "{value:?}");
        }
        for value in ["", "0", "false", "no", "off", "   "] {
            assert!(!truthy(value), "{value:?}");
        }
    }

    #[test]
    fn either_marker_file_enables_portable_mode() {
        let root = std::env::temp_dir().join(format!(
            "remixclient-portable-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        assert!(!marker_in(&root));

        std::fs::write(root.join("remixclient.portable"), b"").unwrap();
        assert!(marker_in(&root));

        std::fs::remove_file(root.join("remixclient.portable")).unwrap();
        std::fs::write(root.join("portable.txt"), b"").unwrap();
        assert!(marker_in(&root));

        let _ = std::fs::remove_dir_all(root);
    }
}
