// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use polyfrost_launcher::auth;
use tauri::{App, Manager};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(auth::init())
        .setup(setup)
        .on_window_event(window_handler)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn window_handler(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.emit("handle_window_close_request", ());
        }
        _ => {}
    }
}

fn setup(_: &mut App) -> Result<(), Box<dyn Error>> {
    Ok(())
}
