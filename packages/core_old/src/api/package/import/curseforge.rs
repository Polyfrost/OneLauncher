//! Launcher Import: `CurseForge`
//! Closed Source, all this information is datamined unfortunately.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{cache_icon, copy_minecraft};
use crate::prelude::{Cluster, ClusterPath, Loader};
use crate::store::{ClusterStage, State};
use crate::utils::http::{fetch, write_icon};
use onelauncher_utils::io;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftInstance {
	pub name: Option<String>,
	pub base_mod_loader: Option<MinecraftInstanceModLoader>,
	pub profile_image_path: Option<PathBuf>,
	pub installed_modpack: Option<InstalledModpack>,
	/// Minecraft game version. Non-prioritized, use this if Vanilla
	pub game_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftInstanceModLoader {
	pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModpack {
	pub thumbnail_url: Option<String>,
}

pub async fn is_valid_curseforge(instance_folder: PathBuf) -> bool {
	let minecraftinstance: String =
		io::read_to_string(&instance_folder.join("minecraftinstance.json"))
			.await
			.unwrap_or(String::new());
	let minecraftinstance: Result<MinecraftInstance, serde_json::Error> =
		serde_json::from_str::<MinecraftInstance>(&minecraftinstance);
	minecraftinstance.is_ok()
}

#[allow(clippy::too_many_lines)]
pub async fn import_curseforge(
	// The path to the CurseForge instance
	curseforge_instance_folder: PathBuf,
	// The path to the cluster
	cluster_path: ClusterPath,
) -> crate::Result<()> {
	let minecraft_instance: String =
		io::read_to_string(&curseforge_instance_folder.join("minecraftinstance.json")).await?;
	let minecraft_instance: MinecraftInstance =
		serde_json::from_str::<MinecraftInstance>(&minecraft_instance)?;
	let override_title: Option<String> = minecraft_instance.name.clone();
	let backup_name = format!(
		"Curseforge-{}",
		curseforge_instance_folder
			.file_name()
			.map_or("Unknown".to_string(), |a| a.to_string_lossy().to_string())
	);

	let state = State::get().await?;
	let mut icon = None;

	if let Some(icon_path) = minecraft_instance.profile_image_path.clone() {
		icon = cache_icon(icon_path).await?;
	} else if let Some(InstalledModpack {
		thumbnail_url: Some(thumbnail_url),
	}) = minecraft_instance.installed_modpack.clone()
	{
		let icon_bytes = fetch(&thumbnail_url, None, &state.fetch_semaphore).await?;
		let filename = thumbnail_url.rsplit('/').last();
		if let Some(filename) = filename {
			icon = Some(
				write_icon(
					filename,
					&state.directories.caches_dir().await,
					icon_bytes,
					&state.io_semaphore,
				)
				.await?,
			);
		}
	}

	if let Some(instance_mod_loader) = minecraft_instance.base_mod_loader {
		let game_version = minecraft_instance.game_version;
		let mut mod_loader = None;
		let mut loader_version = None;

		match instance_mod_loader.name.split('-').collect::<Vec<&str>>()[..] {
			["forge", version] => {
				mod_loader = Some(Loader::Forge);
				loader_version = Some(version.to_string());
			}
			["fabric", version, _game_version] => {
				mod_loader = Some(Loader::Fabric);
				loader_version = Some(version.to_string());
			}
			_ => {}
		}

		let mod_loader = mod_loader.unwrap_or(Loader::Vanilla);
		let loader_version = if mod_loader == Loader::Vanilla {
			None
		} else {
			crate::cluster::create::get_loader_version(
				game_version.clone(),
				mod_loader,
				loader_version,
			)
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
	} else {
		crate::api::cluster::edit(&cluster_path, |cl| {
			cl.meta.name = override_title
				.clone()
				.unwrap_or_else(|| backup_name.to_string());
			cl.meta.icon.clone_from(&icon);
			cl.meta
				.mc_version
				.clone_from(&minecraft_instance.game_version);
			cl.meta.loader_version = None;
			cl.meta.loader = Loader::Vanilla;

			async { Ok(()) }
		})
		.await?;
	}

	let state = State::get().await?;
	let ingress = copy_minecraft(
		cluster_path.clone(),
		curseforge_instance_folder,
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
