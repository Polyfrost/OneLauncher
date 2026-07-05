use oneclient_db::DbPool;
use parking_lot::RwLock;
use tracing::instrument;

use crate::notification::NotificationService;
use crate::patch::Patch;
use crate::paths;
use crate::{LauncherError, LauncherResult};

use super::launcher::LauncherSettings;
use super::profile::{GameSettingsProfile, Resolution, SettingsOsExtra};

#[instrument(skip(notify))]
pub async fn load_settings(notify: Option<&NotificationService>) -> LauncherSettings {
    match async {
        let path = paths::settings_file()?;
        let exists = polyio::try_exists(&path).await?;

        Ok::<LauncherSettings, LauncherError>(if !exists {
            LauncherSettings::default()
        } else {
            let data = polyio::read(&path).await?;
            serde_json::from_slice(&data)?
        })
    }
    .await
    {
        Ok(settings) => settings,
        Err(err) => {
            tracing::warn!("failed to read settings file: {err}");

            if let Some(notify) = notify {
                notify.send_error("Settings", "Failed to load settings");
            }

            LauncherSettings::default()
        }
    }
}

#[instrument(skip(settings))]
pub async fn save_settings(settings: &LauncherSettings) -> LauncherResult<()> {
    let path = paths::settings_file()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let data = serde_json::to_string_pretty(settings)?;
    polyio::write(path, data).await?;
    Ok(())
}

pub async fn get_profile_or_default(
    pool: &DbPool,
    settings: &LauncherSettings,
    name: Option<&str>,
) -> LauncherResult<GameSettingsProfile> {
    if let Some(name) = name
        && let Some(row) = oneclient_db::dao::setting_profile::get_by_name(pool, name).await?
    {
        return GameSettingsProfile::from_row(row);
    }

    Ok(settings.global_game_settings.clone())
}

pub async fn resolve_cluster_profile(
    pool: &DbPool,
    settings: &LauncherSettings,
    profile_name: Option<&str>,
) -> LauncherResult<GameSettingsProfile> {
    let global = settings.global_game_settings.clone();

    let Some(name) = profile_name else {
        return Ok(global);
    };

    let Some(row) = oneclient_db::dao::setting_profile::get_by_name(pool, name).await? else {
        return Ok(global);
    };

    let mut profile = GameSettingsProfile::from_row(row)?;
    if !profile.is_global() {
        profile.merge_global(&global);
    }

    Ok(profile)
}

pub async fn upsert_named_profile(
    pool: &DbPool,
    profile: &GameSettingsProfile,
) -> LauncherResult<GameSettingsProfile> {
    if profile.is_global() {
        return Err(LauncherError::InvalidSettingsProfile {
            reason: "the Global profile is stored in settings.json".into(),
        });
    }

    let row = profile.into_row()?;
    let saved = oneclient_db::dao::setting_profile::upsert(pool, &row).await?;
    GameSettingsProfile::from_row(saved)
}

pub async fn create_profile_from_global(
    pool: &DbPool,
    settings: &LauncherSettings,
    name: &str,
    mem_max: Option<u32>,
    force_fullscreen: Option<bool>,
) -> LauncherResult<GameSettingsProfile> {
    let mut profile = settings.global_game_settings.clone();

    profile.name = name.to_string();
    if let Some(mem) = mem_max {
        profile.mem_max = Some(mem);
    }

    if let Some(fullscreen) = force_fullscreen {
        profile.force_fullscreen = Some(fullscreen);
    }

    upsert_named_profile(pool, &profile).await
}

pub async fn create_settings_profile(
    pool: &DbPool,
    settings: &LauncherSettings,
    name: &str,
) -> LauncherResult<GameSettingsProfile> {
    create_profile_from_global(pool, settings, name, None, None).await
}

pub async fn list_named_profiles(pool: &DbPool) -> LauncherResult<Vec<GameSettingsProfile>> {
    let rows = oneclient_db::dao::setting_profile::list_all(pool).await?;
    rows.into_iter()
        .map(GameSettingsProfile::from_row)
        .collect()
}

pub async fn delete_named_profile(pool: &DbPool, name: &str) -> LauncherResult<()> {
    oneclient_db::dao::setting_profile::delete_by_name(pool, name).await?;
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct ProfileUpdate {
    pub java_path: Patch<String>,
    pub resolution: Patch<Resolution>,
    pub force_fullscreen: Patch<bool>,
    pub mem_max: Patch<u32>,
    pub launch_args: Patch<String>,
    pub launch_env: Patch<String>,
    pub hook_pre: Patch<String>,
    pub hook_wrapper: Patch<String>,
    pub hook_post: Patch<String>,
    pub os_extra: Patch<SettingsOsExtra>,
}

impl ProfileUpdate {
    pub fn apply(&self, profile: &mut GameSettingsProfile) {
        self.java_path.apply_to_option(&mut profile.java_path);
        self.resolution.apply_to_option(&mut profile.resolution);
        self.force_fullscreen
            .apply_to_option(&mut profile.force_fullscreen);
        self.mem_max.apply_to_option(&mut profile.mem_max);
        self.launch_args
            .apply_to_command_option(&mut profile.launch_args);
        self.launch_env
            .apply_to_command_option(&mut profile.launch_env);
        self.hook_pre.apply_to_command_option(&mut profile.hook_pre);
        self.hook_wrapper
            .apply_to_command_option(&mut profile.hook_wrapper);
        self.hook_post
            .apply_to_command_option(&mut profile.hook_post);
        self.os_extra.apply_to_option(&mut profile.os_extra);
    }
}

pub async fn update_named_profile(
    pool: &DbPool,
    name: &str,
    update: ProfileUpdate,
) -> LauncherResult<GameSettingsProfile> {
    let Some(row) = oneclient_db::dao::setting_profile::get_by_name(pool, name).await? else {
        return Err(LauncherError::InvalidSettingsProfile {
            reason: format!("profile '{name}' not found"),
        });
    };

    let mut profile = GameSettingsProfile::from_row(row)?;
    update.apply(&mut profile);

    upsert_named_profile(pool, &profile).await
}

pub async fn save_global_profile(
    settings: &RwLock<LauncherSettings>,
    global: GameSettingsProfile,
) -> LauncherResult<()> {
    {
        let mut lock = settings.write();
        lock.global_game_settings = global;
    }

    let snapshot = settings.read().clone();
    save_settings(&snapshot).await
}
