//! Manages importing data from other launchers.

use crate::prelude::ClusterPath;
use crate::proxy::send::{init_or_edit_ingress, send_ingress};
use crate::proxy::IngressId;
use crate::store::Clusters;
use crate::utils::http::{self, IoSemaphore};
use crate::utils::io::{self, IOError};
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImportType {
	/// MultiMC based launchers
	MultiMC,
	/// Prism Launcher has its own category because it has different logic (also is objectively better than mmc)
	PrismLauncher,
	/// GDLauncher
	GDLauncher,
	/// Curseforge's launcher
	Curseforge,
	/// ATLauncher
	ATLauncher,
	/// Modrinth app.
	Modrinth,
	/// TLauncher
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
			ImportType::MultiMC => write!(f, "MultiMC"),
			ImportType::ATLauncher => write!(f, "ATLauncher"),
			ImportType::Curseforge => write!(f, "Curseforge"),
			ImportType::GDLauncher => write!(f, "GDLauncher"),
			ImportType::Modrinth => write!(f, "Modrinth"),
			ImportType::PrismLauncher => write!(f, "PrismLauncher"),
			ImportType::TLauncher => write!(f, "TLauncher"),
			ImportType::FTBLauncher => write!(f, "FTB"),
			ImportType::Technic => write!(f, "Technic"),
			ImportType::Unknown => write!(f, "Unknown"),
		}
	}
}

pub async fn import_instances(import: ImportType, path: PathBuf) -> crate::Result<Vec<String>> {
    // TODO(pauline)(instance_import): Fix MultiMC based launcher import
	let instances_path = match import {
		ImportType::GDLauncher | ImportType::ATLauncher => "instances".to_string(),
		ImportType::Curseforge => "Instances".to_string(),
		// ImportType::MultiMC => multibased::get_instances_path(path.clone().join("multimc.cfg"))
		// 	.await
		// 	.unwrap_or_else(|| "instances".to_string()),
		// ImportType::PrismLauncher => {
		// 	multibased::get_instances_path(path.clone().join("prismlauncher.cfg"))
		// 		.await
		// 		.unwrap_or_else(|| "instances".to_string())
		// }
		ImportType::Modrinth => "profiles".to_string(),
		ImportType::TLauncher => "instances".to_string(),
		ImportType::Technic => "instances".to_string(),
		ImportType::FTBLauncher => "instances".to_string(),
		_ | ImportType::Unknown => {
			return Err(anyhow::anyhow!("launcher type unknown, cant import").into())
		}
	};

	let instances_dir = path.join(&instances_path);
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
		if path.is_dir() {
			if is_valid_instance(path.clone(), import).await {
				let name = path.file_name();
				if let Some(name) = name {
					instances.push(name.to_string_lossy().to_string());
				}
			}
		}
	}

	Ok(instances)
}

#[tracing::instrument]
#[onelauncher_debug::debugger]
pub async fn import_instance(
	cluster_path: ClusterPath,
	import: ImportType,
	path: PathBuf,
	instance_path: String,
) -> crate::Result<()> {
	tracing::debug!("importing instance from {instance_path}");

    // TODO(pauline)(instance_import): Finish this
	// let result = match import {
	// 	ImportType::MultiMC | ImportType::PrismLauncher => {
	// 		multibased::import(path, instance_path, cluster_path.clone()).await
	// 	}
	// 	ImportType::ATLauncher => {
	// 		atlauncher::import(path, instance_path, cluster_path.clone()).await
	// 	}
	// 	ImportType::GDLauncher => {
	// 		gdlauncher::import(
	// 			path.join("instances").join(instance_path),
	// 			cluster_path.clone(),
	// 		)
	// 		.await
	// 	}
	// 	ImportType::Curseforge => {
	// 		curseforge::import(
	// 			path.join("Instances").join(instance_path),
	// 			cluster_path.clone(),
	// 		)
	// 		.await
	// 	}
	// 	ImportType::Modrinth => {
	// 		modrinth::import(
	// 			path.join("profiles").join(instance_path),
	// 			cluster_path.clone(),
	// 		)
	// 		.await
	// 	}
	// 	ImportType::TLauncher => {
	// 		tlauncher::import(
	// 			path.join("instances").join(instance_path),
	// 			cluster_path.clone(),
	// 		)
	// 		.await
	// 	}
	// 	ImportType::Technic => {
	// 		technic::import(
	// 			path.join("instances").join(instance_path),
	// 			cluster_path.clone(),
	// 		)
	// 		.await
	// 	}
	// 	ImportType::FTBLauncher => {
	// 		ftb::import(
	// 			path.join("instances").join(instance_path),
	// 			cluster_path.clone(),
	// 		)
	// 		.await
	// 	}
	// 	ImportType::Unknown => {
	// 		return Err(anyhow::anyhow!("unknown launcher type").into());
	// 	}
	// };

	// match result {
	// 	Ok(_) => {}
	// 	Err(e) => {
	// 		tracing::warn!("failed to import modpack: {:?}", e);
	// 		let _ = crate::api::cluster::remove(&cluster_path).await;
	// 		return Err(e);
	// 	}
	// }

	// tokio::task::spawn(Clusters::update_versions());

	// tracing::debug!("completed import of instance.");
	Ok(())
}

