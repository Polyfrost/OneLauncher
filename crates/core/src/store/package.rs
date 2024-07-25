//! Handlers for Mod metadata that can be displayed in a GUI mod list or exported as a mod pack

#[allow(clippy::all)]
use async_zip::tokio::read::fs::ZipFileReader;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

use crate::prelude::IOError;
use crate::utils::http::{self, write_icon, FetchSemaphore, IoSemaphore};
use crate::State;

use super::{Cluster, PackagePath};

// TODO: move all of this to prisma, it would be perfect for it
// TODO: Curseforge, Modrinth, SkyClient integration

/// Creates [`Package`] data for a given [`Cluster`] from on-device files and APIs.
/// Paths must be the full paths and not relative paths.
#[tracing::instrument(skip(paths, _cluster, _io_semaphore, _fetch_semaphore))]
pub async fn generate_context(
	_cluster: Cluster,
	paths: Vec<PathBuf>,
	_cache_path: PathBuf,
	_io_semaphore: &IoSemaphore,
	_fetch_semaphore: &FetchSemaphore,
	// credentials: &Credentials,
) -> crate::Result<HashMap<PackagePath, Package>> {
	let mut handles = vec![];

	for path in paths {
		if !path.exists() {
			continue;
		}
		if let Some(ext) = path.extension() {
			if ext == "txt" {
				continue;
			}
		}

		let handle = tokio::spawn(async move {
			let mut file = tokio::fs::File::open(path.clone())
				.await
				.map_err(|e| IOError::with_path(e, &path))?;

			let mut buffer = [0u8; 65536];
			let mut hasher = sha2::Sha512::new();

			loop {
				let read = file.read(&mut buffer).await.map_err(IOError::from)?;
				if read == 0 {
					break;
				}
				hasher.update(&buffer[..read]);
			}

			let hash = format!("{:x}", hasher.finalize());
			Ok::<_, crate::Error>((hash, path))
		});

		handles.push(handle);
	}

	let mut file_path_hashes = HashMap::new();

	for handle in handles {
		let (hash, path) = handle.await??;
		file_path_hashes.insert(hash, path);
	}

	let result = HashMap::new();
	// TODO(pauline): Finish this
	// let mut stream = tokio_stream::iter(result_packages);
	// while let Some((k, v)) = stream.next().await {
	// 	let k = PackagePath::from_path(k).await?;
	// 	result.insert(k, v);
	// }

	Ok(result)
}

/// Represents types of packages handled by the launcher.
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
	/// represents a mod jarfile
	Mod,
	/// represents a datapack file
	DataPack,
	/// represents a resourcepack file
	Resource,
	/// represents a shaderpack file
	Shader,
}

impl PackageType {
	/// attempt to get a [`PackageType`] from a loader string.
	pub fn from_loader(loaders: Vec<String>) -> Option<Self> {
		if loaders
			.iter()
			.any(|x| ["fabric", "forge", "quilt"].contains(&&**x))
		{
			Some(PackageType::Mod)
		} else if loaders.iter().any(|x| x == "datapack") {
			Some(PackageType::DataPack)
		} else if loaders.iter().any(|x| ["iris", "optifine"].contains(&&**x)) {
			Some(PackageType::Shader)
		} else if loaders
			.iter()
			.any(|x| ["vanilla", "canvas", "minecraft"].contains(&&**x))
		{
			Some(PackageType::Resource)
		} else {
			None
		}
	}

	pub fn from_parent(path: PathBuf) -> Option<Self> {
		let path = path.parent()?.file_name()?;
		match path.to_str()? {
			"mods" => Some(PackageType::Mod),
			"datapacks" => Some(PackageType::DataPack),
			"resourcepacks" => Some(PackageType::Resource),
			"shaderpacks" => Some(PackageType::Shader),
			_ => None,
		}
	}

	pub fn get_name(&self) -> &'static str {
		match self {
			PackageType::Mod => "mod",
			PackageType::DataPack => "datapack",
			PackageType::Resource => "resourcepack",
			PackageType::Shader => "shaderpack",
		}
	}

	pub fn get_folder(&self) -> &'static str {
		match self {
			PackageType::Mod => "mods",
			PackageType::DataPack => "datapacks",
			PackageType::Resource => "resourcepacks",
			PackageType::Shader => "shaderpacks",
		}
	}
}

/// A struct that represents a Package.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
pub struct Package {
	pub sha512: String,
	pub meta: PackageMetadata,
	pub file_name: String,
	pub disabled: bool,
}

