use parking_lot::RwLock;

use oneclient_events::EventBus;
use oneclient_common::paths;
use oneclient_db::DbPool;
use crate::{LauncherError, LauncherResult};

use super::launcher::{LauncherSettings, SETTINGS_VERSION};
use oneclient_cluster::GameSettingsProfile;

#[tracing::instrument(level = "debug", skip(notify))]
pub async fn load_settings(notify: Option<&EventBus>) -> LauncherSettings {
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
                notify.notify("Settings").body("Failed to load settings").error().send();
            }

            LauncherSettings::default()
        }
    }
}

const LEGACY_MEM_MAX: u32 = 4096;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn migrate(pool: &DbPool, settings: &mut LauncherSettings) -> LauncherResult<()> {
    if !migrate_settings(settings) {
        return Ok(());
    }

    let mem_max = oneclient_common::default_mem_max();
    if mem_max != LEGACY_MEM_MAX {
        let profiles =
            oneclient_db::dao::setting_profile::replace_mem_max(pool, LEGACY_MEM_MAX, mem_max)
                .await?;

        tracing::info!("lowered the default heap to {mem_max}MB on {profiles} cluster profiles");
    }

    save_settings(settings).await
}

fn migrate_settings(settings: &mut LauncherSettings) -> bool {
    if settings.settings_version >= SETTINGS_VERSION {
        return false;
    }

    if settings.global_game_settings.mem_max == Some(LEGACY_MEM_MAX) {
        settings.global_game_settings.mem_max = Some(oneclient_common::default_mem_max());
    }

    settings.settings_version = SETTINGS_VERSION;
    true
}

/// Prefer [`save_settings_and_apply`] this leaves the HTTP client on its old
/// endpoints/keys so those changes only take effect on the next launch
#[tracing::instrument(level = "debug", skip(settings))]
pub async fn save_settings(settings: &LauncherSettings) -> LauncherResult<()> {
    let path = paths::settings_file()?;

    let data = serde_json::to_string_pretty(settings)?;
    polyio::write_atomic(path, data).await?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip(services, settings))]
pub async fn save_settings_and_apply(
    services: &crate::LauncherServices,
    settings: &LauncherSettings,
) -> LauncherResult<()> {
    save_settings(settings).await?;
    services.requester.set_config(super::net_config(settings));
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[cfg(test)]
mod tests {
    use super::{GameSettingsProfile, LauncherSettings, SETTINGS_VERSION, migrate_settings};

    fn v1(mem_max: Option<u32>) -> LauncherSettings {
        LauncherSettings {
            settings_version: 1,
            global_game_settings: GameSettingsProfile {
                mem_max,
                ..LauncherSettings::default().global_game_settings
            },
            ..LauncherSettings::default()
        }
    }

    #[test]
    fn the_old_hardcoded_heap_becomes_the_per_machine_default() {
        let mut settings = v1(Some(4096));
        migrate_settings(&mut settings);

        assert_eq!(
            settings.global_game_settings.mem_max,
            Some(oneclient_common::default_mem_max())
        );
        assert_eq!(settings.settings_version, SETTINGS_VERSION);
    }

    #[test]
    fn a_heap_the_user_chose_survives() {
        let mut settings = v1(Some(8192));
        migrate_settings(&mut settings);

        assert_eq!(settings.global_game_settings.mem_max, Some(8192));
    }

    #[test]
    fn an_already_migrated_file_is_left_alone() {
        let mut settings = v1(Some(4096));
        settings.settings_version = SETTINGS_VERSION;
        migrate_settings(&mut settings);

        assert_eq!(settings.global_game_settings.mem_max, Some(4096));
    }
}
