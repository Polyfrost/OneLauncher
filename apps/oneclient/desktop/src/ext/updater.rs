use onelauncher_core::store::Core;
use tauri::{Manager, Runtime};
use tauri_plugin_updater::{Update as TauriPluginUpdate, UpdaterExt};
use tokio::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct Update {
	pub version: String,
}

impl Update {
	fn new(update: &TauriPluginUpdate) -> Self {
		Self {
			version: update.version.clone(),
		}
	}
}

#[derive(Default)]
pub struct UpdaterState {
	install_lock: Mutex<()>,
}

async fn get_update<R: Runtime>(
	app: tauri::AppHandle<R>,
) -> Result<Option<TauriPluginUpdate>, String> {
	app.updater()
		.map_err(|e| e.to_string())?
		.check()
		.await
		.map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum UpdateEvent {
	Loading,
	Error { error: String },
	UpdateAvailable { update: Update },
	NoUpdateAvailable,
	Installing,
}

pub async fn check_for_update<R: Runtime>(
	app: tauri::AppHandle<R>,
) -> Result<Option<Update>, String> {
	get_update(app)
		.await
		.map(|update| update.map(|u| Update::new(&u)))
}

pub async fn install_update<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
	let state = app.state::<UpdaterState>();
	let Ok(lock) = state.install_lock.try_lock() else {
		return Err("Update already installing".into());
	};

	get_update(app.clone())
		.await?
		.ok_or_else(|| "No update required".to_string())?
		.download_and_install(|_, _| {}, || {})
		.await
		.map_err(|e| e.to_string())?;

	drop(lock);

	app.restart();

	Ok(())
}

pub fn init<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
	#[cfg(target_os = "linux")]
	let updater_available = false;

	#[cfg(not(target_os = "linux"))]
	let updater_available = true;

	if updater_available {
		if let Some(window) = app.get_webview_window("main") {
			window
				.eval("window.__LAUNCHER_UPDATER__ = true;")
				.expect("Failed to inject updater JS");

			window
				.eval(&format!(
					r#"window.__ONECLIENT_VERSION__ = "{}";"#,
					Core::get().launcher_version,
				))
				.expect("Failed to inject version JS");
		}
	}
	Ok(())
}
