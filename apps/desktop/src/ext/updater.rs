use tauri::plugin::TauriPlugin;
use tauri::{Emitter, Runtime};
use tauri_plugin_updater::{Update as TauriPluginUpdate, UpdaterExt};
use tokio::sync::Mutex;

#[derive(Debug, Clone, specta::Type, serde::Serialize)]
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
pub struct State {
	install_lock: Mutex<()>,
}

async fn get_update(app: tauri::AppHandle) -> Result<Option<TauriPluginUpdate>, String> {
	app.updater_builder()
		.header("X-OneLauncher-Version", "stable")
		.map_err(|e| e.to_string())?
		.build()
		.map_err(|e| e.to_string())?
		.check()
		.await
		.map_err(|e| e.to_string())
}

// TODO: this should be a specta event
#[derive(Debug, Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum UpdateEvent {
	Loading,
	Error(String),
	UpdateAvailable { update: Update },
	NoUpdateAvailable,
	Installing,
}

#[tauri::command]
#[specta::specta]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<Option<Update>, String> {
	app.emit("updater", UpdateEvent::Loading).ok();

	let update = match get_update(app.clone()).await {
		Ok(update) => update,
		Err(e) => {
			app.emit("updater", UpdateEvent::Error(e.clone())).ok();
			return Err(e);
		}
	};

	let update = update.map(|u| Update::new(&u));

	app.emit(
		"updater",
		update.clone().map_or(UpdateEvent::NoUpdateAvailable, |u| {
			UpdateEvent::UpdateAvailable { update: u }
		}),
	)
	.ok();

	Ok(update)
}

#[tauri::command]
#[specta::specta]
pub async fn install_update(
	app: tauri::AppHandle,
	state: tauri::State<'_, State>,
) -> Result<(), String> {
	let lock = match state.install_lock.try_lock() {
		Ok(lock) => lock,
		Err(_) => return Err("Update already installing".into()),
	};

	app.emit("updater", UpdateEvent::Installing).ok();

	get_update(app.clone())
		.await?
		.ok_or_else(|| "No update required".to_string())?
		.download_and_install(|_, _| {}, || {})
		.await
		.map_err(|e| e.to_string())?;

	drop(lock);

	Ok(())
}

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
	tauri::plugin::Builder::new("onelauncher-updater")
		.on_page_load(|window, _| {
			#[cfg(target_os = "linux")]
			let updater_available = false;

			#[cfg(not(target_os = "linux"))]
			let updater_available = true;

			if updater_available {
				window
					.eval("window.__LAUNCHER_UPDATER__ = true;")
					.expect("Failed to inject updater JS");
			}
		})
		.js_init_script(format!(
			r#"window.__ONELAUNCHER_VERSION__ = "{}";"#,
			onelauncher::constants::VERSION,
		))
		.build()
}
