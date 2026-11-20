use parking_lot::RwLock;

use crate::{LauncherError, LauncherResult};
use oneclient_common::paths;
use oneclient_events::EventBus;

use super::launcher::LauncherSettings;
use oneclient_cluster::GameSettingsProfile;

static SETTINGS_WRITE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tracing::instrument(level = "debug", skip(notify))]
pub async fn load_settings(notify: Option<&EventBus>) -> LauncherSettings {
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
            let single =
                serde_json::Value::Object([(key.clone(), field.clone())].into_iter().collect());
            match serde_json::from_value::<LauncherSettings>(single) {
                Ok(_) => true,
                Err(err) => {
                    tracing::warn!("reset setting `{key}` to its default: {err}");
                    false
                }
            }
        })
        .collect();

    serde_json::from_value(serde_json::Value::Object(kept)).unwrap_or_default()
}

#[tracing::instrument(level = "debug", skip(settings))]
pub async fn save_settings(settings: &LauncherSettings) -> LauncherResult<()> {
    let _guard = SETTINGS_WRITE.lock().await;
    write_settings(settings).await
}

async fn write_settings(settings: &LauncherSettings) -> LauncherResult<()> {
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
    let _guard = SETTINGS_WRITE.lock().await;
    write_settings_and_apply(services, settings).await
}

async fn write_settings_and_apply(
    services: &crate::LauncherServices,
    settings: &LauncherSettings,
) -> LauncherResult<()> {
    write_settings(settings).await?;
    let config = super::net_config(settings);
    oneclient_polyplus::set_base_url(&config.plus_api_url);
    services.requester.set_config(config);
    Ok(())
}

/// Take the snapshot only after the previous save is finished. An older pending
/// task cannot overwrite a newer launcher edit with an outdated snapshot.
async fn with_current_settings<T>(
    settings: &RwLock<LauncherSettings>,
    write: impl AsyncFnOnce(&LauncherSettings) -> T,
) -> T {
    let _guard = SETTINGS_WRITE.lock().await;
    let snapshot = settings.read().clone();
    write(&snapshot).await
}

pub async fn save_current_settings(settings: &RwLock<LauncherSettings>) -> LauncherResult<()> {
    with_current_settings(settings, write_settings).await
}

pub async fn save_current_settings_and_apply(
    services: &crate::LauncherServices,
    settings: &RwLock<LauncherSettings>,
) -> LauncherResult<()> {
    with_current_settings(settings, async |snapshot| {
        write_settings_and_apply(services, snapshot).await
    })
    .await
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

    save_current_settings(settings).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn delayed_save_cannot_overwrite_a_later_theme_selection() {
        let settings = RwLock::new(LauncherSettings::default());
        let saved = std::sync::Mutex::new(Vec::new());
        let (started, waiting_for_first) = tokio::sync::oneshot::channel();
        let (finish_first, wait_for_edit) = tokio::sync::oneshot::channel();
        let first = with_current_settings(&settings, async |snapshot| {
            started.send(()).unwrap();
            wait_for_edit.await.unwrap();
            saved.lock().unwrap().push(snapshot.theme.clone());
        });
        let second = async {
            waiting_for_first.await.unwrap();
            settings.write().theme = "system:polyglass-dark".into();
            let mut latest = std::pin::pin!(with_current_settings(&settings, async |snapshot| {
                saved.lock().unwrap().push(snapshot.theme.clone());
            }));
            assert!(
                std::future::poll_fn(|cx| {
                    std::task::Poll::Ready(latest.as_mut().poll(cx).is_pending())
                })
                .await
            );
            finish_first.send(()).unwrap();
            latest.await;
        };
        tokio::join!(first, second);
        assert_eq!(*saved.lock().unwrap(), ["dark", "system:polyglass-dark"]);
    }

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
}
