use onelauncher::processor;
use onelauncher::store::{Cluster, ClusterPath};
use uuid::Uuid;

#[specta::specta]
#[tauri::command]
pub async fn get_running_clusters() -> Result<Vec<Cluster>, String> {
	Ok(processor::get_running_clusters().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn is_cluster_running(uuid: Uuid) -> Result<bool, String> {
	Ok(processor::is_cluster_running(uuid).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_processes_by_path(path: ClusterPath) -> Result<Vec<Uuid>, String> {
	Ok(processor::get_uuids_by_cluster_path(path).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_pid_by_uuid(uuid: Uuid) -> Result<u32, String> {
	Ok(processor::get_pid_by_uuid(uuid).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn kill_process(uuid: Uuid) -> Result<(), String> {
	processor::kill_by_uuid(uuid).await?;
	Ok(())
}
