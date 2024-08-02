use api::statics::get_program_info;
use tauri::{Emitter, Manager};

pub mod api;
pub mod error;
pub mod ext;

#[derive(Clone, serde::Serialize)]
struct SingleInstancePayload {
	args: Vec<String>,
	cwd: String,
}

#[tracing::instrument(skip_all)]
async fn initialize_state(app: tauri::AppHandle) -> api::Result<()> {
	onelauncher::ProxyState::initialize(app).await?;
	let s = onelauncher::State::get().await?;
	onelauncher::State::update();

	s.processor.write().await.restore().await?;
	Ok(())
}

pub async fn run() {
	// initializes the logger and runs the app. if the logger fails to initialize
	// we panic because nothing else can be debugged once the logger fails.
	// the only thing that can fail before the logger should be our `tokio::main` loop.
	let _log_guard = onelauncher::start_logger();
	tracing::info!("initialized tracing subscriber. loading onelauncher...");

	run_app(|app| {
		if let Err(err) = setup(app) {
			tracing::error!("failed to setup app: {:?}", err);
		}
	})
	.await;
}

pub async fn run_app<F: FnOnce(&mut tauri::App) + Send + 'static>(setup: F) {
	let prebuild = tauri_specta::Builder::<tauri::Wry>::new()
		.commands(collect_commands!())
		.events(collect_events!())
		.constant("PROGRAM_INFO", get_program_info());

	#[cfg(debug_assertions)]
	prebuild
		.export(
			specta_typescript::Typescript::default()
				.bigint(specta_typescript::BigIntExportBehavior::BigInt),
			"../frontend/src/bindings.ts",
		)
		.expect("failed to export debug bindings!");

	let builder = tauri::Builder::default()
		.plugin(tauri_plugin_shell::init())
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
		// TODO: add back tauri-plugin-window-state -- it's buggy at the moment
		// .plugin(tauri_plugin_window_state::Builder::default().build())
		.plugin(api::init())
		.menu(tauri::menu::Menu::new)
		.invoke_handler(prebuild.invoke_handler())
		.setup(move |app| {
			prebuild.mount_events(app.handle());
			setup(app);
			Ok(())
		});

	let app = builder
		.build(tauri::generate_context!())
		.expect("failed to build tauri application");

	if let Err(err) = initialize_state(app.app_handle().clone()).await {
		tracing::error!("{err}");
	};

	app.run(|_, _| {})
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
	let win = app.get_webview_window("main").unwrap();
	win.show().unwrap();

	Ok(())
}

// TODO: Add tests
// #[cfg(test)]
// mod tests {
// 	use tauri::Manager;

// 	#[tokio::test]
// 	async fn run_app() {
// 		super::run_app(tauri::test::mock_builder(), |app| {
// 			super::setup(app);

// 			let win = app.get_webview_window("main").unwrap();
// 			tokio::spawn(async move {
// 				tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
// 				win.close().unwrap();
// 			});
// 		})
// 		.await
// 	}
// }
