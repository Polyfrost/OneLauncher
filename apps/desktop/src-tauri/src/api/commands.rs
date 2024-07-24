use std::path::PathBuf;

use interpulse::api::minecraft::Version;
use onelauncher::constants::{NATIVE_ARCH, TARGET_OS, VERSION};
use onelauncher::data::{Loader, MinecraftCredentials, PackageData, Settings};
use onelauncher::store::{Cluster, ClusterPath, MinecraftLogin};
use onelauncher::{cluster, minecraft, processor, settings};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[macro_export]
macro_rules! collect_commands {
	() => {{
		use $crate::api::commands::*;
		tauri_specta::ts::builder()
			.config(
				specta::ts::ExportConfig::default()
					.bigint(specta::ts::BigIntExportBehavior::BigInt),
			)
			.commands(tauri_specta::collect_commands![
				// User
				begin_msa,
				finish_msa,
				get_users,
				get_user,
				remove_user,
				// Cluster
				create_cluster,
				remove_cluster,
				get_cluster,
				get_clusters,
				run_cluster,
				// Processor
				get_running_clusters,
				get_processes_by_path,
				kill_process,
				// Settings
				get_settings,
				set_settings,
				// Metadata
				get_minecraft_versions,
				// Other
				get_program_info,
			])
	}};
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

	if let Some(cluster) = cluster::get(&path, None).await? {
		Ok(cluster.uuid)
	} else {
		Err("Cluster does not exist".to_string())
	}
}

#[specta::specta]
#[tauri::command]
pub async fn remove_cluster(uuid: Uuid) -> Result<(), String> {
	let path = ClusterPath::find_by_uuid(uuid).await?;
	Ok(cluster::remove(&path).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn run_cluster(uuid: Uuid) -> Result<(Uuid, u32), String> {
	let path = ClusterPath::find_by_uuid(uuid).await?;
	let c_lock = cluster::run(&path).await?;

	let p_uuid = c_lock.read().await.uuid;
	let p_pid = c_lock
		.read()
		.await
		.current_child
		.read()
		.await
		.id()
		.unwrap_or(0);

    // tokio::task::spawn(async move {
    //     let mut proc = c_lock.write().await;
	//     if let Err(err) = processor::wait_for(&mut proc).await {
    //         tracing::error!("Error waiting for process: {:?}", err);
    //     };
    // });

    // let mut proc = c_lock.write().await;
	// processor::wait_for(&mut proc).await?;

	Ok((p_uuid, p_pid))
}

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

// #[specta::specta]
// #[tauri::command]
// pub fn update_cluster(cluster: Cluster) -> Result<(), String> {

// }

#[specta::specta]
#[tauri::command]
pub async fn get_cluster(uuid: Uuid) -> Result<Cluster, String> {
	match cluster::get_by_uuid(uuid, None).await? {
		Some(cluster) => Ok(cluster),
		None => Err("Cluster does not exist".into()),
	}
}

#[specta::specta]
#[tauri::command]
pub async fn get_clusters() -> Result<Vec<Cluster>, String> {
	// let mut map = HashMap::<ClusterPath, Cluster>::new();
	// let cluster = placeholder_cluster();
	// map.insert(cluster.cluster_path(), cluster);

	// Ok(map)

	Ok(cluster::list(None).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_minecraft_versions() -> Result<Vec<Version>, String> {
	Ok(onelauncher::api::metadata::get_minecraft_versions()
		.await?
		.versions)
}

#[specta::specta]
#[tauri::command]
pub async fn get_settings() -> Result<Settings, String> {
	Ok(settings::get().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn set_settings(settings: Settings) -> Result<(), String> {
	Ok(settings::set(settings).await?)
}

#[derive(Serialize, Deserialize, Type)]
pub struct ProgramInfo {
	launcher_version: String,
	webview_version: String,
	tauri_version: String,
	dev_build: bool,
	platform: String,
	arch: String,
}

#[specta::specta]
#[tauri::command]
pub fn get_program_info() -> ProgramInfo {
	let webview_version = tauri::webview_version().unwrap_or("UNKNOWN".into());
	let tauri_version = tauri::VERSION;
	let dev_build = tauri::is_dev();

	ProgramInfo {
		launcher_version: VERSION.into(),
		webview_version,
		tauri_version: tauri_version.into(),
		dev_build,
		platform: TARGET_OS.into(),
		arch: NATIVE_ARCH.into(),
	}
}

#[specta::specta]
#[tauri::command]
pub async fn get_users() -> Result<Vec<MinecraftCredentials>, String> {
	Ok(minecraft::users().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_user(uuid: Uuid) -> Result<MinecraftCredentials, String> {
	Ok(minecraft::get_user(uuid).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn begin_msa() -> Result<MinecraftLogin, String> {
	Ok(minecraft::begin().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn finish_msa(
	code: String,
	login: MinecraftLogin,
) -> Result<MinecraftCredentials, String> {
	Ok(minecraft::finish(code.as_str(), login).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn remove_user(uuid: Uuid) -> Result<(), String> {
	Ok(minecraft::remove_user(uuid).await?)
}

// #[specta::specta]
// #[tauri::command]
// pub async fn searchMods(query: String) -> Result<Vec<ModrinthMod>, String> {
//     // Ok(onelauncher::api::)
// }
