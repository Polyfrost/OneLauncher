//! Launcher Import: ATLauncher
//! Source Code available at https://github.com/ATLauncher/ATLauncher

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::package::from::CreatePackDescription;
use crate::package::import::{self, copy_minecraft};
use crate::package::{self};
use crate::prelude::{Cluster, ClusterPath, Loader};
use crate::store::{ClusterStage, PackageData, State};
use crate::utils::io;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ATInstance {
	pub id: String, // minecraft version id ie: 1.12.1, not a name
	pub launcher: ATLauncher,
	pub java_version: ATJavaVersion,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ATLauncher {
	pub name: String,
	pub pack: String,
	pub version: String,
	pub loader_version: ATLauncherLoaderVersion,

	pub managed_project: Option<ATLauncherManagedProject>,
	pub managed_version: Option<ATLauncherManagedVersion>,
	pub managed_manifest: Option<package::from::PackFormat>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ATJavaVersion {
	pub major_version: u8,
	pub component: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ATLauncherLoaderVersion {
	pub r#type: String,
	pub version: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ATLauncherManagedProject {
	pub id: String,
	pub slug: String,
	pub project_type: String,
	pub team: String,
	pub client_side: Option<String>,
	pub server_side: Option<String>,
	pub categories: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ATLauncherManagedVersion {
	pub id: String,
	pub project_id: String,
	pub name: String,
	pub version_number: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ATLauncherManagedVersionFile {
	pub hashes: HashMap<String, String>,
	pub url: String,
	pub filename: String,
	pub primary: bool,
	pub size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ATLauncherManagedVersionDependency {
	pub project_id: Option<String>,
	pub version_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ATLauncherMod {
	pub name: String,
	pub version: String,
	pub file: String,

	pub managed_project: Option<ATLauncherManagedProject>,
	pub managed_version: Option<ATLauncherManagedVersion>,
}

pub async fn is_valid_atlauncher(instance_folder: PathBuf) -> bool {
	let instance: String = io::read_to_string(&instance_folder.join("instance.json"))
		.await
		.unwrap_or("".to_string());
	let instance: Result<ATInstance, serde_json::Error> =
		serde_json::from_str::<ATInstance>(&instance);
	if let Err(e) = instance {
		tracing::warn!(
			"failed to parse instance.json at {}: {}",
			instance_folder.display(),
			e
		);
		false
	} else {
		true
	}
}

#[tracing::instrument]
#[onelauncher_macros::memory]
pub async fn import_atlauncher(
	// The path to the base ATLauncher folder.
	atlauncher_base_path: PathBuf,
	// The instance folder in atlauncher_base_path
	instance_folder: String,
	// The path to the importing Cluster
	cluster_path: ClusterPath,
) -> crate::Result<()> {
	let atlauncher_instance_path = atlauncher_base_path
		.join("instances")
		.join(instance_folder.clone());
	let atinstance: String =
		io::read_to_string(&atlauncher_instance_path.join("instance.json")).await?;
	let atinstance: ATInstance = serde_json::from_str::<ATInstance>(&atinstance)?;

	// Icon path should be {instance_folder}/instance.png if it exists,
	// Another possibility is ATLauncher/configs/images/{safe_pack_name}.png (safe pack name is alphanumeric lowercase)
	let icon_path_primary = atlauncher_instance_path.join("instance.png");
	let safe_pack_name = atinstance
		.launcher
		.pack
		.replace(|c: char| !c.is_alphanumeric(), "")
		.to_lowercase();
	let icon_path_secondary = atlauncher_base_path
		.join("configs")
		.join("images")
		.join(safe_pack_name + ".png");
	let icon = match (icon_path_primary.exists(), icon_path_secondary.exists()) {
		(true, _) => import::cache_icon(icon_path_primary).await?,
		(_, true) => import::cache_icon(icon_path_secondary).await?,
		_ => None,
	};

	// Create description from instance.cfg
	let description = CreatePackDescription {
		icon,
		override_title: Some(atinstance.launcher.name.clone()),
		package_id: None,
		version_id: None,
		existing_ingress: None,
		cluster_path: cluster_path.clone(),
	};

	let backup_name = format!("ATLauncher-{}", instance_folder);
	let minecraft_folder = atlauncher_instance_path;

	import_atlauncher_unmanaged(
		cluster_path,
		minecraft_folder,
		backup_name,
		description,
		atinstance,
	)
	.await?;

	Ok(())
}

async fn import_atlauncher_unmanaged(
	cluster_path: ClusterPath,
	minecraft_folder: PathBuf,
	backup_name: String,
	description: CreatePackDescription,
	atinstance: ATInstance,
) -> crate::Result<()> {
	let mod_loader = format!(
		"\"{}\"",
		atinstance.launcher.loader_version.r#type.to_lowercase()
	);
	let mod_loader: Loader = serde_json::from_str::<Loader>(&mod_loader)?;

	let game_version = atinstance.id;
	let loader_version = if mod_loader != Loader::Vanilla {
		crate::cluster::create::get_loader_version(
			game_version.clone(),
			mod_loader,
			Some(atinstance.launcher.loader_version.version.clone()),
		)
		.await?
	} else {
		None
	};

	crate::api::cluster::edit(&cluster_path, |cl| {
		cl.meta.name = description
			.override_title
			.clone()
			.unwrap_or_else(|| backup_name.to_string());
		cl.stage = ClusterStage::PackDownloading;
		cl.meta.package_data = Some(PackageData {
			package_id: description.package_id.clone(),
			version_id: description.version_id.clone(),
			locked: Some(description.package_id.is_some() && description.version_id.is_some()),
		});
		cl.meta.icon.clone_from(&description.icon);
		cl.meta.mc_version.clone_from(&game_version);
		cl.meta.loader_version.clone_from(&loader_version);
		cl.meta.loader = mod_loader;

		async { Ok(()) }
	})
	.await?;

	// moves .minecraft folder over (ie: overrides such as resourcepacks, mods, etc)
	let state = State::get().await?;
	let ingress = copy_minecraft(
		cluster_path.clone(),
		minecraft_folder,
		&state.io_semaphore,
		None,
	)
	.await?;

	if let Some(cluster_val) = crate::api::cluster::get(&cluster_path, None).await? {
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
