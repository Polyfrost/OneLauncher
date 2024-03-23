use tauri::{plugin::TauriPlugin, Manager, State};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::client::{ClientManager, ClientType, Instance};

pub struct GameManagerState {
    pub client_manager: ClientManager,
}

unsafe impl Send for GameManagerState {}
unsafe impl Sync for GameManagerState {}

pub fn init() -> TauriPlugin<tauri::Wry> {
	tauri::plugin::Builder::new("game")
		.setup(|app, _| {
			app.manage(Mutex::new(GameManagerState {
                client_manager: ClientManager::new(app).expect("Failed to initialize client manager"),
            }));
			Ok(())
		})
		.invoke_handler(tauri::generate_handler![
            create_instance,
            get_instances,
        ])
		.build()
}

#[tauri::command]
async fn create_instance(
    state: State<'_, Mutex<GameManagerState>>,
    name: String,
    version: String,
    cover: Option<String>,
    group: Option<Uuid>,
    client: ClientType
) -> Result<Uuid, String> {
    let manager = &mut state.lock().await.client_manager;

    manager.create_instance(name, version, cover, group, client).await.or_else(|e| {
        Err(e.to_string())
    })
}

#[tauri::command]
async fn get_instances(state: State<'_, Mutex<GameManagerState>>) -> Result<Vec<Instance>, String> {
    let manager = &mut state.lock().await.client_manager;

    Ok(manager.get_instances_owned())
}
