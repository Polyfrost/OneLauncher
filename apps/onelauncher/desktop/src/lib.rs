use onelauncher_core::api::proxy::ProxyTauri;
use onelauncher_core::api::tauri::{TauriLauncherApi, TauriLauncherEventApi};
use onelauncher_core::error::LauncherResult;
use onelauncher_core::store::proxy::ProxyState;
use onelauncher_core::store::semaphore::SemaphoreStore;
use onelauncher_core::store::{Core, CoreOptions, Dirs, State};
use tauri::{Emitter, Manager};

use crate::api::commands::OneLauncherApi;

pub mod api;
pub mod constants;
pub mod ext;

#[derive(Clone, serde::Serialize)]
pub struct SingleInstancePayload {
	args: Vec<String>,
	cwd: String,
}

#[tracing::instrument]
async fn initialize_core() -> LauncherResult<()> {
	let level = if cfg!(debug_assertions) {
		tracing::Level::DEBUG
	} else {
		tracing::Level::INFO
	};

	let opts = CoreOptions {
		curseforge_api_key: Some(constants::CURSEFORGE_API_KEY.to_string()),
		launcher_name: "OneLauncher".to_string(),
		launcher_version: env!("CARGO_PKG_VERSION").to_string(),
		launcher_website: "https://polyfrost.org/".to_string(),
		discord_client_id: Some(constants::DISCORD_CLIENT_ID.to_string()),
		fetch_attempts: 3,
		logger_filter: Some(format!(
			"{}={level},onelauncher_core={level}",
			env!("CARGO_PKG_NAME")
		)),
		..Default::default()
	};

	Core::initialize(opts).await?;
	Dirs::get().await?;
	onelauncher_core::start_logger().await;
	SemaphoreStore::get().await;
	tracing::info!("initialized core modules");

	Ok(())
}

#[tracing::instrument(skip_all)]
#[allow(clippy::large_stack_frames)]
async fn initialize_tauri(builder: tauri::Builder<tauri::Wry>) -> LauncherResult<tauri::App> {
	let router = taurpc::Router::new()
		.export_config(
			specta_typescript::Typescript::default()
				.bigint(specta_typescript::BigIntExportBehavior::BigInt)
				.formatter(ext::specta::formatter)
				.header("// @ts-nocheck \n"),
		)
		.merge(api::commands::OneLauncherApiImpl.into_handler())
		.merge(onelauncher_core::api::tauri::TauriLauncherApiImpl.into_handler())
		.merge(onelauncher_core::api::tauri::TauriLauncherEventApiImpl.into_handler());

	let launcher_path = Dirs::get().await?.base_dir();
	let builder = builder
		.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
			println!("{}, {argv:?}, {cwd}", app.package_info().name);
			app.emit("single-instance", SingleInstancePayload { args: argv, cwd })
				.unwrap();
		}))
		// .plugin(tauri_plugin_updater::Builder::new().build())
		.plugin(tauri_plugin_clipboard_manager::init())
		.plugin(tauri_plugin_dialog::init())
		.plugin(tauri_plugin_deep_link::init())
		.plugin(tauri_plugin_opener::init())
		.menu(tauri::menu::Menu::default)
		.invoke_handler(router.into_handler())
		.setup(move |app| {
			app.asset_protocol_scope()
				.allow_directory(launcher_path, true)
				.unwrap();

			setup_window(app.handle()).expect("failed to setup main window");
			Ok(())
		});

	let app = builder
		.build(tauri::generate_context!())
		.expect("failed to build tauri application");

	Ok(app)
}

#[tracing::instrument(skip_all)]
async fn initialize_state(handle: &tauri::AppHandle) -> LauncherResult<()> {
	let proxy = ProxyTauri::new(handle.clone());
	ProxyState::initialize(proxy).await?;

	State::get().await?;

	onelauncher_core::api::credentials::refresh_accounts().await?;

	tracing::info!("initialized launcher successfully");
	Ok(())
}

pub async fn run() {
	initialize_core().await.expect("failed to initialize core");

	let app = initialize_tauri(tauri::Builder::default())
		.await
		.expect("failed to initialize tauri");

	initialize_state(app.handle())
		.await
		.expect("failed to initialize state");

	app.run(|_, _| {});
}

fn setup_window(handle: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
	let win = handle
		.get_webview_window("main")
		.ok_or_else(|| anyhow::anyhow!("no window called main was found"))?;

	// Initialize window decorations based on settings
	let win_clone = win;
	let _app_handle = handle.clone();
	tokio::task::spawn(async move {
		if let Ok(state) = State::get().await {
			let _settings = state.settings.read().await;
			// native_window_frame=true means use native decorations
			// native_window_frame=false means use custom frame (no decorations)
			#[cfg(target_os = "macos")]
			{
				win_clone.set_decorations(true).ok();
				let win_weak = win_clone.clone();
				_app_handle
					.run_on_main_thread(move || {
						#[cfg(target_os = "macos")]
						{
							use objc2_app_kit::{NSWindow, NSWindowButton};

							if let Ok(ns_window_ptr) = win_weak.ns_window() {
								let ns_window = ns_window_ptr.cast::<NSWindow>();
								unsafe {
									let ns_window = &*ns_window;
									if let Some(btn) =
										ns_window.standardWindowButton(NSWindowButton::CloseButton)
									{
										btn.setHidden(true);
									}
									if let Some(btn) = ns_window
										.standardWindowButton(NSWindowButton::MiniaturizeButton)
									{
										btn.setHidden(true);
									}
									if let Some(btn) =
										ns_window.standardWindowButton(NSWindowButton::ZoomButton)
									{
										btn.setHidden(true);
									}
								}
							}
						}
					})
					.ok();
			}

			#[cfg(not(target_os = "macos"))]
			win_clone
				.set_decorations(_settings.native_window_frame)
				.ok();
		}
		win_clone.show().ok();
	});

	Ok(())
}