/// returns the default path for a given [`ImportType`].
pub fn default_launcher_path(r#type: ImportType) -> Option<PathBuf> {
	let path = match r#type {
		ImportType::MultiMC => None,
		ImportType::PrismLauncher => Some(dirs::data_dir()?.join("PrismLauncher")),
		ImportType::ATLauncher => Some(dirs::data_dir()?.join("ATLauncher")),
		ImportType::GDLauncher => Some(dirs::data_dir()?.join("gdlauncher_next")),
		ImportType::Curseforge => Some(dirs::home_dir()?.join("curseforge").join("minecraft")),
		ImportType::Modrinth => Some(dirs::data_dir()?.join("theseus")),
		ImportType::FTBLauncher => Some(dirs::data_dir()?.join("FTB")),
		ImportType::Technic => Some(dirs::data_dir()?.join("Technic")),
		ImportType::TLauncher => Some(dirs::data_dir()?.join("TLauncher")),
		ImportType::Unknown => None,
	};

	let path = path?;
	if path.exists() {
		Some(path)
	} else {
		None
	}
}

/// checks if a [`PathBuf`] is a valid instance for a given [`ImportType`]
#[tracing::instrument]
#[onelauncher_debug::debugger]
pub async fn is_valid_instance(instance_path: PathBuf, r#type: ImportType) -> bool {
    false
    // TODO(pauline)(instance_import): Finish this
	// match r#type {
	// 	ImportType::MultiMC | ImportType::PrismLauncher => multibased::is_valid(insance_path).await,
	// 	ImportType::ATLauncher => atlauncher::is_valid(instance_path).await,
	// 	ImportType::GDLauncher => gdlauncher::is_valid(instance_path).await,
	// 	ImportType::Curseforge => curseforge::is_valid(instance_path).await,
	// 	ImportType::Modrinth => modrinth::is_valid(instance_path).await,
	// 	ImportType::TLauncher => tlauncher::is_valid(instance_path).await,
	// 	ImportType::FTBLauncher => ftb::is_valid(instance_path).await,
	// 	ImportType::Technic => technic::is_valid(instance_path).await,
	// 	ImportType::Unknown => false,
	// }
}

/// caches an image file
#[tracing::instrument]
#[onelauncher_debug::debugger]
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
	let cluster_full = cluster_path.full_path().await?;
	let subfiles = sub(&minecraft_path).await?;
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
		let child = cluster_path.0.join(child);
		tokio::time::sleep(std::time::Duration::from_millis(1)).await;
		http::copy(&sub, &child, io_semaphore).await?;
		send_ingress(&ingress, 1.0, None).await?;
	}

	Ok(ingress)
}

/// recursively get a [`Vec<PathBuf>`] of all subfiles.
#[onelauncher_debug::debugger]
#[async_recursion::async_recursion]
#[tracing::instrument]
pub async fn sub(path: &Path) -> crate::Result<Vec<PathBuf>> {
	if !path.is_dir() {
		return Ok(vec![path.to_path_buf()]);
	}
	let mut files = Vec::new();
	let mut dir = io::read_dir(&path).await?;
	while let Some(child) = dir
		.next_entry()
		.await
		.map_err(|e| IOError::with_path(e, path))?
	{
		let path_child = child.path();
		files.append(&mut sub(&path_child).await?);
	}

	Ok(files)
}
