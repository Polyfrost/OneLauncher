//! Launcher Import: Modrinth App
//! Source Code available at <https://github.com/modrinth/code>

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::prelude::ClusterPath;
use onelauncher_utils::io;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModrinthProfile {}

pub async fn is_valid_modrinth(instance_folder: PathBuf) -> bool {
	let config: String = io::read_to_string(&instance_folder.join("profile.json"))
		.await
		.unwrap_or(String::new());
	let config: Result<ModrinthProfile, serde_json::Error> =
		serde_json::from_str::<ModrinthProfile>(&config);
	config.is_ok()
}

pub async fn import_modrinth(
	_modrinth_instance_folder: PathBuf,
	_cluster_path: ClusterPath,
) -> crate::Result<()> {
	Ok(())
}
