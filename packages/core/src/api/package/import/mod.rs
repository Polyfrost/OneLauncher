//! Manages importing data from other launchers.

use crate::prelude::ClusterPath;
use crate::proxy::send::{init_or_edit_ingress, send_ingress};
use crate::proxy::IngressId;

use crate::store::Clusters;
use crate::utils::http::{self, IoSemaphore};
use onelauncher_utils::io::{self, IOError};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

pub mod atlauncher;
pub mod curseforge;
pub mod ftb;
pub mod gdlauncher;
pub mod modrinth;
pub mod multibased;
pub mod technic;
pub mod tlauncher;

/// List of launcher types we support importing from.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImportType {
	/// `MultiMC` based launchers
	MultiMC,
	/// Prism Launcher has its own category because it has different logic (also is objectively better than mmc)
	PrismLauncher,
	/// `GDLauncher`
	GDLauncher,
	/// Curseforge's launcher
	Curseforge,
	/// `ATLauncher`
	ATLauncher,
	/// Modrinth app.
	Modrinth,
	/// `TLauncher`
	TLauncher,
	/// FTB Launcher
	FTBLauncher,
	/// Technic
	Technic,
	/// Unknown import option (not widely adopted -> probably a custom launcher with a similar file structure to the above)
	#[serde(other)]
	Unknown,
}

impl fmt::Display for ImportType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::MultiMC => write!(f, "MultiMC"),
			Self::ATLauncher => write!(f, "ATLauncher"),
			Self::Curseforge => write!(f, "Curseforge"),
			Self::GDLauncher => write!(f, "GDLauncher"),
			Self::Modrinth => write!(f, "Modrinth App"),
			Self::PrismLauncher => write!(f, "PrismLauncher"),
			Self::TLauncher => write!(f, "TLauncher"),
			Self::FTBLauncher => write!(f, "Feed The Beast"),
			Self::Technic => write!(f, "Technic"),
			Self::Unknown => write!(f, "Unknown"),
		}
	}
}

impl ImportType {
	pub async fn get_instances_subpath(&self, path: &Path) -> crate::Result<PathBuf> {
		Ok(path.join(
			&(match self {
				Self::GDLauncher | Self::ATLauncher => "instances".to_string(),
				Self::Curseforge => "Instances".to_string(),
				Self::MultiMC => {
					multibased::get_instances_subpath(path.to_path_buf().join("multimc.cfg"))
						.await
						.unwrap_or_else(|| "instances".to_string())
				}
				Self::PrismLauncher => {
					multibased::get_instances_subpath(path.to_path_buf().join("prismlauncher.cfg"))
						.await
						.unwrap_or_else(|| "instances".to_string())
				}
				Self::Modrinth => "profiles".to_string(),
				Self::TLauncher => "tinstances".to_string(),
				Self::FTBLauncher => "ftbinstances".to_string(),
				Self::Technic => "tecinstances".to_string(),
				Self::Unknown => {
					return Err(anyhow::anyhow!("launcher type unknown, cant import").into())
				}
			}),
		))
	}
}

pub async fn get_launcher_instances(import: ImportType, path: Option<PathBuf>) -> crate::Result<(PathBuf, Vec<String>)> {
	let base_path = match &path {
		Some(path) => path,
		None => &default_launcher_path(import).ok_or_else(|| anyhow::anyhow!("could not get launcher base path for {import}"))?
	};

	let instances_dir = import.get_instances_subpath(base_path).await?;

	let mut instances = Vec::new();
	let mut dir = io::read_dir(&instances_dir)
		.await
		.map_err(|_| anyhow::anyhow!("invalid {import} launcher path, failed to import."))?;

	while let Some(e) = dir
		.next_entry()
		.await
		.map_err(|e| IOError::with_path(e, &instances_dir))?
	{
		let path = e.path();
		if path.is_dir() && is_valid_instance(path.clone(), import).await {
			let name = path.file_name();
			if let Some(name) = name {
				instances.push(name.to_string_lossy().to_string());
			}
		}
	}

	Ok((base_path.to_owned(), instances))
}

#[tracing::instrument]
pub async fn import_instances(
	import: ImportType,
	base_path: PathBuf,
	instances: Vec<String>,
) -> crate::Result<()> {
	for instance in instances {
		import_instance(import, base_path.clone(), instance).await?;
	}

	Ok(())
}

#[tracing::instrument]
#[onelauncher_macros::memory]
pub async fn import_instance(
	import: ImportType,
	base_path: PathBuf,
	instance_path: String,
) -> crate::Result<()> {
	tracing::debug!("importing instance from {instance_path}");

	let cluster_path = crate::cluster::create::create_unfinished_cluster(instance_path.clone()).await?;

	let result = match import {
		ImportType::MultiMC | ImportType::PrismLauncher => {
			multibased::import_mmc(base_path, instance_path, cluster_path.clone()).await
		}
		ImportType::ATLauncher => {
			atlauncher::import_atlauncher(base_path, instance_path, cluster_path.clone()).await
		}
		ImportType::GDLauncher => {
			gdlauncher::import_gdlauncher(
				base_path.join("instances").join(instance_path),
				cluster_path.clone(),
			)
			.await
		}
		ImportType::Curseforge => {
			curseforge::import_curseforge(
				base_path.join("Instances").join(instance_path),
				cluster_path.clone(),
			)
			.await
		}
		ImportType::Modrinth => {
			modrinth::import_modrinth(
				base_path.join("profiles").join(instance_path),
				cluster_path.clone(),
			)
			.await
		}
		// ImportType::TLauncher => tlauncher::import_tlauncher(path.join("instances").join(instance_path), cluster_path.clone()).await,
		// ImportType::Technic => technic::import_technic(path.join("instances").join(instance_path), cluster_path.clone()).await,
		// ImportType::FTBLauncher => ftb::import_ftb(path.join("instances").join(instance_path), cluster_path.clone()).await,
		ImportType::Unknown => Err(anyhow::anyhow!("unknown launcher type").into()),
		_ => todo!(),
	};

	match result {
		Ok(()) => {}
		Err(e) => {
			tracing::warn!("failed to import modpack: {:?}", e);
			let _ = crate::api::cluster::remove(&cluster_path).await;
			return Err(e);
		}
	}

	tokio::task::spawn(Clusters::update_versions());

	tracing::debug!("completed import of instance.");
	Ok(())
}

