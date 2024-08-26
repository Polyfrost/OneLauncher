use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use onelauncher::cluster::content::logger;
use onelauncher::cluster::{self};
use onelauncher::data::{Loader, PackageData};
use onelauncher::processor::DetailedProcess;
use onelauncher::store::{Cluster, ClusterPath};
use onelauncher::State;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[specta::specta]
#[tauri::command]
pub async fn get_cluster(uuid: Uuid) -> Result<Cluster, String> {
	match onelauncher::cluster::get_by_uuid(uuid).await? {
		Some(cluster) => Ok(cluster),
		None => Err("Cluster does not exist".into()),
	}
}

#[specta::specta]
#[tauri::command]
pub async fn get_clusters() -> Result<Vec<Cluster>, String> {
	Ok(onelauncher::cluster::list().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_clusters_grouped() -> Result<HashMap<String, Vec<Cluster>>, String> {
	Ok(onelauncher::cluster::list_grouped().await?)
}

#[derive(Serialize, Deserialize, Type)]
pub struct CreateCluster {
	name: String,
	mc_version: String,
	mod_loader: Loader,
	loader_version: Option<String>,
	icon: Option<PathBuf>,
	icon_url: Option<String>,
	package_data: Option<PackageData>,
	skip: Option<bool>,
	skip_watch: Option<bool>,
}

#[specta::specta]
#[tauri::command]
pub async fn create_cluster(props: CreateCluster) -> Result<Uuid, String> {
	let path = cluster::create::create_cluster(
		props.name,
		props.mc_version,
		props.mod_loader,
		props.loader_version,
		props.icon,
		props.icon_url,
		props.package_data,
		props.skip,
		props.skip_watch,
	)
	.await?;

	if let Some(cluster) = cluster::get(&path).await? {
		Ok(cluster.uuid)
	} else {
		Err("Cluster does not exist".to_string())
	}
}

/// Updates the cluster with the given UUID. The cluster only updates game setting fields
#[specta::specta]
#[tauri::command]
pub async fn edit_game_settings(uuid: Uuid, new_cluster: Cluster) -> Result<(), String> {
	let cluster_path = ClusterPath::find_by_uuid(uuid).await?;

	cluster::edit(&cluster_path, |old| {
		// Game
		old.force_fullscreen = new_cluster.force_fullscreen;
		old.resolution = new_cluster.resolution;
		old.memory = new_cluster.memory;

		// Process
		old.init_hooks = new_cluster.init_hooks.clone();

		// Java
		old.java = new_cluster.java.clone();

		async move { Ok(()) }
	})
	.await?;

	State::sync().await?;

	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn edit_cluster_meta(
	uuid: Uuid,
	name: Option<String>,
	icon_path: Option<String>,
) -> Result<(), String> {
	let cluster = cluster::get_by_uuid(uuid)
		.await?
		.ok_or("cluster does not exist")?;

	cluster::edit(&cluster.cluster_path(), move |cluster| {
		if let Some(name) = name.clone() {
			cluster.meta.name = name;
		}

		async move { Ok(()) }
	})
	.await?;

	let icon_path = icon_path.and_then(|x| PathBuf::from_str(x.as_str()).ok());
	cluster::edit_icon(&cluster.cluster_path(), icon_path.as_deref()).await?;

	State::sync().await?;

	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn remove_cluster(uuid: Uuid) -> Result<(), String> {
	let path = ClusterPath::find_by_uuid(uuid).await?;
	Ok(cluster::remove(&path).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn run_cluster(uuid: Uuid) -> Result<DetailedProcess, String> {
	let path = ClusterPath::find_by_uuid(uuid).await?;
	let c_lock = cluster::run_default(&path).await?;
	let child = &*c_lock.read().await;

	Ok(DetailedProcess::from_processor_child(child).await)
}

#[specta::specta]
#[tauri::command]
pub async fn get_cluster_logs(uuid: Uuid) -> Result<Vec<String>, String> {
	let cluster = onelauncher::cluster::get_by_uuid(uuid)
		.await?
		.ok_or("cluster not found")?;
	let logs = logger::get_logs(&cluster.cluster_path(), None)
		.await?
		.iter()
		.map(|x| x.log_file.clone())
		.collect();
	Ok(logs)
}

#[specta::specta]
#[tauri::command]
pub async fn get_cluster_log(uuid: Uuid, log_name: String) -> Result<String, String> {
	let cluster = onelauncher::cluster::get_by_uuid(uuid)
		.await?
		.ok_or("cluster not found")?;
	let log = logger::get_output_by_file(&cluster.cluster_path(), logger::LogType::Info, &log_name)
		.await?;
	Ok(log.0)
}

#[specta::specta]
#[tauri::command]
pub async fn upload_log(uuid: Uuid, log_name: String) -> Result<String, String> {
	let cluster = onelauncher::cluster::get_by_uuid(uuid)
		.await?
		.ok_or("cluster not found")?;
	let log = logger::get_output_by_file(&cluster.cluster_path(), logger::LogType::Info, &log_name)
		.await?;

	let id = logger::upload_log(&cluster.cluster_path(), log).await?;
	Ok(id)
}

#[specta::specta]
#[tauri::command]
pub async fn get_screenshots(uuid: Uuid) -> Result<Vec<String>, String> {
	let cluster = onelauncher::cluster::get_by_uuid(uuid)
		.await?
		.ok_or("cluster not found")?;

	let screenshots =
		cluster::content::screenshots::get_screenshots(&cluster.cluster_path()).await?;

	Ok(screenshots)
}

#[specta::specta]
#[tauri::command]
pub async fn get_worlds(uuid: Uuid) -> Result<Vec<String>, String> {
	let cluster = onelauncher::cluster::get_by_uuid(uuid)
		.await?
		.ok_or("cluster not found")?;

	let screenshots = cluster::content::worlds::get_worlds(&cluster.cluster_path()).await?;

	Ok(screenshots)
}
