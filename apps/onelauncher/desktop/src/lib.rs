use onelauncher_core::api::proxy::ProxyTauri;
use onelauncher_core::api::tauri::{TauriLauncherEventApi, TauriLauncherApi};
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
		logger_filter: Some(
			format!("{}={level},onelauncher_core={level}", env!("CARGO_PKG_NAME")),
		),
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

	let builder = builder
		.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
			println!("{}, {argv:?}, {cwd}", app.package_info().name);
			app.emit("single-instance", SingleInstancePayload { args: argv, cwd })
				.unwrap();
		}))
		.plugin(tauri_plugin_updater::Builder::new().build())
		.plugin(tauri_plugin_clipboard_manager::init())
		.plugin(tauri_plugin_dialog::init())
		.plugin(tauri_plugin_deep_link::init())
		.plugin(tauri_plugin_opener::init())
		.menu(tauri::menu::Menu::new)
		.invoke_handler(router.into_handler())
		.setup(move |app| {
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

	// tokio::task::spawn(async move {
	// 	// let state = State::get().await.expect("failed to get state");
	// 	// let settings = state.settings.read().await;
	// 	// win.set_decorations(settings.);
	// });

	win.show()?;

	Ok(())
}
