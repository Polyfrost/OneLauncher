use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::DateTime;
use onelauncher::constants::{NATIVE_ARCH, TARGET_OS, VERSION};
use onelauncher::data::{ClusterMeta, Loader, MinecraftCredentials, PackageData, Settings};
use onelauncher::store::{Cluster, ClusterPath, MinecraftLogin};
use onelauncher::{cluster, minecraft, settings};
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
				begin_msa,
				finish_msa,
				get_users,
				get_user,
				remove_user,
				create_cluster,
				get_cluster,
				get_clusters,
				get_settings,
				set_settings,
				get_program_info
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

// #[specta::specta]
// #[tauri::command]
// pub fn update_cluster(cluster: Cluster) -> Result<(), String> {

// }

fn placeholder_cluster() -> Cluster {
	let path = ClusterPath("test".into());
	Cluster {
		uuid: Uuid::from_str("56d1cbcf-1961-4477-b263-80e3b1c7a9d1").unwrap(),
		stage: onelauncher::store::ClusterStage::Installed,
		path: path.0,
		meta: ClusterMeta {
			created_at: DateTime::from_timestamp_millis(1718297861712).unwrap(),
			modified_at: DateTime::from_timestamp_millis(1718297861712).unwrap(),
			group: vec![],
			icon: None,
			icon_url: None,
			loader: Loader::Vanilla,
			loader_version: None,
			mc_version: "1.8.9".into(),
			name: "Test Cluster".into(),
			overall_played: 58195,
			recently_played: 0,
			package_data: None,
			played_at: None,
		},
		memory: None,
		java: None,
		resolution: None,
		force_fullscreen: None,
		init_hooks: None,
		packages: HashMap::new(),
		update: None,
	}
}

#[specta::specta]
#[tauri::command]
pub async fn get_cluster(uuid: Uuid) -> Result<Cluster, String> {
	Ok(placeholder_cluster())

	// match cluster::get_by_uuid(uuid, None).await? {
	//     Some(cluster) => Ok(cluster),
	//     None => Err("Cluster does not exist".into())
	// }
}

#[specta::specta]
#[tauri::command]
pub async fn get_clusters() -> Result<HashMap<ClusterPath, Cluster>, String> {
	let mut map = HashMap::<ClusterPath, Cluster>::new();
	let cluster = placeholder_cluster();
	map.insert(cluster.cluster_path(), cluster);

	Ok(map)

	// cluster::list(None).await.map_err(|op| op.into())
}

#[specta::specta]
#[tauri::command]
pub async fn get_settings() -> Result<Settings, String> {
	settings::get().await.map_err(|err| err.into())
}

#[specta::specta]
#[tauri::command]
pub async fn set_settings(settings: Settings) -> Result<(), String> {
	settings::set(settings).await.map_err(|err| err.into())
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
    minecraft::users().await.map_err(|err| err.into())
}

#[specta::specta]
#[tauri::command]
pub async fn get_user(uuid: Uuid) -> Result<MinecraftCredentials, String> {
    minecraft::get_user(uuid).await.map_err(|err| err.into())
}

#[specta::specta]
#[tauri::command]
pub async fn begin_msa() -> Result<MinecraftLogin, String> {
    minecraft::begin().await.map_err(|err| err.into())
}

#[specta::specta]
#[tauri::command]
pub async fn finish_msa(code: String, login: MinecraftLogin) -> Result<MinecraftCredentials, String> {
    minecraft::finish(code.as_str(), login).await.map_err(|err| err.into())
}

#[specta::specta]
#[tauri::command]
pub async fn remove_user(uuid: Uuid) -> Result<(), String> {
    minecraft::remove_user(uuid).await.map_err(|err| err.into())
}
