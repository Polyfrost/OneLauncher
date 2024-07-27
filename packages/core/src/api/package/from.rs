//! Install modpacks from different sources.

use crate::data::{Loader, ManagedPackage, ManagedVersion, PackageData};
use crate::prelude::ClusterPath;
use crate::proxy::send::{init_ingress, send_ingress};
use crate::proxy::IngressId;
use crate::store::{ClusterStage, PackageSide};
use crate::utils::http::{fetch, fetch_advanced, fetch_json, write_icon};
use crate::utils::io;
use crate::{IngressType, InnerPathLinux, State};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub const MODRINTH_API_URL: &str = "https://api.modrinth.com/v2";
pub const CURSEFORGE_API_URL: &str = "https://api.cursefor";

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackFormat {
	pub game: String,
	pub format_version: i32,
	pub version_id: String,
	pub name: String,
	pub summary: Option<String>,
	pub files: Vec<PackFile>,
	pub dependencies: HashMap<PackDependency, String>,
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
	pub path: InnerPathLinux,
	pub hashes: HashMap<PackFileHash, String>,
	pub env: Option<HashMap<EnvType, PackageSide>>,
	pub downloads: Vec<String>,
	pub file_size: u32,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase", from = "String")]
pub enum PackFileHash {
	Sha1,
	Sha512,
	Unknown(String),
}

impl From<String> for PackFileHash {
	fn from(s: String) -> Self {
		return match s.as_str() {
			"sha1" => PackFileHash::Sha1,
			"sha512" => PackFileHash::Sha512,
			_ => PackFileHash::Unknown(s),
		};
	}
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum EnvType {
	Client,
	Server,
}

#[derive(Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum PackDependency {
	#[serde(rename = "forge")]
	Forge,

	#[serde(rename = "neoforge")]
	#[serde(alias = "neo-forge")]
	NeoForge,

	#[serde(rename = "fabric-loader")]
	FabricLoader,

	#[serde(rename = "quilt-loader")]
	QuiltLoader,

	#[serde(rename = "minecraft")]
	Minecraft,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum CreatePackLocation {
	/// Create a pack from a Modrinth modpack.
	FromModrinth {
		package_id: String,
		version_id: String,
		title: String,
		icon_url: Option<String>,
	},
	/// Create a pack from a file (importing, importing an mrpack or zip-style modpack)
	FromFile { path: PathBuf },
}

/// Modpack associated [`Cluster`] wrapper.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePackCluster {
	/// The name of the cluster, and relative path.
	pub name: String,
	/// The game version (MC) of the cluster.
	pub mc_version: String,
	/// The mod [`Loader`] to use (if applicable).
	pub mod_loader: Loader,
	/// The mod [`Loader`] version to use, set to `latest` or `stable` or the ID of the loader.
	/// The default value is `latest` for the latest version of the mod loader.
	pub loader_version: Option<String>,
	/// The icon of the cluster.
	pub icon: Option<PathBuf>,
	/// The URL of an icon for a cluster. (ONLY USE THIS FOR A TEMPORARY CLUSTER)
	pub icon_url: Option<String>,
	/// The linked package data (mainly used for modpacks and updating files).
	pub package_data: Option<PackageData>,
	/// Whether or not the cluster should skip being installed.
	pub skip: Option<bool>,
	/// Whether or not the cluster should skip file watching.
	pub skip_watch: Option<bool>,
}

impl Default for CreatePackCluster {
	fn default() -> Self {
		CreatePackCluster {
			name: "Untitled".to_string(),
			mc_version: "1.20.4".to_string(),
			mod_loader: Loader::Vanilla,
			loader_version: None,
			icon: None,
			icon_url: None,
			package_data: None,
			skip: Some(true),
			skip_watch: Some(false),
		}
	}
}

#[derive(Clone)]
pub struct CreatePack {
	pub file: bytes::Bytes,
	pub description: CreatePackDescription,
}

#[derive(Clone, Debug)]
pub struct CreatePackDescription {
	pub icon: Option<PathBuf>,
	pub override_title: Option<String>,
	pub package_id: Option<String>,
	pub version_id: Option<String>,
	pub existing_ingress: Option<IngressId>,
	pub cluster_path: ClusterPath,
}

pub fn get_cluster_from_pack(location: CreatePackLocation) -> CreatePackCluster {
	match location {
		CreatePackLocation::FromModrinth {
			package_id,
			version_id,
			title,
			icon_url,
		} => CreatePackCluster {
			name: title,
			icon_url,
			package_data: Some(PackageData {
				package_id: Some(package_id),
				version_id: Some(version_id),
				locked: Some(true),
			}),
			..Default::default()
		},
		CreatePackLocation::FromFile { path } => {
			let file_name = path
				.file_stem()
				.unwrap_or_default()
				.to_string_lossy()
				.to_string();
			CreatePackCluster {
				name: file_name,
				..Default::default()
			}
		}
	}
}

#[tracing::instrument]
#[onelauncher_macros::memory]
pub async fn generate_pack_from_version_id(
	package_id: String,
	version_id: String,
	title: String,
	icon_url: Option<String>,
	cluster_path: ClusterPath,
	existing_ingress: Option<IngressId>,
) -> crate::Result<CreatePack> {
	let state = State::get().await?;

	let ingress = if let Some(ing) = existing_ingress {
		send_ingress(&ing, 0.0, Some("downloading pack file")).await?;
		ing
	} else {
		init_ingress(
			IngressType::DownloadPack {
				cluster_path: cluster_path.full_path().await?,
				package_name: title,
				icon: icon_url,
				package_version: version_id.clone(),
			},
			100.0,
			"downloading pack file",
		)
		.await?
	};

	send_ingress(&ingress, 0.0, Some("fetching version")).await?;
	let version: ManagedVersion = fetch_json(
		Method::GET,
		&format!("{}version/{}", MODRINTH_API_URL, version_id),
		None,
		None,
		&state.fetch_semaphore,
	)
	.await?;
	send_ingress(&ingress, 10.0, None).await?;

	let (url, hash) = if let Some(file) = version.files.iter().find(|x| x.primary) {
		Some((file.url.clone(), file.hashes.get("sha1")))
	} else {
		version
			.files
			.first()
			.map(|file| (file.url.clone(), file.hashes.get("sha1")))
	}
	.ok_or_else(|| {
		crate::ErrorKind::AnyhowError(anyhow::anyhow!("specified version has no files"))
	})?;

	let file = fetch_advanced(
		Method::GET,
		&url,
		hash.map(|x| &**x),
		None,
		None,
		Some((&ingress, 70.0)),
		&state.fetch_semaphore,
	)
	.await?;
	send_ingress(&ingress, 0.0, Some("fetching project metadata")).await?;

	let pkg: ManagedPackage = fetch_json(
		Method::GET,
		&format!("{}project/{}", MODRINTH_API_URL, version.package_id),
		None,
		None,
		&state.fetch_semaphore,
	)
	.await?;

	send_ingress(&ingress, 10.0, Some("Retrieving icon")).await?;
	let icon = if let Some(icon_url) = pkg.icon_url {
		let state = State::get().await?;
		let icon_bytes = fetch(&icon_url, None, &state.fetch_semaphore).await?;
		let filename = icon_url.rsplit('/').next();

		if let Some(filename) = filename {
			Some(
				write_icon(
					filename,
					&state.directories.caches_dir().await,
					icon_bytes,
					&state.io_semaphore,
				)
				.await?,
			)
		} else {
			None
		}
	} else {
		None
	};
	send_ingress(&ingress, 10.0, None).await?;

	Ok(CreatePack {
		file,
		description: CreatePackDescription {
			icon,
			override_title: None,
			package_id: Some(package_id),
			version_id: Some(version_id),
			existing_ingress: Some(ingress),
			cluster_path,
		},
	})
}

#[tracing::instrument]
#[onelauncher_macros::memory]
pub async fn generate_pack_from_file(
	path: PathBuf,
	cluster_path: ClusterPath,
) -> crate::Result<CreatePack> {
	let file = io::read(&path).await?;
	Ok(CreatePack {
		file: bytes::Bytes::from(file),
		description: CreatePackDescription {
			icon: None,
			override_title: None,
			package_id: None,
			version_id: None,
			existing_ingress: None,
			cluster_path,
		},
	})
}

/// Sets generated cluster attributes to the pack attributes (using `cluster::edit`).
/// This includes the pack name, icon, game version, loader version, and loader.
#[onelauncher_macros::memory]
pub async fn set_cluster_information(
	cluster_path: ClusterPath,
	description: &CreatePackDescription,
	backup_name: &str,
	deps: &HashMap<PackDependency, String>,
	ignore_lock: bool,
) -> crate::Result<()> {
	let mut game_version: Option<&String> = None;
	let mut mod_loader = None;
	let mut loader_version = None;

	for (key, value) in deps {
		match key {
			PackDependency::Forge => {
				mod_loader = Some(Loader::Forge);
				loader_version = Some(value);
			}
			PackDependency::NeoForge => {
				mod_loader = Some(Loader::NeoForge);
				loader_version = Some(value);
			}
			PackDependency::FabricLoader => {
				mod_loader = Some(Loader::Fabric);
				loader_version = Some(value);
			}
			PackDependency::QuiltLoader => {
				mod_loader = Some(Loader::Quilt);
				loader_version = Some(value);
			}
			PackDependency::Minecraft => game_version = Some(value),
		}
	}

	let game_version = if let Some(game_version) = game_version {
		game_version
	} else {
		return Err(anyhow::anyhow!("pack did not specify Minecraft version").into());
	};

	let mod_loader = mod_loader.unwrap_or(Loader::Vanilla);
	let loader_version = if mod_loader != Loader::Vanilla {
		crate::cluster::create::get_loader_version(
			game_version.clone(),
			mod_loader,
			loader_version.cloned(),
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
		let package_id = description.package_id.clone();
		let version_id = description.version_id.clone();

		cl.meta.package_data = if package_id.is_some() && version_id.is_some() {
			Some(PackageData {
				package_id,
				version_id,
				locked: if !ignore_lock {
					Some(true)
				} else {
					cl.meta.package_data.as_ref().and_then(|x| x.locked)
				},
			})
		} else {
			None
		};

		cl.meta.icon.clone_from(&description.icon);
		cl.meta.mc_version.clone_from(game_version);
		cl.meta.loader_version.clone_from(&loader_version);
		cl.meta.loader = mod_loader;

		async { Ok(()) }
	})
	.await?;

	Ok(())
}
