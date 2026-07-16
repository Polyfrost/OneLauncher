use std::path::{Path, PathBuf};

use serde::Deserialize;
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::LauncherResult;
use crate::packages::domain::GameLoader;
use crate::paths;
use crate::settings::{GameSettingsProfile, Resolution, store};

use super::fs::{copy_tree, dir_has_content};
use super::{ImportTarget, MigrationDetection, MigrationSource, SourceInstance};

const IMPORT_EXCLUDE_TOP: &[&str] = &[
    "mods",
    "logs",
    "optionsLC.txt",
    "optionsof.txt",
    "base-modpack.mrpack",
    "servers.dat_old",
];

pub fn old_root() -> Option<PathBuf> {
    let base = directories::BaseDirs::new().map(|d| d.home_dir().join(".lunarclient"));

    let root = match std::env::var("LUNAR_CLIENT_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => base?,
    };

    let exists = root.is_dir();
    tracing::debug!(root = %root.display(), exists, "lunar migration: resolved old root");
    exists.then_some(root)
}

/// Lunar mod ids appear in two spellings across config versions (`MOTION_BLUR` and `motionBlur`)
fn normalize_mod_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

/// Maps a Lunar mod onto the OneClient bundle category that ships an equivalent.
/// Ids absent from this table have no counterpart and contribute no category.
fn category_for_mod(normalized_id: &str) -> Option<&'static str> {
    let category = match normalized_id {
        "FPS" | "CPS" | "PING" | "CLOCK" | "PLAYTIME" | "MEMORY" | "STOPWATCH" | "COORDINATES"
        | "COORDS" | "DIRECTIONHUD" | "ARMORSTATUS" | "KEYSTROKES" | "SCOREBOARD"
        | "POTIONEFFECTS" | "POTIONCOUNTER" | "ITEMCOUNTER" | "TOTEMCOUNTER" | "ITEMTRACKER"
        | "DAYCOUNTER" | "F3DISPLAY" | "BOSSBAR" | "TAB" | "TITLES" | "TITLEMOD"
        | "INVENTORYMOD" | "HORSESTATS" | "PACKDISPLAY" | "RESOURCEDISPLAY" | "SERVERADDRESS"
        | "SERVERADDRESSMOD" | "TEAMVIEW" | "SHINYPOTS" => "HUD",

        "HITBOX" | "HITCOLOR" | "PVPINFO" | "TIERTAGGER" | "UHCOVERLAY" | "SNAPLOOK"
        | "HURTCAM" | "DAMAGETINT" | "REACHDISPLAY" | "RANGE" | "COMBO" | "COOLDOWNS"
        | "OVERLAYMOD" | "TNTCOUNTDOWN" => "PvP",

        "MOTIONBLUR" | "MENUBLUR" | "3DSKINS" | "SKINLAYERS3D" | "GLINTCOLORIZER" | "CROSSHAIR"
        | "BLOCKOUTLINE" | "FOG" | "FOGMOD" | "FOV" | "FOVMOD" | "ITEMPHYSICS" | "ITEMPHYSIC"
        | "SCROLLABLETOOLTIPS" | "CHAT" | "NAMETAG" | "NICKHIDER" | "TIMECHANGER"
        | "WEATHERCHANGER" | "PARTICLECHANGER" | "PARTICLEMOD" | "SATURATION" | "SATURATIONMOD"
        | "COLORSATURATION" | "LIGHTING" | "2DITEMS" | "ITEMS2D" | "SHULKERPREVIEW"
        | "AUDIOSUBTITLES" | "SOUNDCHANGER" | "MOBSIZE" | "MOMENTUM" | "MOMENTUMMOD"
        | "ONESEVENVISUALS" | "PACKORGANIZER" | "ITEMCUSTOMIZER" | "AUTOTEXTACTIONS"
        | "AUTOTEXTHOTKEY" | "TEXTHOTKEY" => "QoL",

        "ZOOM" | "WAILA" | "FREELOOK" | "MUMBLELINK" | "WORLDEDITCUI" | "CHUNKBORDERS"
        | "SCREENSHOT" | "REPLAYMOD" | "REWIND" | "RADIO" | "MINIMAP" | "WAYPOINTS" | "MARKERS"
        | "KILLSOUNDS" | "QUICKPLAY" | "TOGGLESNEAK" | "GUISCALE" | "HYPIXELMOD"
        | "HYPIXELBEDWARS" => "Utility",

        "SKYBLOCK" | "SKYBLOCKADDONS" | "NEU" => "SkyBlock",

        _ => return None,
    };

    Some(category)
}

