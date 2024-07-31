use interpulse::api::minecraft::Version;
use onelauncher::data::Settings;
use onelauncher::settings;

mod cluster;
pub use crate::api::commands::cluster::*;

mod users;
pub use crate::api::commands::users::*;

mod processor;
pub use crate::api::commands::processor::*;

mod package;
pub use crate::api::commands::package::*;

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
			upload_log,
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
