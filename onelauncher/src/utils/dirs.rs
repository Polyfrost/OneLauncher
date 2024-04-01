use std::path::PathBuf;

use crate::constants;

#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
	#[error("failed to get the user's config directory")]
	ConfigDir,
}

pub fn app_config_dir() -> crate::Result<PathBuf> {
	dirs::config_dir()
		.and_then(|dir| Some(dir.join(constants::APP_CONFIG_DIR)))
		.ok_or(DirectoryError::ConfigDir.into())
}

pub fn assets_dir() -> crate::Result<PathBuf> {
    Ok(app_config_dir()?.join("assets"))
}

pub fn libraries_dir() -> crate::Result<PathBuf> {
	Ok(app_config_dir()?.join("libraries"))
}

pub fn natives_dir() -> crate::Result<PathBuf> {
	Ok(app_config_dir()?.join("natives"))
}

pub fn java_dir() -> crate::Result<PathBuf> {
	Ok(app_config_dir()?.join("java"))
}

pub fn clusters_dir() -> crate::Result<PathBuf> {
	Ok(app_config_dir()?.join("clusters"))
}

pub fn cluster_dir(uuid: String) -> crate::Result<PathBuf> {
    Ok(clusters_dir()?.join(uuid))
}

pub fn clients_dir() -> crate::Result<PathBuf> {
	Ok(app_config_dir()?.join("clients"))
}
