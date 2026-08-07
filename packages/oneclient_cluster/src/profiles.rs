//! A profile is a sparse override on the global baseline a `None` field
//! inherits so `Patch::Clear` means "inherit again" not "set to nothing"
//! The baseline is passed in so this crate needn't depend on launcher config

use oneclient_common::patch::Patch;
use oneclient_db::DbPool;

use crate::error::{ClusterError, ClusterResult};
use crate::profile::{GameSettingsProfile, SettingsOsExtra};
use oneclient_common::Resolution;
use oneclient_common::domain::PackageUpdateMode;

#[tracing::instrument(level = "debug", skip(pool, global))]
pub async fn get_profile_or_default(
    pool: &DbPool,
    global: &GameSettingsProfile,
    name: Option<&str>,
) -> ClusterResult<GameSettingsProfile> {
    if let Some(name) = name
        && let Some(row) = oneclient_db::dao::setting_profile::get_by_name(pool, name).await?
    {
        return GameSettingsProfile::from_row(row);
    }

    Ok(global.clone())
}

#[tracing::instrument(level = "debug", skip(pool, global))]
/// Falls back to the whole baseline when the name is missing or unknown so a
/// deleted profile cannot brick a cluster
pub async fn resolve_cluster_profile(
    pool: &DbPool,
    global: &GameSettingsProfile,
    profile_name: Option<&str>,
) -> ClusterResult<GameSettingsProfile> {
    let Some(name) = profile_name else {
        return Ok(global.clone());
    };

    let Some(row) = oneclient_db::dao::setting_profile::get_by_name(pool, name).await? else {
        return Ok(global.clone());
    };

    let mut profile = GameSettingsProfile::from_row(row)?;
    if !profile.is_global() {
        profile.merge_global(global);
    }

    Ok(profile)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn upsert_named_profile(
    pool: &DbPool,
    profile: &GameSettingsProfile,
) -> ClusterResult<GameSettingsProfile> {
    if profile.is_global() {
        return Err(ClusterError::InvalidProfile {
            reason: "the Global profile is stored in settings.json".into(),
        });
    }

    let row = profile.into_row()?;
    let saved = oneclient_db::dao::setting_profile::upsert(pool, &row).await?;
    GameSettingsProfile::from_row(saved)
}

#[tracing::instrument(level = "debug", skip(pool, global))]
pub async fn create_profile_from_global(
    pool: &DbPool,
    global: &GameSettingsProfile,
    name: &str,
    mem_max: Option<u32>,
    force_fullscreen: Option<bool>,
) -> ClusterResult<GameSettingsProfile> {
    let mut profile = global.clone();

    profile.name = name.to_string();
    if let Some(mem) = mem_max {
        profile.mem_max = Some(mem);
    }

    if let Some(fullscreen) = force_fullscreen {
        profile.force_fullscreen = Some(fullscreen);
    }

    upsert_named_profile(pool, &profile).await
}

#[tracing::instrument(level = "debug", skip(pool, global))]
pub async fn create_settings_profile(
    pool: &DbPool,
    global: &GameSettingsProfile,
    name: &str,
) -> ClusterResult<GameSettingsProfile> {
    create_profile_from_global(pool, global, name, None, None).await
}

#[tracing::instrument(level = "debug", skip(pool))]
pub async fn list_named_profiles(pool: &DbPool) -> ClusterResult<Vec<GameSettingsProfile>> {
    let rows = oneclient_db::dao::setting_profile::list_all(pool).await?;
    rows.into_iter()
        .map(GameSettingsProfile::from_row)
        .collect()
}

#[tracing::instrument(level = "debug", skip(pool))]
pub async fn delete_named_profile(pool: &DbPool, name: &str) -> ClusterResult<()> {
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
    pub browser_update_mode: Patch<PackageUpdateMode>,
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
        self.browser_update_mode
            .apply_to_option(&mut profile.browser_update_mode);
    }
}

#[tracing::instrument(level = "debug", skip(pool, update))]
pub async fn update_named_profile(
    pool: &DbPool,
    name: &str,
    update: ProfileUpdate,
) -> ClusterResult<GameSettingsProfile> {
    let Some(row) = oneclient_db::dao::setting_profile::get_by_name(pool, name).await? else {
        return Err(ClusterError::InvalidProfile {
            reason: format!("profile '{name}' not found"),
        });
    };

    let mut profile = GameSettingsProfile::from_row(row)?;
    update.apply(&mut profile);

    upsert_named_profile(pool, &profile).await
}

