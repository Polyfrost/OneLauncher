use parking_lot::RwLock;

use crate::{LauncherError, LauncherResult};
use oneclient_common::paths;
use oneclient_events::EventBus;

use super::launcher::LauncherSettings;
use oneclient_cluster::GameSettingsProfile;

#[tracing::instrument(level = "debug", skip(notify))]
pub async fn load_settings(notify: Option<&EventBus>) -> LauncherSettings {
    let settings = load_settings_file(notify).await;
    with_overrides(settings, env_overrides())
}

fn env_overrides() -> Vec<(String, String)> {
    std::env::vars_os()
        .filter_map(|(key, value)| {
            let key = key.to_str()?.strip_prefix(ENV_PREFIX)?.to_ascii_lowercase();
            Some((key, value.into_string().ok()?))
        })
        .collect()
}

/// Any `settings.json` key can be overridden with `ONECLIENT_SETTING_<KEY>`
pub const ENV_PREFIX: &str = "ONECLIENT_SETTING_";

/// Values are read as JSON and fall back to a plain string, so
/// `ONECLIENT_SETTING_THEME=light` and `ONECLIENT_SETTING_MAX_CONCURRENT_REQUESTS=10` both work
fn with_overrides(
    settings: LauncherSettings,
    overrides: impl IntoIterator<Item = (String, String)>,
) -> LauncherSettings {
    let mut overrides = overrides.into_iter().peekable();
    if overrides.peek().is_none() {
        return settings;
    }
    let Ok(serde_json::Value::Object(mut fields)) = serde_json::to_value(&settings) else {
        return settings;
    };

    for (key, raw) in overrides {
        let Some(value) = serde_json::from_str(&raw)
            .ok()
            .into_iter()
            .chain([serde_json::Value::String(raw)])
            .find(|value| fits(&key, value).is_ok())
        else {
            tracing::warn!(
                "ignored invalid {ENV_PREFIX}{}, keeping the saved `{key}`",
                key.to_ascii_uppercase()
            );
            continue;
        };
        tracing::info!(
            "setting `{key}` overridden by {ENV_PREFIX}{}",
            key.to_ascii_uppercase()
        );
        fields.insert(key, value);
    }

    salvaged_settings(serde_json::Value::Object(fields))
}

async fn load_settings_file(notify: Option<&EventBus>) -> LauncherSettings {
    match async {
        let path = paths::settings_file()?;
        let exists = polyio::try_exists(&path).await?;

        Ok::<LauncherSettings, LauncherError>(if !exists {
            LauncherSettings::default()
        } else {
            let data = polyio::read(&path).await?;
            let mut settings =
                salvaged_settings(serde_json::from_slice::<serde_json::Value>(&data)?);
            settings.migrate();
            settings
        })
    }
    .await
    {
        Ok(settings) => settings,
        Err(err) => {
            tracing::warn!("failed to read settings file: {err}");

            if let Some(notify) = notify {
                notify
                    .notify("Settings")
                    .body("Failed to load settings")
                    .error()
                    .send();
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
            settings = salvaged_settings(value);
            settings.migrate();
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

/// Keeps every field that parses on its own so one bad value only resets itself
fn salvaged_settings(value: serde_json::Value) -> LauncherSettings {
    let serde_json::Value::Object(fields) = value else {
        tracing::warn!("settings file is not a JSON object");
        return LauncherSettings::default();
    };

    let kept: serde_json::Map<_, _> = fields
        .into_iter()
        .filter(|(key, field)| {
            let fits = fits(key, field);
            if let Err(err) = &fits {
                tracing::warn!("reset setting `{key}` to its default: {err}");
            }
            fits.is_ok()
        })
        .collect();

    serde_json::from_value(serde_json::Value::Object(kept)).unwrap_or_default()
}

fn fits(key: &str, field: &serde_json::Value) -> serde_json::Result<LauncherSettings> {
    let single = serde_json::Value::Object([(key.to_owned(), field.clone())].into_iter().collect());
    serde_json::from_value(single)
}

#[tracing::instrument(level = "debug", skip(settings))]
pub async fn save_settings(settings: &LauncherSettings) -> LauncherResult<()> {
    let path = paths::settings_file()?;

    let overridden: Vec<String> = env_overrides().into_iter().map(|(key, _)| key).collect();
    let stored = if overridden.is_empty() {
        None
    } else {
        polyio::read(&path)
            .await
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
    };
    let data = serde_json::to_string_pretty(&without_overrides(settings, stored, overridden)?)?;
    polyio::write_atomic(path, data).await?;
    Ok(())
}

/// Environment overrides only live in memory, so overridden keys keep whatever the file had
fn without_overrides(
    settings: &LauncherSettings,
    stored: Option<serde_json::Value>,
    overridden: impl IntoIterator<Item = String>,
) -> serde_json::Result<serde_json::Value> {
    let mut value = serde_json::to_value(settings)?;
    let stored = match stored {
        Some(serde_json::Value::Object(stored)) => stored,
        _ => serde_json::Map::new(),
    };
    if let serde_json::Value::Object(fields) = &mut value {
        for key in overridden {
            match stored.get(&key) {
                Some(saved) => fields.insert(key, saved.clone()),
                None => fields.remove(&key),
            };
        }
    }
    Ok(value)
}

#[tracing::instrument(level = "debug", skip(services, settings))]
pub async fn save_settings_and_apply(
    services: &crate::LauncherServices,
    settings: &LauncherSettings,
) -> LauncherResult<()> {
    save_settings(settings).await?;
    let config = super::net_config(settings);
    oneclient_polyplus::set_base_url(&config.plus_api_url);
    services.requester.set_config(config);
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

    fn salvage(raw: &str) -> LauncherSettings {
        salvaged_settings(serde_json::from_str(raw).expect("valid json"))
    }

    #[test]
    fn one_bad_field_does_not_cost_the_user_their_library() {
        let raw = r#"{"max_concurrent_requests":"25","theme":42,"log_debug":true,"data_dir":"D:\\OneClient"}"#;
        let settings = salvage(raw);

        assert_eq!(
            settings.data_dir,
            Some(std::path::PathBuf::from("D:\\OneClient")),
            "the location has to survive whatever else is wrong with the file"
        );
        assert!(settings.log_debug);
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.max_concurrent_requests, 25);
    }

    #[test]
    fn the_default_location_stays_the_default() {
        assert_eq!(salvage(r#"{"log_debug":true}"#).data_dir, None);
        assert_eq!(salvage(r#"{"data_dir":null}"#).data_dir, None);
        assert_eq!(salvage(r#"{"data_dir":42}"#).data_dir, None);
    }

    #[test]
    fn env_overrides_are_never_saved() {
        let key = || "max_concurrent_requests".to_owned();
        for (saved, env, effective) in [(25, "7", 7), (41, "typo", 41)] {
            let stored = serde_json::json!({ "max_concurrent_requests": saved });
            let settings = with_overrides(salvage(&stored.to_string()), [(key(), env.to_owned())]);
            assert_eq!(settings.max_concurrent_requests, effective);

            let written = without_overrides(&settings, Some(stored), [key()]).unwrap();
            assert_eq!(written["max_concurrent_requests"], saved);
        }
    }
}
