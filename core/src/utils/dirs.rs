use std::path::PathBuf;

use thiserror::Error;

use crate::{constants, PolyResult};

#[derive(Debug, Error)]
pub enum DirectoryError {
    #[error("failed to get the user's home directory")]
    HomeDir,
    #[error("failed to get the user's config directory")]
    ConfigDir,
}

pub fn app_config_dir() -> PolyResult<PathBuf> {
    dirs::config_dir()
        .and_then(|dir| Some(dir.join(constants::APP_CONFIG_DIR)))
        .ok_or(DirectoryError::ConfigDir.into())
}

pub fn libraries_dir() -> PolyResult<PathBuf> {
    Ok(app_config_dir()?.join("libraries"))
}

pub fn java_dir() -> PolyResult<PathBuf> {
    Ok(app_config_dir()?.join("java"))
}

pub fn instances_dir() -> PolyResult<PathBuf> {
    Ok(app_config_dir()?.join("instances"))
}

pub fn manifests_dir() -> PolyResult<PathBuf> {
    Ok(app_config_dir()?.join("manifests"))
}