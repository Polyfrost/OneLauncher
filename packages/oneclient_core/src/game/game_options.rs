//! Edits to the game's own `options.txt` applied right before launch

use std::path::Path;

use oneclient_common::domain::GraphicsBackend;

use crate::LauncherResult;

const OPTIONS_NAME: &str = "options.txt";
const GRAPHICS_BACKEND_KEY: &str = "preferredGraphicsBackend";

/// Every other line is kept as is so the game's own settings survive
pub async fn write_graphics_backend(
    game_dir: &Path,
    backend: GraphicsBackend,
) -> LauncherResult<()> {
    let path = game_dir.join(OPTIONS_NAME);
    // A missing file is a first launch the game fills in the rest itself
    let current = if polyio::try_exists(&path).await.unwrap_or(false) {
        polyio::read_to_string(&path).await?
    } else {
        String::new()
    };

    let updated = set_option(
        &current,
        GRAPHICS_BACKEND_KEY,
        &format!("\"{}\"", backend.as_str()),
    );
    if updated != current {
        polyio::write(&path, updated).await?;
    }
    Ok(())
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
