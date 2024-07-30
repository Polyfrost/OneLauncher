use std::path::PathBuf;
use std::str::FromStr;

use interpulse::api::minecraft::Version;
use onelauncher::data::{Loader, ManagedPackage, MinecraftCredentials, PackageData, Settings};
use onelauncher::package::content;
use onelauncher::store::{Cluster, ClusterPath};
use onelauncher::{cluster, minecraft, processor, settings, State};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[macro_export]
macro_rules! collect_commands {
	() => {{
		use $crate::api::commands::*;
		use $crate::ext::updater::*;
		tauri_specta::collect_commands![
			// User
			auth_login,
			get_users,
			get_user,
			get_default_user,
			set_default_user,
			remove_user,
			// Cluster
			create_cluster,
			edit_cluster,
			remove_cluster,
			get_cluster,
			get_clusters,
			run_cluster,
			get_cluster_logs,
			get_cluster_log,
			// Processor
			get_running_clusters,
			get_processes_by_path,
			kill_process,
			// Settings
			get_settings,
			set_settings,
			// Metadata
			get_minecraft_versions,
			// Package
			random_mods,
			get_mod,
			download_mod,
			// Updater
			check_for_update,
			install_update,
		]
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
pub async fn edit_cluster(
	uuid: Uuid,
	name: Option<String>,
	icon_path: Option<String>,
) -> Result<(), String> {
	let cluster = cluster::get_by_uuid(uuid, None)
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
pub async fn get_cluster_logs(uuid: Uuid) -> Result<Vec<String>, String> {
	let cluster = cluster::get_by_uuid(uuid, None).await?.ok_or("cluster not found")?;
	let logs = cluster::get_logs(&cluster.cluster_path()).await?;
	Ok(logs)
}

#[specta::specta]
#[tauri::command]
pub async fn get_cluster_log(uuid: Uuid, log_name: String) -> Result<String, String> {
	let cluster = cluster::get_by_uuid(uuid, None).await?.ok_or("cluster not found")?;
	let log = cluster::get_log(&cluster.cluster_path(), log_name).await?;
	Ok(log)
}

#[specta::specta]
#[tauri::command]
pub async fn get_clusters() -> Result<Vec<Cluster>, String> {
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
pub async fn get_default_user() -> Result<Option<MinecraftCredentials>, String> {
	let uuid = minecraft::get_default_user().await?;

	match uuid {
		Some(uuid) => Ok(Some(minecraft::get_user(uuid).await?)),
		None => Ok(None),
	}
}

#[specta::specta]
#[tauri::command]
pub async fn set_default_user(uuid: Uuid) -> Result<(), String> {
	minecraft::set_default_user(uuid).await?;
	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn auth_login(handle: AppHandle) -> Result<Option<MinecraftCredentials>, String> {
	let flow = minecraft::begin().await?;
	let now = chrono::Utc::now();

	if let Some(win) = handle.get_webview_window("login") {
		win.close().map_err(|err| err.to_string())?;
	}

	let win = tauri::WebviewWindowBuilder::new(
		&handle,
		"login",
		tauri::WebviewUrl::External(
			flow.redirect_uri
				.parse()
				.map_err(|_| anyhow::anyhow!("failed to parse auth redirect url"))
				.map_err(|err| err.to_string())?,
		),
	)
	.title("Log into OneLauncher")
	.always_on_top(true)
	.center()
	.build()
	.map_err(|err| err.to_string())?;

	win.request_user_attention(Some(tauri::UserAttentionType::Critical))
		.map_err(|err| err.to_string())?;

	while (chrono::Utc::now() - now) < chrono::Duration::minutes(10) {
		if win.title().is_err() {
			return Ok(None);
		}

		if win
			.url()
			.map_err(|err| err.to_string())?
			.as_str()
			.starts_with("https://login.live.com/oauth20_desktop.srf")
		{
			if let Some((_, code)) = win
				.url()
				.map_err(|err| err.to_string())?
				.query_pairs()
				.find(|x| x.0 == "code")
			{
				win.close().map_err(|err| err.to_string())?;
				let value = minecraft::finish(&code.clone(), flow).await?;

				return Ok(Some(value));
			}
		}

		tokio::time::sleep(std::time::Duration::from_millis(50)).await;
	}

	win.close().map_err(|err| err.to_string())?;

	Ok(None)
}

#[specta::specta]
#[tauri::command]
pub async fn remove_user(uuid: Uuid) -> Result<(), String> {
	Ok(minecraft::remove_user(uuid).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn random_mods() -> Result<Vec<ManagedPackage>, String> {
	let provider = content::Providers::Modrinth;
	Ok(provider.list().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_mod(project_id: String) -> Result<ManagedPackage, String> {
	let provider = content::Providers::Modrinth;
	Ok(provider.get(&project_id).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn download_mod(cluster_id: Uuid, version_id: String) -> Result<(), String> {
	let cluster = cluster::get_by_uuid(cluster_id, None)
		.await?
		.ok_or("cluster not found")?;
	let provider = content::Providers::Modrinth;
	let game_version = cluster.meta.mc_version.clone();

	provider
		.get_version_for_game_version(&version_id, &game_version)
		.await?
		.files
		.first()
		.ok_or("no files found")?
		.download_to_cluster(&cluster)
		.await?;

	Ok(())
}
