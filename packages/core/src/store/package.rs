//! Handlers for Mod metadata that can be displayed in a GUI mod list or exported as a mod pack

use crate::package::content::Providers;
use crate::store::Loader;
use crate::utils::http::{write_icon, IoSemaphore};
use async_zip::tokio::read::fs::ZipFileReader;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::PackagePath;

// TODO: Curseforge, Modrinth, SkyClient integration

/// Represents types of packages handled by the launcher.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
	/// Represents a mod jarfile
	Mod,
	/// Represents a datapack file
	DataPack,
	/// Represents a resourcepack file
	ResourcePack,
	/// Represents a shaderpack file
	ShaderPack,
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
			Some(PackageType::ShaderPack)
		} else if loaders
			.iter()
			.any(|x| ["vanilla", "canvas", "minecraft"].contains(&&**x))
		{
			Some(PackageType::ResourcePack)
		} else {
			None
		}
	}

	pub fn from_parent(path: PathBuf) -> Option<Self> {
		let path = path.parent()?.file_name()?;
		match path.to_str()? {
			"mods" => Some(PackageType::Mod),
			"datapacks" => Some(PackageType::DataPack),
			"resourcepacks" => Some(PackageType::ResourcePack),
			"shaderpacks" => Some(PackageType::ShaderPack),
			_ => None,
		}
	}

	pub fn get_name(&self) -> &'static str {
		match self {
			PackageType::Mod => "mod",
			PackageType::DataPack => "datapack",
			PackageType::ResourcePack => "resourcepack",
			PackageType::ShaderPack => "shaderpack",
		}
	}

	pub fn get_folder(&self) -> &'static str {
		match self {
			PackageType::Mod => "mods",
			PackageType::DataPack => "datapacks",
			PackageType::ResourcePack => "resourcepacks",
			PackageType::ShaderPack => "shaderpacks",
		}
	}

	pub fn get_meta(&self) -> PathBuf {
		PathBuf::from(format!("{}/.packages.json", self.get_folder()))
	}

	pub fn iterator() -> impl Iterator<Item = PackageType> {
		[
			PackageType::Mod,
			PackageType::DataPack,
			PackageType::ResourcePack,
			PackageType::ShaderPack,
		]
		.iter()
		.copied()
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchResult {
	pub slug: String,
	pub title: String,
	pub description: String,
	#[serde(default)]
	pub categories: Vec<String>,
	pub client_side: PackageSide,
	pub server_side: PackageSide,
	pub project_type: PackageType,
	pub downloads: u32,
	#[serde(default)]
	pub icon_url: String,
	pub project_id: String,
	pub author: String,
	#[serde(default)]
	pub display_categories: Vec<String>,
	pub versions: Vec<String>,
	pub follows: u32,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
	#[serde(default)]
	pub license: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProviderSearchResults {
	pub provider: Providers,
	pub results: Vec<SearchResult>,
}

/// A struct that represents a Package.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Package {
	pub sha512: String,
	pub meta: PackageMetadata,
	pub file_name: String,
	pub disabled: bool,
}

impl Package {
	pub async fn new(path: &PackagePath, meta: PackageMetadata) -> crate::Result<Self> {
		let file_name = path
			.0
			.file_name()
			.ok_or_else(|| crate::ErrorKind::AnyhowError(anyhow::anyhow!("no file name")))?
			.to_string_lossy()
			.to_string();
		let sha512 = String::from("unknown");

		Ok(Self {
			sha512,
			meta,
			file_name,
			disabled: false,
		})
	}
}

pub type Packages = HashMap<PackagePath, Package>;

/// Metadata that represents a [`Package`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PackageMetadata {
	Managed {
		package_id: String,
		provider: Providers,
		package_type: PackageType,
		title: String,
		version_id: String,
		version_formatted: String,
	},
	Mapped {
		title: Option<String>,
		description: Option<String>,
		authors: Vec<String>,
		version: Option<String>,
		icon: Option<PathBuf>,
		package_type: Option<PackageType>,
	},
	Unknown,
}

impl PackageMetadata {
	pub fn from_managed_package(package: ManagedPackage, version: ManagedVersion) -> Self {
		Self::Managed {
			package_id: package.id,
			version_id: version.id,
			version_formatted: version.version_id,
			title: package.title,
			provider: package.provider,
			package_type: package.package_type,
		}
	}
}

/// Universal metadata for any managed user from a Mod distribution platform.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManagedUser {
	pub id: String,
	pub username: String,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub avatar_url: Option<String>,
	#[serde(default)]
	pub bio: Option<String>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Author {
	Team {
		team: String,
		organization: Option<String>,
	},
	Users(Vec<ManagedUser>),
}

/// Universal metadata for any managed package from a Mod distribution platform.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManagedPackage {
	// Core Metadata
	pub provider: Providers,
	pub id: String,
	pub uid: Option<String>,
	pub package_type: PackageType,
	pub title: String,
	pub description: String,
	pub body: String,
	pub main: String,
	pub versions: Vec<String>,
	pub game_versions: Vec<String>,
	pub loaders: Vec<Loader>,
	pub icon_url: Option<String>,

	pub created: DateTime<Utc>,
	pub updated: DateTime<Utc>,
	pub client: PackageSide,
	pub server: PackageSide,
	pub downloads: u32,
	pub followers: u32,
	pub categories: Vec<String>,
	pub optional_categories: Option<Vec<String>>,
	pub license: Option<License>,

	pub author: Author,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct License {
	#[serde(default)]
	pub id: String,
	#[serde(default)]
	pub name: String,
	#[serde(default)]
	pub url: Option<String>,
}

/// Universal managed package version of a package.
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
	pub loaders: Vec<Loader>,
}

impl ManagedVersion {
	pub fn get_primary_file(&self) -> Option<&ManagedVersionFile> {
		self.files.iter().find(|f| f.primary)
	}
}

/// Universal interface for managed package files.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct ManagedVersionFile {
	pub url: String,
	pub file_name: String,
	pub primary: bool,

	pub size: u32,
	pub file_type: Option<PackageFile>,
	pub hashes: HashMap<String, String>,
}

/// Universal interface for managed package dependencies.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManagedDependency {
	pub version_id: Option<String>,
	pub package_id: Option<String>,
	pub file_name: Option<String>,
	pub dependency_type: PackageDependency,
}

/// The type of a [`ManagedDependency`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum PackageSide {
	Required,
	Optional,
	Unsupported,
	Unknown,
}

/// The file type of a [`Package`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum PackageFile {
	RequiredPack,
	OptionalPack,
	Unknown,
}

#[tracing::instrument(skip(io_semaphore))]
#[onelauncher_macros::memory]
pub async fn read_icon(
	icon_path: Option<String>,
	cache_dir: &Path,
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
					let path = write_icon(&icon_path, cache_dir, bytes, io_semaphore).await?;

					return Ok(Some(path));
				}
			}
		}
	}

	Ok(None)
}
