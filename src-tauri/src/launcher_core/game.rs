use launcher_core::game::client::{ClientType, Instance};
use tauri::State;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::GameManagerState;

#[tauri::command]
pub async fn create_instance(
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
pub async fn get_instances(state: State<'_, Mutex<GameManagerState>>) -> Result<Vec<Instance>, String> {
    let manager = &mut state.lock().await.client_manager;

    Ok(manager.get_instances_owned())
}