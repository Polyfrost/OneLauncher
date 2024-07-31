use onelauncher::{processor, store::{Cluster, ClusterPath}};
use uuid::Uuid;

#[specta::specta]
#[tauri::command]
pub async fn get_running_clusters() -> Result<Vec<Cluster>, String> {
	Ok(processor::get_running_clusters().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_processes_by_path(path: ClusterPath) -> Result<Vec<Uuid>, String> {
	Ok(processor::get_uuids_by_cluster_path(path).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn kill_process(uuid: Uuid) -> Result<(), String> {
	processor::kill_by_uuid(uuid).await?;
	Ok(())
}