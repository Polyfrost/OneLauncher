use launcher_core::game::client::{ClientType, Instance, Manifest};
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
    let uuid = manager.create_instance(name, version, cover, group, client).await?;
    Ok(uuid)
}

#[tauri::command]
pub async fn get_instances(
    state: State<'_, Mutex<GameManagerState>>
) -> Result<Vec<Instance>, String> {
    let manager = &mut state.lock().await.client_manager;
    Ok(manager.get_instances_owned())
}

#[tauri::command]
pub async fn get_instance(
    state: State<'_, Mutex<GameManagerState>>,
    uuid: Uuid
) -> Result<Instance, String> {
    let manager = &mut state.lock().await.client_manager;
    let instance = manager.get_instance(uuid)?;
    Ok(instance.clone())
}

#[tauri::command]
pub async fn get_manifest(
    state: State<'_, Mutex<GameManagerState>>,
    uuid: Uuid
) -> Result<Manifest, String> {
    let manager = &mut state.lock().await.client_manager;
    let manifest = manager.get_manifest(uuid)?;
    Ok(manifest.clone())
}