/// Metadata that represents a [`Package`].
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PackageMetadata {
	Managed {
		package: Box<ManagedPackage>,
		version: Box<ManagedVersion>,
		users: Box<ManagedUser>,
		update: Option<Box<ManagedVersion>>,
		incompatible: bool,
	},
	Mapped {
		title: Option<String>,
		description: Option<String>,
		authors: Vec<String>,
		version: Option<String>,
		icon: Option<PathBuf>,
		package_type: Option<String>,
	},
	Unknown,
}

/// Universal metadata for any managed package from a Mod distribution platform.
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManagedPackage {
	// Core Metadata
	pub id: String,
	pub uid: Option<String>,
	pub package_type: PackageType,
	pub title: String,
	pub description: String,
	pub main: String,
	pub versions: Vec<String>,
	pub game_versions: Vec<String>,
	pub loaders: Vec<String>,
	pub icon_url: Option<String>,

	pub created: DateTime<Utc>,
	pub updated: DateTime<Utc>,
	pub client: PackageSide,
	pub server: PackageSide,
	pub downloads: u32,
	pub followers: u32,
	pub categories: Vec<String>,
	pub optional_categories: Option<Vec<String>>,
}

/// Universal managed package version of a package.
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManagedVersion {
	pub id: String,
	pub package_id: String,
	pub author: String,
	pub name: String,

	pub featured: bool,
	pub version_id: String,
	pub changelog: String,
	pub changelog_url: Option<String>,

	pub published: DateTime<Utc>,
	pub downloads: u32,
	pub version_type: String,

	pub files: Vec<ManagedVersionFile>,
	pub deps: Vec<ManagedDependency>,
	pub game_versions: Vec<String>,
	pub loaders: Vec<String>,
}

/// Universal interface for managed package files.
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct ManagedVersionFile {
	pub url: String,
	pub file_name: String,
	pub primary: bool,

	pub size: u32,
	pub file_type: Option<PackageFile>,
	pub hashes: HashMap<String, String>,
}

impl ManagedVersionFile {
	#[tracing::instrument]
	pub async fn download_to_cluster(&self, cluster: &Cluster) -> crate::Result<()> {
		tracing::info!(
			"downloading mod '{}' to cluster '{}'",
			self.file_name,
			cluster.meta.name
		);
		let path = cluster
			.get_full_path()
			.await?
			.join("mods")
			.join(&self.file_name);
		let state = State::get().await?;

		// TODO: Implement hashes
		let bytes = http::fetch(&self.url, None, &state.fetch_semaphore).await?;
		http::write(&path, &bytes, &state.io_semaphore).await?;

		Ok(())
	}
}

/// Universal interface for managed package dependencies.
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManagedDependency {
	pub version_id: Option<String>,
	pub package_id: Option<String>,
	pub file_name: Option<String>,
	pub dependency_type: PackageDependency,
}

/// Universal interface for managed package authors and users.
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManagedUser {
	pub id: String,
	pub role: String,
	pub username: String,
	pub name: Option<String>,
	pub avatar: Option<String>,
	pub description: Option<String>,
	pub created: DateTime<Utc>,
}

/// The type of a [`ManagedDependency`].
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PackageDependency {
	Required,
	Optional,
	Incompatible,
	Embedded,
}

/// The Client/Server side type of a [`Package`].
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
pub enum PackageSide {
	Required,
	Optional,
	Unsupported,
	Unknown,
}

/// The file type of a [`Package`].
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum PackageFile {
	RequiredPack,
	OptionalPack,
	Unknown,
}

#[tracing::instrument(skip(io_semaphore))]
async fn read_icon(
	icon_path: Option<String>,
	cache_path: &Path,
	path: &PathBuf,
	io_semaphore: &IoSemaphore,
) -> crate::Result<Option<PathBuf>> {
	if let Some(icon_path) = icon_path {
		let zip_reader = ZipFileReader::new(path).await;
		if let Ok(zip_reader) = zip_reader {
			let zip_idx_res = zip_reader
				.file()
				.entries()
				.iter()
				.position(|f| f.filename().as_str().unwrap_or_default() == icon_path);

			if let Some(zip_idx) = zip_idx_res {
				let mut bytes = Vec::new();
				if zip_reader
					.reader_with_entry(zip_idx)
					.await?
					.read_to_end_checked(&mut bytes)
					.await
					.is_ok()
				{
					let bytes = bytes::Bytes::from(bytes);
					let path = write_icon(&icon_path, cache_path, bytes, io_semaphore).await?;

					return Ok(Some(path));
				}
			}
		}
	}

	Ok(None)
}