/// returns the default path for a given [`ImportType`].
#[must_use]
pub fn default_launcher_path(r#type: ImportType) -> Option<PathBuf> {
	let path = match r#type {
		ImportType::PrismLauncher => Some(dirs::data_dir()?.join("PrismLauncher")),
		ImportType::ATLauncher => Some(dirs::data_dir()?.join("ATLauncher")),
		ImportType::GDLauncher => Some(dirs::data_dir()?.join("gdlauncher_next")),
		ImportType::Curseforge => Some(dirs::home_dir()?.join("curseforge").join("minecraft")),
		ImportType::Modrinth => Some(dirs::data_dir()?.join("theseus")),
		ImportType::FTBLauncher => Some(dirs::data_dir()?.join("FTB")),
		ImportType::Technic => Some(dirs::data_dir()?.join("Technic")),
		ImportType::TLauncher => Some(dirs::data_dir()?.join("TLauncher")),
		ImportType::Unknown | ImportType::MultiMC => None, // MultiMC data is in it's application directory
	};

	let path = path?;
	if path.exists() {
		Some(path)
	} else {
		None
	}
}

/// Checks if a [`PathBuf`] is a valid instance for a given [`ImportType`]
#[tracing::instrument]
#[onelauncher_macros::memory]
pub async fn is_valid_instance(instance_path: PathBuf, r#type: ImportType) -> bool {
	match r#type {
		ImportType::MultiMC | ImportType::PrismLauncher => {
			multibased::is_valid_multibased(instance_path).await
		}
		ImportType::ATLauncher => atlauncher::is_valid_atlauncher(instance_path).await,
		ImportType::GDLauncher => gdlauncher::is_valid_gdlauncher(instance_path).await,
		ImportType::Curseforge => curseforge::is_valid_curseforge(instance_path).await,
		// ImportType::Modrinth => modrinth::is_valid_modrinth(instance_path).await,
		// ImportType::TLauncher => tlauncher::is_valid_tlauncher(instance_path).await,
		// ImportType::FTBLauncher => ftb::is_valid_ftb(instance_path).await,
		// ImportType::Technic => technic::is_valid_technic(instance_path).await,
		ImportType::Unknown => false,
		_ => todo!(),
	}
}

#[tracing::instrument]
#[onelauncher_macros::memory]
pub async fn cache_icon(icon_path: PathBuf) -> crate::Result<Option<PathBuf>> {
	let state = crate::State::get().await?;
	let bytes = tokio::fs::read(&icon_path).await;
	if let Ok(bytes) = bytes {
		let bytes = bytes::Bytes::from(bytes);
		let cache_dir = &state.directories.caches_dir().await;
		let semaphore = &state.io_semaphore;
		Ok(Some(
			http::write_icon(&icon_path.to_string_lossy(), cache_dir, bytes, semaphore).await?,
		))
	} else {
		Ok(None)
	}
}

pub async fn copy_minecraft(
	cluster_path: ClusterPath,
	minecraft_path: PathBuf,
	io_semaphore: &IoSemaphore,
	old_ingress: Option<IngressId>,
) -> crate::Result<IngressId> {
	let cluster_path_full = cluster_path.full_path().await?;
	let subfiles = sub(&minecraft_path, false).await?;
	let total_subfiles = subfiles.len() as u64;
	let ingress = init_or_edit_ingress(
		old_ingress,
		crate::IngressType::CopyCluster {
			import: minecraft_path.clone(),
			cluster_name: cluster_path.to_string(),
		},
		total_subfiles as f64,
		"copying files in preexisting cluster",
	)
	.await?;

	for sub in subfiles {
		let child = sub
			.strip_prefix(&minecraft_path)
			.map_err(|_| anyhow::anyhow!("invalid .minecraft file {}", &sub.display()))?;
		let child = cluster_path_full.join(child);
		tokio::time::sleep(std::time::Duration::from_millis(1)).await;
		http::copy(&sub, &child, io_semaphore).await?;
		send_ingress(&ingress, 1.0, None).await?;
	}

	Ok(ingress)
}

/// recursively get a [`Vec<PathBuf>`] of all subfiles.
#[onelauncher_macros::memory]
#[async_recursion::async_recursion]
#[tracing::instrument]
pub async fn sub(path: &Path, include_empty: bool) -> crate::Result<Vec<PathBuf>> {
	if !path.is_dir() {
		return Ok(vec![path.to_path_buf()]);
	}
	let mut files = Vec::new();
	let mut dir = io::read_dir(&path).await?;
	let mut has_files = false;
	while let Some(child) = dir
		.next_entry()
		.await
		.map_err(|e| IOError::with_path(e, path))?
	{
		has_files = true;
		let path_child = child.path();
		files.append(&mut sub(&path_child, include_empty).await?);
	}

	if !has_files && include_empty {
		files.push(path.to_path_buf());
	}

	Ok(files)
}
