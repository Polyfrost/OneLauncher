use onelauncher::game::{client::{Cluster, Manifest}, clients::ClientType};
use tauri::State;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::GameManagerState;

#[tauri::command]
pub async fn create_cluster(
	state: State<'_, Mutex<GameManagerState>>,
	name: String,
	version: String,
	client: ClientType,
	cover: Option<String>,
	group: Option<Uuid>,
) -> Result<Uuid, String> {
	let manager = &mut state.lock().await.client_manager;
	let uuid = manager
		.create_cluster(name, version, cover, group, client)
		.await?;
	Ok(uuid)
}

#[tauri::command]
pub async fn get_clusters(
	state: State<'_, Mutex<GameManagerState>>,
) -> Result<Vec<Cluster>, String> {
	let manager = &mut state.lock().await.client_manager;
	Ok(manager.get_clusters_owned())
}

#[tauri::command]
pub async fn get_cluster(
	state: State<'_, Mutex<GameManagerState>>,
	uuid: Uuid,
) -> Result<Cluster, String> {
	let manager = &mut state.lock().await.client_manager;
	let instance = manager.get_cluster(uuid)?;
	Ok(instance.clone())
}

#[tauri::command]
pub async fn launch_cluster(
    state: State<'_, Mutex<GameManagerState>>,
	uuid: Uuid,
) -> Result<(), String> {
    let manager = &mut state.lock().await.client_manager;
    manager.launch_cluster(uuid).await?; // TODO: Change how this works
    Ok(())
}

#[tauri::command]
pub async fn get_manifest(
	state: State<'_, Mutex<GameManagerState>>,
	uuid: Uuid,
) -> Result<Manifest, String> {
	let manager = &mut state.lock().await.client_manager;
	let manifest = manager.get_manifest(uuid)?;
	Ok(manifest.clone())
}
