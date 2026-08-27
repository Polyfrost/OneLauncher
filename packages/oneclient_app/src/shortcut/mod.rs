#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
use linux as imp;
#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(windows)]
use windows as imp;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

const MAX_STEM: usize = 96;
const FALLBACK_STEM: &str = "OneClient";
const MAX_COLLISIONS: usize = 20;
const FORBIDDEN: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub struct ShortcutRequest {
    pub cluster_name: String,
    pub folder_name: String,
    pub dir: PathBuf,
}

pub fn create(request: &ShortcutRequest) -> Result<PathBuf> {
    let exe = launcher_exe()?;

    std::fs::create_dir_all(&request.dir)
        .with_context(|| format!("couldn't open {}", request.dir.display()))?;

    let path = unique_path(&request.dir, &file_stem(&request.cluster_name), imp::EXTENSION)?;
    imp::write(request, &exe, &path)
        .with_context(|| format!("couldn't write {}", path.display()))?;

    tracing::info!(path = %path.display(), folder = request.folder_name, "wrote cluster shortcut");
    Ok(path)
}

#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    directories::UserDirs::new()
        .and_then(|dirs| dirs.desktop_dir().map(Path::to_path_buf))
        .filter(|dir| dir.is_dir())
}

pub fn launcher_exe() -> Result<PathBuf> {
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let path = PathBuf::from(appimage);
        if path.is_file() {
            return Ok(path);
        }
    }

    let exe = std::env::current_exe().context("couldn't locate the OneClient executable")?;
    Ok(dunce::canonicalize(&exe).unwrap_or(exe))
}

fn file_stem(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| {
            if FORBIDDEN.contains(&c) || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();

    let trimmed = replaced.trim().trim_end_matches(['.', ' ']).trim();
    if trimmed.is_empty() {
        return FALLBACK_STEM.to_string();
    }

    let truncated: String = trimmed.chars().take(MAX_STEM).collect();
    let truncated = truncated.trim_end().trim_end_matches('.').to_string();
    if truncated.is_empty() {
        return FALLBACK_STEM.to_string();
    }

    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&truncated)) {
        return format!("{truncated}-shortcut");
    }

    truncated
}

fn unique_path(dir: &Path, stem: &str, extension: &str) -> Result<PathBuf> {
    for attempt in 1..=MAX_COLLISIONS {
        let name = if attempt == 1 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem} ({attempt}).{extension}")
        };

        let candidate = dir.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    bail!("there are already {MAX_COLLISIONS} shortcuts named \"{stem}\" in that folder")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_name_is_left_alone() {
        assert_eq!(file_stem("Fabric 1.20"), "Fabric 1.20");
    }

    #[test]
    fn path_separators_cannot_escape_the_chosen_folder() {
        assert_eq!(file_stem("../../etc/passwd"), "..-..-etc-passwd");
        assert_eq!(file_stem(r"a\b"), "a-b");
    }

    #[test]
    fn a_name_that_is_all_illegal_falls_back() {
        assert_eq!(file_stem("   "), FALLBACK_STEM);
        assert_eq!(file_stem(""), FALLBACK_STEM);
        assert_eq!(file_stem("..."), FALLBACK_STEM);
    }

    #[test]
    fn reserved_device_names_are_pushed_out_of_the_way() {
        assert_eq!(file_stem("NUL"), "NUL-shortcut");
        assert_eq!(file_stem("com1"), "com1-shortcut");
        assert_eq!(file_stem("CONSOLE"), "CONSOLE");
    }

    #[test]
    fn a_trailing_dot_is_dropped_before_windows_drops_it_silently() {
        assert_eq!(file_stem("Pack."), "Pack");
        assert_eq!(file_stem("Pack "), "Pack");
    }

    #[test]
    fn a_very_long_name_is_cut_to_a_usable_length() {
        let stem = file_stem(&"ą".repeat(400));
        assert_eq!(stem.chars().count(), MAX_STEM);
    }

    #[test]
    fn a_control_character_cannot_reach_the_file_name() {
        assert_eq!(file_stem("Pack\nEvil"), "Pack-Evil");
    }

    #[test]
    fn the_first_shortcut_gets_the_plain_name() {
        let dir = std::env::temp_dir().join("oneclient-shortcut-plain");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = unique_path(&dir, "Pack", "url").unwrap();
        assert_eq!(path.file_name().unwrap(), "Pack.url");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_shortcut_does_not_replace_the_first() {
        let dir = std::env::temp_dir().join("oneclient-shortcut-collide");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Pack.url"), b"existing").unwrap();

        let path = unique_path(&dir, "Pack", "url").unwrap();
        assert_eq!(path.file_name().unwrap(), "Pack (2).url");
        assert_eq!(std::fs::read(dir.join("Pack.url")).unwrap(), b"existing");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
