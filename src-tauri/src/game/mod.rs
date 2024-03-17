use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{plugin::TauriPlugin, Manager};

pub mod client;
pub mod manifest;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JavaVersion {
    V8,
    V16,
    V17
}

impl ToString for JavaVersion {
    fn to_string(&self) -> String {
        match self {
            JavaVersion::V8 => "8".to_string(),
            JavaVersion::V16 => "16".to_string(),
            JavaVersion::V17 => "17".to_string()
        }
    }
}

// Tauri plugin for the game module
pub struct GameManagerState {
    pub selected_client: Option<Box<dyn client::GameClient>>,
}

unsafe impl Send for GameManagerState {}

impl Default for GameManagerState {
    fn default() -> Self {
        GameManagerState {
            selected_client: None
        }
    }
}

pub fn init<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("game")
        .setup(|app, _| {
            app.manage(Mutex::new(GameManagerState::default()));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launch_game
        ])
        .build()
}

enum LaunchGameError {
    NoGameSelected
}

#[tauri::command]
async fn launch_game<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), u8> {
    let state = app.state::<Mutex<GameManagerState>>();
    let state = state.lock().unwrap();
    match state.selected_client {
        Some(ref client) => println!("Launching game: {:?}", client.get_manifest().name),
        None => println!("No game selected")
    }

    Err(LaunchGameError::NoGameSelected as u8)
}