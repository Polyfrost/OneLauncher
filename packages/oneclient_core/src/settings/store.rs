use parking_lot::RwLock;

use oneclient_events::EventBus;
use oneclient_common::paths;
use crate::{LauncherError, LauncherResult};

use super::launcher::LauncherSettings;
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

/// Persists settings to disk.
///
/// Prefer [`save_settings_and_apply`] where the services are in reach: this one
/// leaves the HTTP client on whatever endpoints and API keys it already had, so
/// a changed CurseForge key or custom endpoint would not take effect until the
/// next launch.
#[tracing::instrument(level = "debug", skip(settings))]
pub async fn save_settings(settings: &LauncherSettings) -> LauncherResult<()> {
    let path = paths::settings_file()?;

    let data = serde_json::to_string_pretty(settings)?;
    polyio::write_atomic(path, data).await?;
    Ok(())
}

/// Persists settings and pushes the endpoint/credential subset into the
/// transport, so an API key or endpoint change applies immediately.
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
