// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use onelauncher_gui::api;
use tauri::{menu::Menu, App, Manager};

#[derive(Clone, serde::Serialize)]
struct Payload {
	args: Vec<String>,
	cwd: String,
}

#[tokio::main]
async fn main() {
	let _log_guard = onelauncher::start_logger();
	tracing::info!("initialized tracing subscriber. loading OneLauncher");

	tauri::Builder::default()
		.plugin(tauri_plugin_shell::init())
		.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
			println!("{}, {argv:?}, {cwd}", app.package_info().name);

			app.emit("single-instance", Payload { args: argv, cwd })
				.unwrap();
		}))
		.plugin(tauri_plugin_window_state::Builder::default().build())
		.plugin(api::init())
		.menu(|handle| Menu::new(handle))
		.setup(setup)
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}

fn setup(_: &mut App) -> Result<(), Box<dyn Error>> {
	Ok(())
}
