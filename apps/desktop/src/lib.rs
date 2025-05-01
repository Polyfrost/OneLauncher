use onelauncher_core::{api::proxy::ProxyTauri, error::LauncherResult, store::{proxy::ProxyState, semaphore::SemaphoreStore, Core, CoreOptions, Dirs, State}};
use tauri::{Emitter, Manager};

pub mod api;
pub mod ext;
pub mod constants;

#[derive(Clone, serde::Serialize)]
pub struct SingleInstancePayload {
	args: Vec<String>,
	cwd: String,
}

#[tracing::instrument]
async fn initialize_core() -> LauncherResult<()> {
	let opts = CoreOptions {
		curseforge_api_key: Some(constants::CURSEFORGE_API_KEY.to_string()),
		launcher_name: "OneLauncher".to_string(),
		launcher_version: env!("CARGO_PKG_VERSION").to_string(),
		launcher_website: "https://polyfrost.org/".to_string(),
		discord_client_id: Some(constants::DISCORD_CLIENT_ID.to_string()),
		fetch_attempts: 3,
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
	let prebuild = tauri_specta::Builder::<tauri::Wry>::new()
		.commands(collect_commands!())
		.events(collect_events!());

	#[cfg(debug_assertions)]
	prebuild
		.export(
			specta_typescript::Typescript::default()
				.bigint(specta_typescript::BigIntExportBehavior::BigInt)
				.formatter(ext::specta::formatter),
			"../../packages/client/src/bindings.ts",
		)
		.expect("failed to export debug bindings!");

	let builder = builder
		.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
			println!("{}, {argv:?}, {cwd}", app.package_info().name);
			app.emit("single-instance", SingleInstancePayload { args: argv, cwd })
				.unwrap();
		}))
		.plugin(tauri_plugin_updater::Builder::new().build())
		.plugin(tauri_plugin_clipboard_manager::init())
		.plugin(ext::updater::plugin())
		.manage(ext::updater::State::default())
		.plugin(tauri_plugin_dialog::init())
		.plugin(tauri_plugin_deep_link::init())
		// .plugin(api::init())
		.menu(tauri::menu::Menu::new)
		.invoke_handler(prebuild.invoke_handler())
		.setup(move |app| {
			prebuild.mount_events(app.handle());
			setup_window(app.handle());
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
	let app = initialize_tauri(tauri::Builder::default()).await.expect("failed to initialize tauri");
	initialize_state(app.handle()).await.expect("failed to initialize state");

	app.run(|_, _| {});
}

fn setup_window(handle: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
	let win = handle.get_webview_window("main").unwrap();

	// tokio::task::spawn(async move {
	// 	// let state = State::get().await.expect("failed to get state");
	// 	// let settings = state.settings.read().await;
	// 	// win.set_decorations(settings.);
	// });


	win.show().unwrap();

	Ok(())
}