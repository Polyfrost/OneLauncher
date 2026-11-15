use std::sync::Arc;

use anyhow::{Context, bail};
use clap::{Subcommand, ValueEnum};
use oneclient_cluster::{GameSettingsProfile, PackageUpdateMode, ProfileUpdate, SettingsOsExtra};
use oneclient_common::Patch;
use oneclient_common::Resolution;
use oneclient_core::settings::LauncherSettings;
use oneclient_core::{Cluster, LauncherState};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Output;
use crate::output::{accent, bold, dim, table};

const HIDDEN: &[&str] = &[
    "settings_version",
    "global_game_settings",
    "view_states",
    "seen_versions",
    "pending_release_migrations",
    "microsoft_java_migrated",
    "seen_changelog_version",
    "acknowledged_disable_warnings",
];

const OWNED: &[(&str, &str)] = &[
    ("global_game_settings", "game-settings set"),
    ("data_dir", "storage move"),
    ("previous_data_dir", "storage move"),
    ("accepted_tos_version", "terms accept"),
    ("accepted_privacy_version", "terms accept"),
    ("declined_tos", "terms decline"),
    ("seen_onboarding", "setup"),
];

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum SettingsCommand {
    /// List every launcher setting
    #[command(visible_alias = "ls")]
    List {
        #[arg(long)]
        json: bool,
    },
    /// Print one setting
    Get { key: String },
    /// Change a setting, values are JSON or plain text
    Set {
        key: String,
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    /// Put a setting back to its default
    Reset { key: String },
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum GameSettingsCommand {
    /// Show the game settings
    Show,
    /// Change a game setting
    Set {
        key: GameKey,
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    /// Put a game setting back to its default, or a cluster's back to the global one
    Reset { key: GameKey },
    /// Clear a game setting, or make a cluster's follow the global one
    Unset { key: GameKey },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum GameKey {
    /// Maximum memory, e.g. 4096, 4096M or 4G
    Memory,
    /// Java executable path, or "auto"
    Java,
    /// Extra JVM arguments
    JvmArgs,
    /// Extra game arguments
    GameArgs,
    /// Environment variables, e.g. "KEY=value OTHER=value"
    Env,
    /// Window size, e.g. 1920x1080
    Resolution,
    /// Start in fullscreen, true or false
    Fullscreen,
    /// Command run before the game starts
    HookPre,
    /// Command the game is launched through
    HookWrapper,
    /// Command run after the game exits
    HookPost,
    /// GPU to run on, see `game-settings show`
    Gpu,
    /// Feral GameMode on Linux, true or false
    Gamemode,
    /// What to do with package updates at launch: automatic, skip or prompt
    UpdateMode,
}

impl GameKey {
    const ALL: [Self; 13] = [
        Self::Memory,
        Self::Java,
        Self::JvmArgs,
        Self::GameArgs,
        Self::Env,
        Self::Resolution,
        Self::Fullscreen,
        Self::HookPre,
        Self::HookWrapper,
        Self::HookPost,
        Self::Gpu,
        Self::Gamemode,
        Self::UpdateMode,
    ];

    fn name(self) -> String {
        self.to_possible_value()
            .map(|value| value.get_name().to_string())
            .unwrap_or_default()
    }

    fn read(self, profile: &GameSettingsProfile) -> Option<String> {
        let extra = profile.os_extra.as_ref();
        match self {
            Self::Memory => profile.mem_max.map(|mb| format!("{mb} MB")),
            Self::Java => profile.java_path.clone(),
            Self::JvmArgs => profile.launch_args.clone(),
            Self::GameArgs => profile.game_args.clone(),
            Self::Env => profile.launch_env.clone(),
            Self::Resolution => profile
                .resolution
                .map(|r| format!("{}x{}", r.width, r.height)),
            Self::Fullscreen => profile.force_fullscreen.map(|on| on.to_string()),
            Self::HookPre => profile.hook_pre.clone(),
            Self::HookWrapper => profile.hook_wrapper.clone(),
            Self::HookPost => profile.hook_post.clone(),
            Self::Gpu => extra.and_then(|extra| extra.gpu.clone()),
            #[cfg(target_os = "linux")]
            Self::Gamemode => extra
                .and_then(|extra| extra.enable_gamemode)
                .map(|on| on.to_string()),
            #[cfg(not(target_os = "linux"))]
            Self::Gamemode => None,
            Self::UpdateMode => profile
                .browser_update_mode
                .and_then(|mode| serde_json::to_value(mode).ok())
                .and_then(|mode| mode.as_str().map(str::to_string)),
        }
        .filter(|value| !value.is_empty())
    }

    fn copy(self, from: &GameSettingsProfile, to: &mut GameSettingsProfile) {
        match self {
            Self::Memory => to.mem_max = from.mem_max,
            Self::Java => to.java_path.clone_from(&from.java_path),
            Self::JvmArgs => to.launch_args.clone_from(&from.launch_args),
            Self::GameArgs => to.game_args.clone_from(&from.game_args),
            Self::Env => to.launch_env.clone_from(&from.launch_env),
            Self::Resolution => to.resolution = from.resolution,
            Self::Fullscreen => to.force_fullscreen = from.force_fullscreen,
            Self::HookPre => to.hook_pre.clone_from(&from.hook_pre),
            Self::HookWrapper => to.hook_wrapper.clone_from(&from.hook_wrapper),
            Self::HookPost => to.hook_post.clone_from(&from.hook_post),
            Self::Gpu | Self::Gamemode => {
                let mut extra = to.os_extra.clone().unwrap_or_default();
                let defaults = from.os_extra.clone().unwrap_or_default();
                match self {
                    Self::Gpu => extra.gpu = defaults.gpu,
                    #[cfg(target_os = "linux")]
                    _ => extra.enable_gamemode = defaults.enable_gamemode,
                    #[cfg(not(target_os = "linux"))]
                    _ => {}
                }
                to.os_extra = Some(extra);
            }
            Self::UpdateMode => to.browser_update_mode = from.browser_update_mode,
        }
    }

    fn update(
        self,
        value: Option<&str>,
        os_extra: Option<SettingsOsExtra>,
    ) -> anyhow::Result<ProfileUpdate> {
        let mut update = ProfileUpdate::default();
        let text = |value: Option<&str>| match value.map(str::trim) {
            Some(value) if !value.is_empty() => Patch::Set(value.to_string()),
            _ => Patch::Clear,
        };

        match self {
            Self::Memory => update.mem_max = parse(value, parse_memory)?,
            Self::Java => {
                update.java_path = match value {
                    Some(value) if value.eq_ignore_ascii_case("auto") => Patch::Clear,
                    value => text(value),
                }
            }
            Self::JvmArgs => update.launch_args = text(value),
            Self::GameArgs => update.game_args = text(value),
            Self::Env => update.launch_env = text(value),
            Self::Resolution => update.resolution = parse(value, parse_resolution)?,
            Self::Fullscreen => update.force_fullscreen = parse(value, parse_bool)?,
            Self::HookPre => update.hook_pre = text(value),
            Self::HookWrapper => update.hook_wrapper = text(value),
            Self::HookPost => update.hook_post = text(value),
            Self::Gpu => {
                let gpu = value.map(str::to_string);
                if let Some(gpu) = &gpu {
                    let known = oneclient_core::game::gpu::detect();
                    if !known.is_empty() && !known.iter().any(|g| &g.id == gpu) {
                        bail!(
                            "unknown GPU \"{gpu}\", pick one of: {}",
                            known
                                .iter()
                                .map(|g| g.id.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
                update.os_extra = Patch::Set(SettingsOsExtra {
                    gpu,
                    ..os_extra.unwrap_or_default()
                });
            }
            #[cfg(target_os = "linux")]
            Self::Gamemode => {
                let enabled = value.map(parse_bool).transpose()?;
                update.os_extra = Patch::Set(SettingsOsExtra {
                    enable_gamemode: enabled,
                    ..os_extra.unwrap_or_default()
                });
            }
            #[cfg(not(target_os = "linux"))]
            Self::Gamemode => bail!("GameMode is only available on Linux"),
            Self::UpdateMode => update.browser_update_mode = parse(value, parse_update_mode)?,
        }
        Ok(update)
    }
}

fn parse<T>(
    value: Option<&str>,
    parser: fn(&str) -> anyhow::Result<T>,
) -> anyhow::Result<Patch<T>> {
    Ok(match value {
        Some(value) => Patch::Set(parser(value.trim())?),
        None => Patch::Clear,
    })
}

fn parse_memory(value: &str) -> anyhow::Result<u32> {
    let lower = value.to_lowercase();
    let (number, scale) = match lower.strip_suffix(['g', 'm']) {
        Some(number) if lower.ends_with('g') => (number, 1024),
        Some(number) => (number, 1),
        None => (lower.as_str(), 1),
    };
    let amount: u32 = number
        .trim()
        .parse()
        .with_context(|| format!("\"{value}\" is not an amount of memory, try 4096 or 4G"))?;
    Ok(amount * scale)
}

fn parse_resolution(value: &str) -> anyhow::Result<Resolution> {
    let (width, height) = value
        .split_once(['x', 'X', '*'])
        .with_context(|| format!("\"{value}\" is not a resolution, try 1920x1080"))?;
    Ok(Resolution {
        width: width.trim().parse()?,
        height: height.trim().parse()?,
    })
}

pub(crate) fn parse_bool(value: &str) -> anyhow::Result<bool> {
    match value.to_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Ok(true),
        "false" | "off" | "no" | "0" => Ok(false),
        _ => bail!("\"{value}\" is not true or false"),
    }
}

fn parse_update_mode(value: &str) -> anyhow::Result<PackageUpdateMode> {
    serde_json::from_value(Value::String(value.to_lowercase()))
        .with_context(|| format!("\"{value}\" is not automatic, skip or prompt"))
}

pub(super) fn parse_graphics_backend(
    value: &str,
) -> anyhow::Result<oneclient_common::domain::GraphicsBackend> {
    serde_json::from_value(serde_json::Value::String(value.to_lowercase()))
        .ok()
        .with_context(|| format!("\"{value}\" is not default, opengl or vulkan"))
}

fn normalize(key: &str) -> String {
    key.trim().replace('-', "_")
}

fn render(value: &Value) -> String {
    match value {
        Value::Null => dim("unset"),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

pub(super) async fn run(
    state: &Arc<LauncherState>,
    command: SettingsCommand,
    out: &Output,
) -> anyhow::Result<()> {
    let current = serde_json::to_value(state.settings.read().clone())?;
    let Value::Object(fields) = current else {
        bail!("the settings file is not an object");
    };

    match command {
        SettingsCommand::List { json } => {
            let visible = fields
                .iter()
                .filter(|(key, _)| !HIDDEN.contains(&key.as_str()));
            if json {
                let map: serde_json::Map<String, Value> =
                    visible.map(|(k, v)| (k.clone(), v.clone())).collect();
                out.line(serde_json::to_string_pretty(&map)?);
                return Ok(());
            }
            let rows: Vec<Vec<String>> = visible
                .map(|(key, value)| vec![bold(key), render(value)])
                .collect();
            out.line(table(&["KEY", "VALUE"], &rows));
            Ok(())
        }
        SettingsCommand::Get { key } => {
            let key = normalize(&key);
            let value = fields.get(&key).with_context(|| {
                format!("no setting \"{key}\", see `oneclient_cli settings list`")
            })?;
            out.line(render(value));
            Ok(())
        }
        SettingsCommand::Set { key, value } => {
            let key = normalize(&key);
            let parsed = match fields.get(&key) {
                Some(Value::String(_)) => Value::String(value.clone()),
                _ => serde_json::from_str(&value).unwrap_or_else(|_| Value::String(value.clone())),
            };
            change(state, fields, &key, parsed, &value).await?;
            out.line(format!("Set {} to {}", bold(&key), accent(&value)));
            Ok(())
        }
        SettingsCommand::Reset { key } => {
            let key = normalize(&key);
            let defaults = serde_json::to_value(LauncherSettings::default())?;
            let default = defaults.get(&key).cloned().unwrap_or(Value::Null);
            let shown = render(&default);
            change(state, fields, &key, default, &shown).await?;
            out.line(format!("Reset {} to {shown}", bold(&key)));
            Ok(())
        }
    }
}

async fn change(
    state: &Arc<LauncherState>,
    fields: serde_json::Map<String, Value>,
    key: &str,
    value: Value,
    shown: &str,
) -> anyhow::Result<()> {
    if let Some((_, command)) = OWNED.iter().find(|(owned, _)| *owned == key) {
        bail!("{key} is changed with `oneclient_cli {command}`");
    }
    if !fields.contains_key(key) || HIDDEN.contains(&key) {
        bail!("no setting \"{key}\", see `oneclient_cli settings list`");
    }

    let attempt = |value: Value| {
        let mut fields = fields.clone();
        fields.insert(key.to_string(), value);
        serde_json::from_value::<LauncherSettings>(Value::Object(fields))
    };
    let updated = match (attempt(value.clone()), value) {
        (Ok(updated), _) => Ok(updated),
        (Err(_), Value::String(text)) if text.contains('-') => {
            attempt(Value::String(text.replace('-', "_")))
        }
        (Err(err), _) => Err(err),
    }
    .with_context(|| format!("\"{shown}\" is not a valid value for {key}"))?;

    *state.settings.write() = updated.clone();
    oneclient_core::settings::store::save_settings_and_apply(&state.services, &updated).await?;
    state.discord.set_enabled(updated.discord_enabled);
    Ok(())
}

pub(super) async fn global(
    state: &Arc<LauncherState>,
    command: GameSettingsCommand,
    out: &Output,
) -> anyhow::Result<()> {
    let mut profile = state.settings.read().global_game_settings.clone();
    match command {
        GameSettingsCommand::Show => {
            show(&profile, None, out);
            Ok(())
        }
        GameSettingsCommand::Set { key, value } => {
            key.update(Some(&value), profile.os_extra.clone())?
                .apply(&mut profile);
            oneclient_core::settings::store::save_global_profile(&state.settings, profile).await?;
            out.line(format!("Set {} to {}", bold(key.name()), accent(&value)));
            Ok(())
        }
        GameSettingsCommand::Unset { key } => {
            key.update(None, profile.os_extra.clone())?
                .apply(&mut profile);
            oneclient_core::settings::store::save_global_profile(&state.settings, profile).await?;
            out.line(format!("Cleared {}", bold(key.name())));
            Ok(())
        }
        GameSettingsCommand::Reset { key } => {
            key.copy(&GameSettingsProfile::default_global_profile(), &mut profile);
            let shown = key.read(&profile).unwrap_or_else(|| "unset".into());
            oneclient_core::settings::store::save_global_profile(&state.settings, profile).await?;
            out.line(format!("Reset {} to {shown}", bold(key.name())));
            Ok(())
        }
    }
}

async fn own_profile(
    state: &LauncherState,
    cluster: &Cluster,
) -> anyhow::Result<Option<GameSettingsProfile>> {
    let Some(name) = &cluster.setting_profile_name else {
        return Ok(None);
    };
    let row = oneclient_db::dao::setting_profile::get_by_name(&state.services.db, name).await?;
    Ok(row.map(GameSettingsProfile::from_row).transpose()?)
}

pub(super) async fn cluster(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    command: GameSettingsCommand,
    out: &Output,
) -> anyhow::Result<()> {
    let global = state.settings.read().global_game_settings.clone();
    match command {
        GameSettingsCommand::Show => {
            let effective = state.clusters.resolve_settings(&global, cluster).await?;
            let own = own_profile(state, cluster).await?;
            show(&effective, Some(own.as_ref()), out);
            Ok(())
        }
        GameSettingsCommand::Set { key, value } => {
            let own = own_profile(state, cluster).await?;
            ensure_profile(state, cluster, &global).await?;
            let update = key.update(Some(&value), own.and_then(|p| p.os_extra))?;
            state.clusters.update_profile(cluster.id, update).await?;
            out.line(format!(
                "Set {} to {} for {}",
                bold(key.name()),
                accent(&value),
                cluster.name
            ));
            Ok(())
        }
        GameSettingsCommand::Reset { key } | GameSettingsCommand::Unset { key } => {
            let own = own_profile(state, cluster).await?;
            ensure_profile(state, cluster, &global).await?;
            let update = match key {
                GameKey::Gpu | GameKey::Gamemode => {
                    let mut own = own.unwrap_or_else(|| global.clone());
                    key.copy(&GameSettingsProfile::default_global_profile(), &mut own);
                    ProfileUpdate {
                        os_extra: match own.os_extra {
                            Some(extra) => Patch::Set(extra),
                            None => Patch::Clear,
                        },
                        ..ProfileUpdate::default()
                    }
                }
                _ => key.update(None, None)?,
            };
            state.clusters.update_profile(cluster.id, update).await?;
            out.line(format!(
                "{} now follows the global setting for {}",
                bold(key.name()),
                cluster.name
            ));
            Ok(())
        }
    }
}

async fn ensure_profile(
    state: &LauncherState,
    cluster: &Cluster,
    global: &GameSettingsProfile,
) -> anyhow::Result<()> {
    if cluster.setting_profile_name.is_none() {
        state
            .clusters
            .create_and_assign_profile(global, cluster.id, &cluster.folder_name)
            .await?;
    }
    Ok(())
}

fn show(profile: &GameSettingsProfile, own: Option<Option<&GameSettingsProfile>>, out: &Output) {
    let rows: Vec<Vec<String>> = GameKey::ALL
        .iter()
        .filter(|key| cfg!(target_os = "linux") || **key != GameKey::Gamemode)
        .map(|key| {
            let value = key.read(profile).unwrap_or_else(|| dim("unset"));
            let mut row = vec![bold(key.name()), value];
            if let Some(own) = own {
                row.push(if own.and_then(|own| key.read(own)).is_some() {
                    accent("cluster")
                } else {
                    dim("global")
                });
            }
            row
        })
        .collect();

    let headers: &[&str] = if own.is_some() {
        &["KEY", "VALUE", "FROM"]
    } else {
        &["KEY", "VALUE"]
    };
    out.line(table(headers, &rows));

    let gpus = oneclient_core::game::gpu::detect();
    if !gpus.is_empty() {
        out.line(dim(format!(
            "\nGPUs: {}",
            gpus.iter()
                .map(|gpu| format!("{} ({})", gpu.id, gpu.name))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
}
