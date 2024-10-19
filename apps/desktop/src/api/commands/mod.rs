use interpulse::api::minecraft::Version;
use onelauncher::data::Settings;
use onelauncher::settings;
use tauri::{AppHandle, Manager};

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
			edit_game_settings,
			edit_cluster_meta,
			remove_cluster,
			get_cluster,
			get_clusters,
			get_clusters_grouped,
			run_cluster,
			get_cluster_logs,
			get_cluster_log,
			upload_log,
			get_screenshots,
			get_worlds,
			// Processor
			get_running_clusters,
			get_processes_by_path,
			kill_process,
			is_cluster_running,
			get_pid_by_uuid,
			get_user_by_process,
			get_process_started_at,
			get_processes_detailed_by_path,
			get_process_detailed_by_id,
			// Settings
			get_settings,
			set_settings,
			// Metadata
			get_minecraft_versions,
			// Launcher Packages (Instances)
			import_launcher_instances,
			// Provider Packages
			get_provider_package,
			get_provider_packages,
			get_all_provider_package_versions,
			get_provider_package_versions,
			search_provider_packages,
			get_provider_authors,
			get_package_body,
			download_provider_package,
			// Cluster Packages
			get_cluster_package,
			get_cluster_packages,
			add_cluster_package,
			remove_cluster_package,
			sync_cluster_packages,
			sync_cluster_packages_by_type,
			// Updater
			check_for_update,
			install_update,
			// Other
			set_window_style,
			get_program_info
		]
	}};
}

#[specta::specta]
#[tauri::command]
pub fn get_program_info() -> Result<super::statics::ProgramInfo, String> {
	Ok(super::statics::get_program_info())
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
pub fn set_window_style(handle: AppHandle, custom: bool) -> Result<(), String> {
	let window = handle.get_webview_window("main").unwrap();
	onelauncher::utils::window::set_window_styling(&window, custom).map_err(|e| e.to_string())
}
