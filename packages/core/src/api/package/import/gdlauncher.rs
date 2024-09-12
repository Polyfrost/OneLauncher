//! Launcher Import: `GDLauncher` (legacy)
//! Source Code available at <https://github.com/gorilla-devs/GDLauncher>

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{cache_icon, copy_minecraft};
use crate::prelude::{Cluster, ClusterPath, Loader};
use crate::store::{ClusterStage, State};
use onelauncher_utils::io;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GDLauncherConfig {
	pub background: Option<String>,
	pub loader: GDLauncherLoader,
	// pub mods: Vec<GDLauncherMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GDLauncherLoader {
	pub loader_type: Loader,
	pub loader_version: Option<String>,
	pub mc_version: String,
	pub source: Option<String>,
	pub source_name: Option<String>,
}

pub async fn is_valid_gdlauncher(instance_folder: PathBuf) -> bool {
	let config: String = io::read_to_string(&instance_folder.join("config.json"))
		.await
		.unwrap_or(String::new());
	let config: Result<GDLauncherConfig, serde_json::Error> =
		serde_json::from_str::<GDLauncherConfig>(&config);
	config.is_ok()
}

pub async fn import_gdlauncher(
	gdlauncher_instance_folder: PathBuf,
	cluster_path: ClusterPath,
) -> crate::Result<()> {
	let config: String =
		io::read_to_string(&gdlauncher_instance_folder.join("config.json")).await?;
	let config: GDLauncherConfig = serde_json::from_str::<GDLauncherConfig>(&config)?;
	let override_title: Option<String> = config.loader.source_name.clone();
	let backup_name = format!(
		"GDLauncher-{}",
		gdlauncher_instance_folder
			.file_name()
			.map_or("Unknown".to_string(), |a| a.to_string_lossy().to_string())
	);

	let icon = config
		.background
		.clone()
		.map(|b| gdlauncher_instance_folder.join(b));
	let icon = if let Some(icon) = icon {
		cache_icon(icon).await?
	} else {
		None
	};

	let game_version = config.loader.mc_version;
	let mod_loader = config.loader.loader_type;
	let loader_version = config.loader.loader_version;

	let loader_version = if mod_loader == Loader::Vanilla {
		None
	} else {
		crate::cluster::create::get_loader_version(game_version.clone(), mod_loader, loader_version)
			.await?
	};

	crate::api::cluster::edit(&cluster_path, |cl| {
		cl.meta.name = override_title
			.clone()
			.unwrap_or_else(|| backup_name.to_string());
		cl.stage = ClusterStage::PackDownloading;
		cl.meta.icon.clone_from(&icon);
		cl.meta.mc_version.clone_from(&game_version);
		cl.meta.loader_version.clone_from(&loader_version);
		cl.meta.loader = mod_loader;

		async { Ok(()) }
	})
	.await?;

	let state = State::get().await?;
	let ingress = copy_minecraft(
		cluster_path.clone(),
		gdlauncher_instance_folder,
		&state.io_semaphore,
		None,
	)
	.await?;

	if let Some(cluster_val) = crate::api::cluster::get(&cluster_path).await? {
		crate::game::install_minecraft(&cluster_val, Some(ingress), false).await?;
		{
			let state = State::get().await?;
			let mut watcher = state.watcher.write().await;
			Cluster::watch(&cluster_val.get_full_path().await?, &mut watcher).await?;
		}
		State::sync().await?;
	}

	Ok(())
}
