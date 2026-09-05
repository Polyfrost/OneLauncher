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

            recover_after_failed_load().await
        }
    }
}

async fn recover_after_failed_load() -> LauncherSettings {
    let mut settings = LauncherSettings::default();

    let Ok(path) = paths::settings_file() else {
        return settings;
    };

    let Ok(data) = polyio::read(&path).await else {
        return settings;
    };

    match serde_json::from_slice::<serde_json::Value>(&data) {
        Ok(value) => {
            settings.data_dir = salvaged_data_dir(&value);

            if let Some(dir) = &settings.data_dir {
                tracing::warn!(
                    data_dir = %dir.display(),
                    "settings did not parse; kept the data folder they named"
                );
            }
        }
        Err(err) => {
            tracing::warn!("settings file is not valid JSON: {err}");

            let Ok(aside) = paths::damaged_settings_file() else {
                return settings;
            };

            match polyio::rename(&path, &aside).await {
                Ok(()) => {
                    tracing::warn!(path = %aside.display(), "kept the damaged settings file")
                }
                Err(err) => {
                    tracing::warn!("could not set the damaged settings file aside: {err}")
                }
            }
        }
    }

    settings
}

fn salvaged_data_dir(value: &serde_json::Value) -> Option<std::path::PathBuf> {
    value
        .get("data_dir")
        .and_then(serde_json::Value::as_str)
        .filter(|raw| !raw.trim().is_empty())
        .map(std::path::PathBuf::from)
}

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
    use super::*;

    fn salvage(raw: &str) -> Option<std::path::PathBuf> {
        salvaged_data_dir(&serde_json::from_str(raw).expect("valid json"))
    }

    #[test]
    fn one_bad_field_does_not_cost_the_user_their_library() {
        let raw = r#"{"max_concurrent_requests":"25","data_dir":"D:\\OneClient"}"#;

        assert_eq!(
            salvage(raw),
            Some(std::path::PathBuf::from("D:\\OneClient")),
            "the location has to survive whatever else is wrong with the file"
        );
    }

    #[test]
    fn the_default_location_stays_the_default() {
        assert_eq!(salvage(r#"{"log_debug":true}"#), None);
        assert_eq!(salvage(r#"{"data_dir":null}"#), None);
        assert_eq!(salvage(r#"{"data_dir":"   "}"#), None);
        assert_eq!(salvage(r#"{"data_dir":42}"#), None);
    }
}
