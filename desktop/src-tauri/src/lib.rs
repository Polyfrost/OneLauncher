use tauri::Manager;

pub mod api;
pub mod error;
pub mod ext;

// #[tracing::instrument(skip_all)]
// #[tauri::command]
// async fn initialize_state(app: tauri::AppHandle) -> api::Result<()> {
// 	onelauncher::ProxyState::initialize(app).await?;
// 	let s = onelauncher::State::get().await?;
// 	onelauncher::State::update();

// 	s.processor.write().await.restore().await?;
// 	Ok(())
// }

#[tauri::command]
fn is_dev() -> bool {
	cfg!(debug_assertions)
}

#[derive(Clone, serde::Serialize)]
struct SingleInstancePayload {
	args: Vec<String>,
	cwd: String,
}

pub async fn run() {
	// initializes the logger and runs the app. if the logger fails to initialize
	// we panic because nothing else can be debugged once the logger fails.
	// the only thing that can fail before the logger should be our `tokio::main` loop.
	let _log_guard = onelauncher::start_logger();
	tracing::info!("initialized tracing subscriber. loading onelauncher...");

	run_app(tauri::Builder::default(), |app| {
		if let Err(err) = setup(app) {
			tracing::error!("failed to setup app: {:?}", err);
		}
	})
	.await;
}

pub async fn run_app<R: tauri::Runtime, F: FnOnce(&mut tauri::App<R>) + Send + 'static>(
	builder: tauri::Builder<R>,
	setup: F,
) {
	let builder = builder
		.plugin(tauri_plugin_shell::init())
		.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
			println!("{}, {argv:?}, {cwd}", app.package_info().name);
			app.emit("single-instance", SingleInstancePayload { args: argv, cwd })
				.unwrap();
		}))
		.plugin(tauri_plugin_window_state::Builder::default().build())
		.menu(tauri::menu::Menu::new)
		.setup(move |app| {
			setup(app);
			Ok(())
		});

	let builder = builder
		.plugin(api::init())
		.invoke_handler(tauri::generate_handler![
			// initialize_state,
			is_dev,
		]);

	let app = builder
		.build(tauri::tauri_build_context!())
		.expect("failed to build tauri application");

	app.run(|_app_handle, _event| {})
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
	// todo setup deep linking once docs are done

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