#[derive(Debug, Deserialize)]
struct LunarModEntry {
    #[serde(default)]
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct LunarSettingsProfile {
    name: String,
    #[serde(default)]
    default: bool,
    #[serde(default)]
    active: bool,
}

#[tracing::instrument(level = "debug")]
async fn active_profile_name(root: &Path) -> Option<String> {
    let path = root
        .join("settings")
        .join("game")
        .join("profile_manager.json");
    let raw = polyio::read(&path).await.ok()?;
    let profiles: Vec<LunarSettingsProfile> = serde_json::from_slice(&raw)
        .inspect_err(|err| {
            tracing::warn!(error = %err, "malformed profile_manager.json; no mod settings");
        })
        .ok()?;

    let chosen = profiles
        .iter()
        .find(|p| p.active)
        .or_else(|| profiles.iter().find(|p| p.default))
        .or_else(|| profiles.first())?;

    tracing::debug!(profile = %chosen.name, "lunar migration: active settings profile");
    Some(chosen.name.clone())
}

/// The active profile's `mods.json`, as a raw object.
async fn active_profile_mods(root: &Path) -> Option<serde_json::Map<String, serde_json::Value>> {
    let name = active_profile_name(root).await?;
    let path = root
        .join("settings")
        .join("game")
        .join(name)
        .join("mods.json");
    let raw = polyio::read(&path).await.ok()?;

    serde_json::from_slice(&raw)
        .inspect_err(|err| {
            tracing::warn!(path = %path.display(), error = %err, "malformed mods.json");
        })
        .ok()
}

#[tracing::instrument(level = "debug")]
async fn enabled_categories(root: &Path) -> Vec<String> {
    let settings_dir = root.join("settings").join("game");
    let mut categories: Vec<String> = Vec::new();

    let mut entries = match polyio::read_dir(&settings_dir).await {
        Ok(entries) => entries,
        Err(err) => {
            tracing::debug!(error = %err, "lunar migration: no game settings dir; no categories");
            return categories;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let mods_file = entry.path().join("mods.json");
        if !mods_file.is_file() {
            continue;
        }

        let raw = match polyio::read(&mods_file).await {
            Ok(raw) => raw,
            Err(err) => {
                tracing::warn!(path = %mods_file.display(), error = %err, "unreadable mods.json");
                continue;
            }
        };

        // Values are heterogeneous (`"version": 14` alongside mod objects), so
        // anything that does not deserialize into a mod entry is skipped.
        let parsed: serde_json::Map<String, serde_json::Value> = match serde_json::from_slice(&raw)
        {
            Ok(parsed) => parsed,
            Err(err) => {
                tracing::warn!(path = %mods_file.display(), error = %err, "malformed mods.json");
                continue;
            }
        };

        for (mod_id, value) in parsed {
            let Ok(entry) = serde_json::from_value::<LunarModEntry>(value) else {
                continue;
            };
            if !entry.enabled {
                continue;
            }

            let Some(category) = category_for_mod(&normalize_mod_id(&mod_id)) else {
                continue;
            };
            if !categories.iter().any(|c| c == category) {
                categories.push(category.to_string());
            }
        }
    }

    categories.sort();
    categories
}

fn loader_from_json(raw: &str) -> GameLoader {
    let loaders: Vec<String> = serde_json::from_str(raw).unwrap_or_default();

    if loaders.iter().any(|l| l.eq_ignore_ascii_case("fabric")) {
        GameLoader::Fabric
    } else if loaders.iter().any(|l| l.eq_ignore_ascii_case("quilt")) {
        GameLoader::Quilt
    } else if loaders.iter().any(|l| l.eq_ignore_ascii_case("forge")) {
        GameLoader::Forge
    } else {
        GameLoader::Vanilla
    }
}

#[tracing::instrument]
pub async fn detect() -> LauncherResult<Option<MigrationDetection>> {
    let Some(root) = old_root() else {
        return Ok(None);
    };

    let db_path = root.join("db").join("profiles.db");
    if !db_path.exists() {
        tracing::debug!(db = %db_path.display(), "lunar migration: no profiles.db, skipping");
        return Ok(None);
    }

    match detect_inner(&root, &db_path).await {
        Ok(detection) => {
            tracing::info!(
                instances = detection.instances.len(),
                "lunar migration: detected old install"
            );
            Ok(Some(detection))
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to read lunar profiles.db; skipping migration");
            Ok(None)
        }
    }
}

#[tracing::instrument(level = "debug")]
async fn detect_inner(root: &Path, db_path: &Path) -> LauncherResult<MigrationDetection> {
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .read_only(true)
        .immutable(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;

    let profile_rows = sqlx::query("SELECT path, game_version, loaders FROM profiles")
        .fetch_all(&pool)
        .await?;

    pool.close().await;

    let categories = enabled_categories(root).await;

    let mut instances = Vec::with_capacity(profile_rows.len());
    for (index, row) in profile_rows.into_iter().enumerate() {
        let folder_name: String = row.try_get("path")?;
        let mc_version: String = row.try_get("game_version")?;
        let loaders_raw: String = row.try_get("loaders")?;

        let has_game_dir = dir_has_content(&root.join("profiles").join(&folder_name)).await;

        instances.push(SourceInstance {
            // Lunar keys profiles by uuid; SourceInstance ids are informational.
            instance_id: index as i64,
            folder_name,
            mc_version,
            mc_loader: loader_from_json(&loaders_raw),
            categories: categories.clone(),
            has_game_dir,
        });
    }

    Ok(MigrationDetection {
        source: MigrationSource::LunarClient,
        root: root.to_path_buf(),
        instances,
    })
}

#[derive(Debug, Deserialize)]
struct LunarLauncherFile {
    settings: LunarLauncherSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LunarLauncherSettings {
    allocated_memory: Option<u32>,
    game_resolution: Option<LunarResolution>,
}

#[derive(Debug, Deserialize)]
struct LunarResolution {
    width: Option<u32>,
    height: Option<u32>,
    #[serde(default)]
    fullscreen: Option<bool>,
}

#[tracing::instrument]
pub async fn import_settings() -> LauncherResult<()> {
    let Some(root) = old_root() else {
        return Ok(());
    };

    let path = root.join("settings").join("launcher.json");
    if !path.is_file() {
        tracing::debug!(path = %path.display(), "lunar migration: no launcher.json");
        return Ok(());
    }

    let raw = polyio::read(&path).await?;
    let parsed: LunarLauncherFile = match serde_json::from_slice(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::warn!(error = %err, "malformed lunar launcher.json; keeping current settings");
            return Ok(());
        }
    };

    let state = crate::state::LauncherState::get()?;
    let mut global: GameSettingsProfile = state.settings.read().global_game_settings.clone();

    apply_lunar_settings(&mut global, &parsed.settings);

    store::save_global_profile(&state.settings, global).await?;
    tracing::info!("lunar migration: imported launcher settings");

    Ok(())
}

fn apply_lunar_settings(global: &mut GameSettingsProfile, lunar: &LunarLauncherSettings) {
    if let Some(memory) = lunar.allocated_memory {
        global.mem_max = Some(memory);
    }

    if let Some(resolution) = &lunar.game_resolution {
        if let (Some(width), Some(height)) = (resolution.width, resolution.height) {
            global.resolution = Some(Resolution::new(width, height));
        }
        if let Some(fullscreen) = resolution.fullscreen {
            global.force_fullscreen = Some(fullscreen);
        }
    }
}

/// Lunar stores the vanilla options it manages in `optionsLC.txt`, pretty much the same as options.txt just in json
#[tracing::instrument(level = "debug")]
async fn merge_lunar_options(src: &Path, dest: &Path) -> LauncherResult<()> {
    let lunar_options = src.join("optionsLC.txt");
    if !lunar_options.is_file() {
        return Ok(());
    }

    let raw = polyio::read(&lunar_options).await?;
    let parsed: serde_json::Map<String, serde_json::Value> = match serde_json::from_slice(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::warn!(error = %err, "malformed optionsLC.txt; keeping vanilla options.txt");
            return Ok(());
        }
    };

    let options_path = dest.join("options.txt");
    let existing = match polyio::read(&options_path).await {
        Ok(existing) => String::from_utf8_lossy(&existing).into_owned(),
        Err(_) => String::new(),
    };

    let merged = merge_options_text(&existing, &parsed);
    polyio::write(&options_path, merged.into_bytes()).await?;

    Ok(())
}

/// Overwrites `options.txt` entries Lunar owns, appends the ones it adds, and
/// leaves every other line untouched.
fn merge_options_text(
    existing: &str,
    lunar: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let overrides: Vec<(String, String)> = lunar
        .iter()
        .filter_map(|(key, value)| {
            // Lunar's own bookkeeping keys are not vanilla options.
            if key == "lastLaunchedVersion" || key == "version" {
                return None;
            }
            let rendered = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => return None,
            };
            Some((key.clone(), rendered))
        })
        .collect();

    let mut out = String::with_capacity(existing.len());
    let mut written: Vec<&str> = Vec::new();

    for line in existing.lines() {
        let key = line.split_once(':').map(|(k, _)| k);
        match key.and_then(|k| overrides.iter().find(|(ok, _)| ok == k)) {
            Some((k, v)) => {
                out.push_str(&format!("{k}:{v}\n"));
                written.push(k);
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }

    for (key, value) in &overrides {
        if !written.iter().any(|w| w == key) {
            out.push_str(&format!("{key}:{value}\n"));
        }
    }

    out
}

#[tracing::instrument(skip(target))]
pub async fn import_game_dir(folder_name: &str, target: ImportTarget) -> LauncherResult<()> {
    let Some(root) = old_root() else {
        return Ok(());
    };
    let src = root.join("profiles").join(folder_name);
    if !src.is_dir() {
        tracing::warn!(
            folder_name,
            "lunar profile folder missing; nothing to import"
        );
        return Ok(());
    }

    let dest = match &target {
        ImportTarget::Shared => paths::shared_minecraft_dir()?,
        ImportTarget::Dedicated { new_cluster_id } => {
            let state = crate::state::LauncherState::get()?;
            let cluster = crate::clusters::ClusterManager::get(&state, *new_cluster_id).await?;
            let dir = cluster.dir()?;
            polyio::create_dir_all(&dir).await?;
            // Mark dedicated so game_dir() resolves to this cluster's own dir.
            polyio::write(cluster.dedicated_marker()?, Vec::new()).await?;
            dir
        }
    };

    polyio::create_dir_all(&dest).await?;
    copy_tree(&src, &dest, IMPORT_EXCLUDE_TOP).await?;
    merge_lunar_options(&src, &dest).await?;

    if let Some(mods) = active_profile_mods(&root).await {
        super::lunar_mods::apply(&mods, &dest).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_ids_normalize_across_spellings() {
        assert_eq!(
            normalize_mod_id("MOTION_BLUR"),
            normalize_mod_id("motionBlur")
        );
        assert_eq!(
            normalize_mod_id("TOGGLE_SNEAK"),
            normalize_mod_id("toggleSneak")
        );
        assert_eq!(normalize_mod_id("HIT_COLOR"), normalize_mod_id("hitColor"));
    }

    #[test]
    fn known_mods_map_to_bundle_categories() {
        assert_eq!(category_for_mod(&normalize_mod_id("FPS")), Some("HUD"));
        assert_eq!(category_for_mod(&normalize_mod_id("HITBOX")), Some("PvP"));
        assert_eq!(
            category_for_mod(&normalize_mod_id("motionBlur")),
            Some("QoL")
        );
        assert_eq!(category_for_mod(&normalize_mod_id("ZOOM")), Some("Utility"));
        assert_eq!(
            category_for_mod(&normalize_mod_id("SKYBLOCK")),
            Some("SkyBlock")
        );
        assert_eq!(category_for_mod(&normalize_mod_id("TOTALLY_UNKNOWN")), None);
    }

    #[test]
    fn ichor_only_profile_falls_back_to_vanilla() {
        assert_eq!(loader_from_json(r#"["ichor"]"#), GameLoader::Vanilla);
        assert_eq!(
            loader_from_json(r#"["ichor","fabric"]"#),
            GameLoader::Fabric
        );
        assert_eq!(loader_from_json("not json"), GameLoader::Vanilla);
    }

    #[test]
    fn lunar_options_overwrite_matching_keys_and_keep_the_rest() {
        let existing = "fov:0.0\nrenderDistance:12\nkeep:me\n";
        let mut lunar = serde_json::Map::new();
        lunar.insert("fov".into(), serde_json::json!("0.5"));
        lunar.insert("maxFps".into(), serde_json::json!("170"));
        lunar.insert("lastLaunchedVersion".into(), serde_json::json!("v1_8"));

        let merged = merge_options_text(existing, &lunar);

        assert!(merged.contains("fov:0.5"));
        assert!(!merged.contains("fov:0.0"));
        assert!(merged.contains("renderDistance:12"));
        assert!(merged.contains("keep:me"));
        assert!(merged.contains("maxFps:170"));
        // Lunar bookkeeping is not a vanilla option.
        assert!(!merged.contains("lastLaunchedVersion"));
    }

    #[test]
    fn allocated_memory_and_resolution_land_on_the_global_profile() {
        let mut global = GameSettingsProfile {
            name: "Global".to_string(),
            java_path: None,
            resolution: None,
            force_fullscreen: None,
            mem_max: None,
            launch_args: None,
            launch_env: None,
            hook_pre: None,
            hook_wrapper: None,
            hook_post: None,
            os_extra: None,
        };

        apply_lunar_settings(
            &mut global,
            &LunarLauncherSettings {
                allocated_memory: Some(4195),
                game_resolution: Some(LunarResolution {
                    width: Some(1280),
                    height: Some(720),
                    fullscreen: Some(true),
                }),
            },
        );

        assert_eq!(global.mem_max, Some(4195));
        assert_eq!(global.resolution, Some(Resolution::new(1280, 720)));
        assert_eq!(global.force_fullscreen, Some(true));
    }
}
