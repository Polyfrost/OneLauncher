// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use polyfrost_launcher::{auth, game};
use tauri::{menu::Menu, App};

#[tokio::main]
async fn main() {
	tauri::Builder::default()
		.plugin(tauri_plugin_shell::init())
		.plugin(auth::init())
		.plugin(game::plugin::init())
		.menu(|handle| Menu::new(handle))
		.setup(setup)
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}

fn setup(_: &mut App) -> Result<(), Box<dyn Error>> {
	Ok(())
}
