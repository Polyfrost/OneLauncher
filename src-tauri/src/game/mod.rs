use serde::{Deserialize, Serialize};
use tauri::{plugin::TauriPlugin, Manager};
use tokio::sync::Mutex;

use self::client::get_game_client;

pub mod client;
mod clients;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
unsafe impl Sync for GameManagerState {}
impl Default for GameManagerState {
    fn default() -> Self {
        GameManagerState {
            selected_client: None
        }
    }
}

pub fn init() -> TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("game")
        .setup(|app, _| {
            app.manage(Mutex::new(GameManagerState::default()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launch_game,
            set_selected_client
        ])
        .build()
}

#[tauri::command]
async fn launch_game(state: tauri::State<'_, Mutex<GameManagerState>>) -> Result<(), String> {
    let state = state.lock().await;
    let selected_client = match &state.selected_client {
        Some(client) => client,
        None => return Err("No client selected".into())
    };

    if let Err(err) = selected_client.setup().await {
        return Err(format!("An error has occurred during install: {}", err));
    }

    match selected_client.launch().await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("An error has occurred: {}", e))
    }
}

// TODO: Implement setting client by UUID, as currently this is creating a new client and setting that
#[tauri::command]
async fn set_selected_client(handle: tauri::AppHandle, state: tauri::State<'_, Mutex<GameManagerState>>, details: client::GameClientDetails) -> Result<(), String> {
    let mut state = state.lock().await;
    let client = get_game_client(handle, details);
    state.selected_client = Some(client);
    Ok(())
}