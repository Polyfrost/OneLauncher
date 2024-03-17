// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use polyfrost_launcher::{auth, game::{client::{download_java, GameClient}, JavaVersion}};
use tauri::{generate_handler, menu::Menu, App, Runtime};

#[tauri::command]
async fn download_java_test<R: Runtime>(app: tauri::AppHandle<R>, window: tauri::Window<R>) {
    let res = download_java(&app, JavaVersion::V8).await;
    println!("{:#?}", res);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(auth::init())
        .invoke_handler(generate_handler![download_java_test])
        .menu(|handle| Menu::new(handle))
        .setup(setup)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup(_: &mut App) -> Result<(), Box<dyn Error>> {
    Ok(())
}
