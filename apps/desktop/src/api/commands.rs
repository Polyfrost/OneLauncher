use interpulse::api::minecraft::Version;
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
			// auth_login,
			// get_users,
			// get_user,
			// get_default_user,
			// set_default_user,
			// remove_user,
			// Cluster
			// create_cluster,
			// edit_game_settings,
			// edit_cluster_meta,
			// remove_cluster,
			// get_cluster,
			// get_clusters,
			// get_clusters_grouped,
			// run_cluster,
			// get_cluster_logs,
			// get_cluster_log,
			// upload_log,
			// get_screenshots,
			// get_worlds,
			// get_optimal_java_version,
			// repair_cluster,
			// Processor
			// get_running_clusters,
			// get_processes_by_path,
			// kill_process,
			// is_cluster_running,
			// get_pid_by_uuid,
			// get_user_by_process,
			// get_process_started_at,
			// get_processes_detailed_by_path,
			// get_process_detailed_by_id,
			// Settings
			// get_settings,
			// set_settings,
			// Metadata
			// get_minecraft_versions,
			// Launcher Packages (Instances)
			// get_launcher_instances,
			// import_instances,
			// Provider Packages
			// get_provider_package,
			// get_provider_packages,
			// get_all_provider_package_versions,
			// get_provider_package_versions,
			// search_provider_packages,
			// get_provider_authors,
			// get_package_body,
			// download_provider_package,
			// Cluster Packages
			// get_cluster_package,
			// get_cluster_packages,
			// add_cluster_package,
			// remove_cluster_package,
			// set_cluster_package_enabled,
			// sync_cluster_packages,
			// sync_cluster_packages_by_type,
			// updater
			// check_for_update,
			// install_update,
			// other
			// set_window_style,
			// get_program_info,
			// get_featured_packages,
			// get_zulu_packages,
			// install_java_from_package,
			// open_dev_tools,
		]
	}};
}

#[onelauncher_core::command]
pub fn open_dev_tools(_webview: tauri::WebviewWindow) {
	#[cfg(feature = "devtools")]
	_webview.open_devtools();
}

// #[onelauncher_core::command]
// pub async fn get_zulu_packages() -> Result<Vec<onelauncher::java::JavaZuluPackage>, String> {
// 	Ok(onelauncher::java::get_zulu_packages().await?)
// }

#[onelauncher_core::command]
pub async fn install_java_from_package(download: onelauncher::java::JavaZuluPackage) -> Result<std::path::PathBuf, String> {
	Ok(onelauncher::java::install_java_from_package(download).await?)
}

#[onelauncher_core::command]
pub async fn get_featured_packages() -> Result<Vec<onelauncher::package::content::FeaturedPackage>, String> {
	Ok(onelauncher::package::content::get_featured_packages().await?)
}

#[onelauncher_core::command]
pub fn get_program_info() -> Result<super::statics::ProgramInfo, String> {
	Ok(super::statics::get_program_info())
}

#[onelauncher_core::command]
pub async fn get_minecraft_versions() -> Result<Vec<Version>, String> {
	Ok(onelauncher::api::metadata::get_minecraft_versions()
		.await?
		.versions)
}

#[onelauncher_core::command]
pub async fn get_settings() -> Result<Settings, String> {
	Ok(settings::get().await?)
}

#[onelauncher_core::command]
pub async fn set_settings(settings: Settings) -> Result<(), String> {
	Ok(settings::set(settings).await?)
}

#[onelauncher_core::command]
pub fn set_window_style(handle: AppHandle, custom: bool) -> Result<(), String> {
	let window = handle.get_webview_window("main").unwrap();
	onelauncher::utils::window::set_window_styling(&window, custom).map_err(|e| e.to_string())
}
