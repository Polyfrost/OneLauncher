//! The game's own `options.txt` is the source of truth for the settings the
//! launcher exposes, so the game's crash fallback can still rewrite them

use std::collections::HashMap;
use std::path::Path;

use oneclient_common::domain::GraphicsBackend;

use crate::LauncherResult;
use crate::game::GameError;

const OPTIONS_NAME: &str = "options.txt";
const GRAPHICS_BACKEND_KEY: &str = "preferredGraphicsBackend:";
const NOT_ONLY_BOOLEAN: &[&str] = &["renderClouds", "chatPreview", "ao"];

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
    polyio::write_atomic(&path, updated).await?;
    Ok(())
}

fn find_graphics_backend(options: &str) -> Option<&str> {
    options
        .lines()
        .find_map(|line| line.strip_prefix(GRAPHICS_BACKEND_KEY))
}

pub async fn read_options(game_dir: &Path) -> Option<Vec<(String, String)>> {
    let options = polyio::read_to_string(game_dir.join(OPTIONS_NAME))
        .await
        .ok()?;
    Some(parse_options(&options))
}

pub async fn write_options(game_dir: &Path, edits: &HashMap<String, String>) -> LauncherResult<()> {
    let path = game_dir.join(OPTIONS_NAME);
    let options = polyio::read_to_string(&path).await?;
    polyio::write_atomic(&path, apply_options(&options, edits)).await?;
    Ok(())
}

pub async fn export_options(
    game_dir: &Path,
    edits: &HashMap<String, String>,
    dest: &Path,
) -> LauncherResult<()> {
    let options = polyio::read_to_string(game_dir.join(OPTIONS_NAME)).await?;
    polyio::write(dest, apply_options(&options, edits)).await?;
    Ok(())
}

pub fn changed_options(
    entries: &[(String, String)],
    edits: &HashMap<String, String>,
) -> HashMap<String, String> {
    edits
        .iter()
        .filter(|(name, value)| !entries.iter().any(|(k, v)| k == *name && v == *value))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

pub fn is_boolean_option(key: &str, value: &str) -> bool {
    matches!(value, "true" | "false") && !NOT_ONLY_BOOLEAN.contains(&key)
}

fn parse_options(options: &str) -> Vec<(String, String)> {
    options
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn apply_options(options: &str, edits: &HashMap<String, String>) -> String {
    options
        .split_inclusive('\n')
        .map(|chunk| {
            let line = chunk.trim_end_matches(['\r', '\n']);
            match line.split_once(':') {
                Some((key, value)) if edits.get(key).is_some_and(|new| new != value) => {
                    format!("{key}:{}{}", edits[key], &chunk[line.len()..])
                }
                _ => chunk.to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_options_drops_untouched_values() {
        let entries = parse_options("fov:0.0\nlang:en_us");
        let edits = HashMap::from([
            ("fov".to_string(), "1.0".to_string()),
            ("lang".to_string(), "en_us".to_string()),
        ]);
        assert_eq!(
            changed_options(&entries, &edits),
            HashMap::from([("fov".to_string(), "1.0".to_string())])
        );
    }

    #[test]
    fn boolean_options_skip_mixed_keys() {
        assert!(is_boolean_option("fullscreen", "false"));
        assert!(!is_boolean_option("renderClouds", "true"));
        assert!(!is_boolean_option("chatPreview", "true"));
        assert!(!is_boolean_option("ao", "true"));
        assert!(!is_boolean_option("lang", "\"true\""));
        assert!(!is_boolean_option("fov", "0.0"));
    }

    #[test]
    fn edits_only_changed_lines() {
        let options =
            "version:3465\r\nfov:0.0\nlang:en_us\nbroken line\nkey_key.attack:key.mouse.left";
        assert_eq!(
            parse_options(options)[3],
            ("key_key.attack".to_string(), "key.mouse.left".to_string())
        );

        let edits = HashMap::from([
            ("version".to_string(), "3465".to_string()),
            ("fov".to_string(), "1.0".to_string()),
            ("key_key.attack".to_string(), "key.keyboard.f".to_string()),
            ("missing".to_string(), "x".to_string()),
        ]);
        assert_eq!(
            apply_options(options, &edits),
            "version:3465\r\nfov:1.0\nlang:en_us\nbroken line\nkey_key.attack:key.keyboard.f"
        );
    }
}
