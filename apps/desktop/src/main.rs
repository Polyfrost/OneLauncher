// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit = "1024"]

#[tokio::main]
async fn main() {
	onelauncher_gui::run().await;
}
