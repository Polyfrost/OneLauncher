//! The game's own `options.txt` is the source of truth for the settings the
//! launcher exposes, so the game's crash fallback can still rewrite them

use std::path::Path;

use oneclient_common::domain::GraphicsBackend;

use crate::LauncherResult;
use crate::game::GameError;

const OPTIONS_NAME: &str = "options.txt";
const GRAPHICS_BACKEND_KEY: &str = "preferredGraphicsBackend:";

/// `None` until a game that has the option saves the file
pub async fn read_graphics_backend(game_dir: &Path) -> Option<GraphicsBackend> {
    let options = polyio::read_to_string(game_dir.join(OPTIONS_NAME))
        .await
        .ok()?;
    serde_json::from_str(find_graphics_backend(&options)?).ok()
}

/// Only replaces a line the game wrote itself, the game resets the option
/// in any file it has not upgraded yet
pub async fn write_graphics_backend(
    game_dir: &Path,
    backend: GraphicsBackend,
) -> LauncherResult<()> {
    let path = game_dir.join(OPTIONS_NAME);
    let options = polyio::read_to_string(&path).await.unwrap_or_default();
    let Some(current) = find_graphics_backend(&options) else {
        return Err(GameError::OptionsOutdated.into());
    };

    let updated = options.replacen(
        &format!("{GRAPHICS_BACKEND_KEY}{current}"),
        &format!("{GRAPHICS_BACKEND_KEY}{}", serde_json::to_string(&backend)?),
        1,
    );
    polyio::write(&path, updated).await?;
    Ok(())
}

fn find_graphics_backend(options: &str) -> Option<&str> {
    options
        .lines()
        .find_map(|line| line.strip_prefix(GRAPHICS_BACKEND_KEY))
}
