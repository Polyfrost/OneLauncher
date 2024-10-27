#![allow(trivial_casts)]
use onelauncher::utils::window::set_window_styling;
use tauri::{Emitter, Manager};

pub mod api;
pub mod error;
pub mod ext;

#[derive(Clone, serde::Serialize)]
pub struct SingleInstancePayload {
	args: Vec<String>,
	cwd: String,
}

#[tracing::instrument(skip_all)]
async fn initialize_state(handle: &tauri::AppHandle) -> api::Result<()> {
	onelauncher::ProxyState::initialize(handle).await?;
	let s = onelauncher::State::get().await?;
	onelauncher::State::update();
	s.processor.write().await.restore().await?;
	Ok(())
}

/// the main entrypoint to the desktop add (initializes the logger and runs the app)
///
/// if the logger fails to initialize then we will panic because
/// nothing else can be debugged once the logger fails.
///
/// the only thing that can fail before the logger should be our [`tokio::main`] loop.
pub async fn run() {
	let _log_guard = onelauncher::start_logger();
	tracing::info!("initialized logger. loading onelauncher/tauri...");

	run_app(tauri::Builder::default(), |app| {
		if let Err(err) = setup(app) {
			tracing::error!("failed to setup app: {:?}", err);
		}
	})
	.await;
}

pub async fn run_app<F: FnOnce(&tauri::AppHandle<tauri::Wry>) + Send + 'static>(
	builder: tauri::Builder<tauri::Wry>,
	setup: F,
) {
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
		.plugin(tauri_plugin_deep_link::init())
		.plugin(api::init())
		.menu(tauri::menu::Menu::new)
		.invoke_handler(prebuild.invoke_handler())
		.setup(move |app| {
			prebuild.mount_events(app.handle());
			setup(app.handle());
			Ok(())
		});

	let app = builder
		.build(tauri::generate_context!())
		.expect("failed to build tauri application");

	if let Err(err) = initialize_state(app.handle()).await {
		tracing::error!("{err}");
	};

	app.run(|_, _| {});
}

fn setup(handle: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
	let win = handle.get_webview_window("main").unwrap();

	tokio::task::spawn(async move {
		let state = onelauncher::State::get();
		let state = match state.await {
			Ok(state) => state,
			Err(err) => {
				tracing::error!("{err}");
				return;
			}
		};

		let settings = state.settings.read().await;

		if let Err(err) = set_window_styling(&win, settings.custom_frame) {
			tracing::error!(err);
		};

		win.show().unwrap();
	});

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
