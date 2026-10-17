//! The game's own `options.txt` is the source of truth for the settings the
//! launcher exposes, so the game's crash fallback can still rewrite them

use std::path::Path;

use oneclient_common::domain::GraphicsBackend;

use crate::LauncherResult;
use crate::game::GameError;

const OPTIONS_NAME: &str = "options.txt";
const VERSION_KEY: &str = "version";
const GRAPHICS_BACKEND_KEY: &str = "preferredGraphicsBackend";
/// `OptionsForceDefaultGraphicsApiFix` resets the backend in anything older
const GRAPHICS_BACKEND_DATA_VERSION: u32 = 4892;

pub async fn read_graphics_backend(game_dir: &Path) -> LauncherResult<GraphicsBackend> {
    let current = read_options(game_dir).await?.unwrap_or_default();
    Ok(get_option(&current, GRAPHICS_BACKEND_KEY)
        .and_then(|value| GraphicsBackend::parse(value.trim_matches('"')))
        .unwrap_or_default())
}

/// Every other line is kept as is so the game's own settings survive
pub async fn write_graphics_backend(
    game_dir: &Path,
    backend: GraphicsBackend,
) -> LauncherResult<()> {
    let current = match read_options(game_dir).await? {
        // A missing file is a first launch the game fills in the rest itself
        None => {
            polyio::create_dir_all(game_dir).await?;
            format!("{VERSION_KEY}:{GRAPHICS_BACKEND_DATA_VERSION}\n")
        }
        Some(current)
            if get_option(&current, VERSION_KEY)
                .and_then(|version| version.parse::<u32>().ok())
                .is_none_or(|version| version < GRAPHICS_BACKEND_DATA_VERSION) =>
        {
            return Err(GameError::OptionsOutdated.into());
        }
        Some(current) => current,
    };

    let updated = set_option(
        &current,
        GRAPHICS_BACKEND_KEY,
        &format!("\"{}\"", backend.as_str()),
    );
    polyio::write(&game_dir.join(OPTIONS_NAME), updated).await?;
    Ok(())
}

async fn read_options(game_dir: &Path) -> LauncherResult<Option<String>> {
    let path = game_dir.join(OPTIONS_NAME);
    if !polyio::try_exists(&path).await.unwrap_or(false) {
        return Ok(None);
    }
    Ok(Some(polyio::read_to_string(&path).await?))
}

fn get_option<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
}

fn set_option(contents: &str, key: &str, value: &str) -> String {
    let line = format!("{key}:{value}");
    let prefix = format!("{key}:");
    let newline = if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    let mut found = false;
    let mut lines: Vec<&str> = contents
        .lines()
        .map(|existing| {
            if existing.starts_with(&prefix) {
                found = true;
                line.as_str()
            } else {
                existing
            }
        })
        .collect();
    if !found {
        lines.push(&line);
    }

    let mut out = lines.join(newline);
    out.push_str(newline);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_existing_line_and_keeps_the_rest() {
        let before = "version:4000\npreferredGraphicsBackend:\"opengl\"\nfov:0.5\n";
        let after = set_option(before, GRAPHICS_BACKEND_KEY, "\"vulkan\"");
        assert_eq!(
            after,
            "version:4000\npreferredGraphicsBackend:\"vulkan\"\nfov:0.5\n"
        );
    }

    #[test]
    fn appends_when_missing() {
        let after = set_option("fov:0.5", GRAPHICS_BACKEND_KEY, "\"default\"");
        assert_eq!(after, "fov:0.5\npreferredGraphicsBackend:\"default\"\n");
    }

    #[test]
    fn creates_from_empty() {
        assert_eq!(
            set_option("", GRAPHICS_BACKEND_KEY, "\"opengl\""),
            "preferredGraphicsBackend:\"opengl\"\n",
        );
    }

    #[test]
    fn keeps_crlf_files_crlf() {
        let after = set_option("fov:0.5\r\n", GRAPHICS_BACKEND_KEY, "\"vulkan\"");
        assert_eq!(after, "fov:0.5\r\npreferredGraphicsBackend:\"vulkan\"\r\n");
    }
}
