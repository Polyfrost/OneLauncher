//! Handlers for Mod metadata that can be displayed in a GUI mod list or exported as a mod pack

use crate::package::content::Providers;
use crate::store::Loader;
use crate::utils::crypto;
use crate::utils::http::{write_icon, IoSemaphore};
use crate::State;
use async_zip::tokio::read::fs::ZipFileReader;
use chrono::{DateTime, Utc};
use onelauncher_utils::io;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::vec;
use tokio::fs::DirEntry;

use super::{ClusterPath, Clusters, Directories, PackagePath};

/// Represents types of packages handled by the launcher.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
	/// Represents a mod jarfile
	Mod,
	/// Represents a datapack file
	DataPack,
	/// Represents a resourcepack file
	ResourcePack,
	/// Represents a shaderpack file
	#[serde(alias = "shader")]
	ShaderPack,
	/// Represents a modpack file
	ModPack,
}

impl PackageType {
	/// attempt to get a [`PackageType`] from a loader string.
	#[must_use]
	pub fn from_loader(loaders: &[String]) -> Option<Self> {
		if loaders
			.iter()
			.any(|x| ["fabric", "forge", "quilt"].contains(&&**x))
		{
			Some(Self::Mod)
		} else if loaders.iter().any(|x| x == "datapack") {
			Some(Self::DataPack)
		} else if loaders.iter().any(|x| ["iris", "optifine"].contains(&&**x)) {
			Some(Self::ShaderPack)
		} else if loaders
			.iter()
			.any(|x| ["vanilla", "canvas", "minecraft"].contains(&&**x))
		{
			Some(Self::ResourcePack)
		} else {
			None
		}
	}

	#[must_use]
	pub fn from_parent(path: &Path) -> Option<Self> {
		let path = path.parent()?.file_name()?;
		match path.to_str()? {
			"mods" => Some(Self::Mod),
			"datapacks" => Some(Self::DataPack),
			"resourcepacks" => Some(Self::ResourcePack),
			"shaderpacks" => Some(Self::ShaderPack),
			"modpack" => Some(Self::ModPack),
			_ => None,
		}
	}

	#[must_use]
	pub const fn get_name(&self) -> &'static str {
		match self {
			Self::Mod => "mod",
			Self::DataPack => "datapack",
			Self::ResourcePack => "resourcepack",
			Self::ShaderPack => "shader",
			Self::ModPack => "modpack",
		}
	}

	#[must_use]
	pub const fn get_folder(&self) -> &'static str {
		match self {
			Self::Mod => "mods",
			Self::DataPack => "datapacks",
			Self::ResourcePack => "resourcepacks",
			Self::ShaderPack => "shaderpacks",
			Self::ModPack => "modpack",
		}
	}

	#[must_use]
	pub fn get_meta(&self) -> PathBuf {
		PathBuf::from(format!(
			"{}/{}",
			self.get_folder(),
			self.get_meta_file_name()
		))
	}

	#[must_use]
	pub fn get_meta_file_name(&self) -> String {
		String::from(".packages.json")
	}

	pub async fn file_matches(&self, entry: &DirEntry) -> crate::Result<bool> {
		if !(entry.path().try_exists()?) {
			return Ok(false);
		}

		Ok(match self {
			Self::Mod => {
				entry.file_type().await?.is_file()
					&& entry.path().extension() == Some("jar".as_ref())
			}
			_ => false,
		})
	}

	pub fn iterator() -> impl Iterator<Item = Self> {
		[
			Self::Mod,
			Self::DataPack,
			Self::ResourcePack,
			Self::ShaderPack,
			Self::ModPack,
		]
		.iter()
		.copied()
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
	pub downloads: u64,
	#[serde(default)]
	pub icon_url: String,
	pub project_id: String,
	pub author: String,
	pub versions: Vec<String>,
	pub follows: u32,
	pub date_created: Option<DateTime<Utc>>,
	pub date_modified: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProviderSearchResults {
	pub provider: Providers,
	pub results: Vec<SearchResult>,
	pub total: u32,
}

/// A struct that represents a Package.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Package {
	pub sha1: String,
	pub meta: PackageMetadata,
	pub file_name: String,
	pub disabled: bool,
}

impl Package {
	pub fn new(path: &PackagePath, meta: PackageMetadata) -> crate::Result<Self> {
		let file_name = path
			.0
			.file_name()
			.ok_or_else(|| crate::ErrorKind::AnyhowError(anyhow::anyhow!("no file name")))?
			.to_string_lossy()
			.to_string();

		let sha1 = crypto::sha1_file(&path.0)?;

		Ok(Self {
			sha1,
			meta,
			file_name,
			disabled: false,
		})
	}
}

pub type PackagesMap = HashMap<PackagePath, Package>;

pub struct Packages {
	managers: HashMap<ClusterPath, PackageManager>,
}

impl Packages {
	pub async fn initialize(clusters: &Clusters) -> Self {
		let mut this = Self { managers: HashMap::new() };

		// TODO: This should probably not clone and store the cluster path in like 2 areas
		for cluster_path in clusters.0.keys() {
			this.add_cluster(cluster_path.clone()).await;
		}

		this
	}

	pub async fn add_cluster(&mut self, cluster_path: ClusterPath) {
		let mgr = PackageManager::new(cluster_path.clone());
		self.managers.insert(cluster_path, mgr);
	}

	#[must_use]
	pub fn get(&self, cluster_path: &ClusterPath) -> Option<&PackageManager> {
		self.managers.get(cluster_path)
	}

	pub fn get_mut(&mut self, cluster_path: &ClusterPath) -> Option<&mut PackageManager> {
		self.managers.get_mut(cluster_path)
	}
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PackagesMeta {
	pub packages: PackagesMap,
	pub package_type: PackageType,
}

impl PackagesMeta {
	#[must_use]
	pub fn new(package_type: PackageType) -> Self {
		Self {
			packages: PackagesMap::new(),
			package_type,
		}
	}
}

#[derive(Debug)]
pub struct PackageManager {
	pub cluster_path: ClusterPath,
	synced: bool,
	modpacks: PackagesMeta,
	mods: PackagesMeta,
	datapacks: PackagesMeta,
	resourcepacks: PackagesMeta,
	shaderpacks: PackagesMeta,
}

impl PackageManager {
	#[must_use]
	pub fn new(cluster_path: ClusterPath) -> Self {
		Self {
			cluster_path,
			modpacks: PackagesMeta::new(PackageType::ModPack),
			synced: false,
			mods: PackagesMeta::new(PackageType::Mod),
			datapacks: PackagesMeta::new(PackageType::DataPack),
			resourcepacks: PackagesMeta::new(PackageType::ResourcePack),
			shaderpacks: PackagesMeta::new(PackageType::ShaderPack),
		}
	}

	pub async fn get_meta_file(
		&self,
		dirs: &Directories,
		package_type: PackageType,
	) -> crate::Result<PathBuf> {
		Ok(self
			.cluster_path
			.full_path_dirs(dirs)
			.await?
			.join(package_type.get_meta()))
	}

	#[async_recursion::async_recursion]
	async fn get_and_sync(&mut self, dirs: Option<&Directories>) -> &mut Self {
		if !self.synced {
			self.synced = true; // early set to prevent infinite loop

			let state = State::get().await.expect("couldn't get state when syncing packages");
			let dirs = match dirs {
				Some(dirs) => dirs,
				None => &state.directories,
			};

			self.sync_packages(dirs).await;
		}

		self
	}

	/// Get the `PackagesMeta` for a specific package type. Syncs.
	#[must_use]
	pub async fn get(&mut self, package_type: PackageType) -> &PackagesMeta {
		self.get_and_sync(None).await;
		match package_type {
			PackageType::Mod => &self.mods,
			PackageType::DataPack => &self.datapacks,
			PackageType::ResourcePack => &self.resourcepacks,
			PackageType::ShaderPack => &self.shaderpacks,
			PackageType::ModPack => &self.modpacks,
		}
	}

	/// Get the `PackagesMeta` for a specific package type. Does not sync.
	fn get_mut(&mut self, package_type: PackageType) -> &mut PackagesMeta {
		match package_type {
			PackageType::Mod => &mut self.mods,
			PackageType::ModPack => &mut self.modpacks,
			PackageType::DataPack => &mut self.datapacks,
			PackageType::ResourcePack => &mut self.resourcepacks,
			PackageType::ShaderPack => &mut self.shaderpacks,
		}
	}

	/// Get the PackagesMeta for a specific package type. Does not sync.
	#[tracing::instrument]
	async fn get_from_file(
		&self,
		dirs: &Directories,
		package_type: PackageType,
	) -> crate::Result<PackagesMeta> {
		let packages_meta = &self.get_meta_file(dirs, package_type).await?;

		let meta: PackagesMeta = if packages_meta.exists() && packages_meta.is_file() {
			if let Ok(meta) = serde_json::from_slice(&io::read(packages_meta).await?) {
				meta
			} else {
				io::copy(packages_meta, packages_meta.with_extension("bak")).await?;
				io::write(packages_meta, "{}").await?;
				PackagesMeta::new(package_type)
			}
		} else {
			io::write(packages_meta, "{}").await?;
			PackagesMeta::new(package_type)
		};

		Ok(meta)
	}

	// add a package to the manager
	#[tracing::instrument(skip(self, package))]
	pub async fn add_package(
		&mut self,
		package_path: PackagePath,
		package: Package,
		package_type: Option<PackageType>,
	) -> crate::Result<()> {
		let package_type = package
			.meta
			.get_package_type()
			.unwrap_or(package_type.ok_or_else(|| anyhow::anyhow!("no package type"))?);
		let packages = &mut self.get_mut(package_type).packages;
		packages.insert(package_path, package);

		Ok(())
	}

	// remove a package to the manager
	#[tracing::instrument(skip(self))]
	pub async fn remove_package(
		&mut self,
		package_path: &PackagePath,
		package_type: PackageType,
	) -> crate::Result<()> {
		let full_path = self
			.cluster_path
			.clone()
			.full_path()
			.await?
			.join(package_type.get_folder())
			.join(package_path.0.clone());
		io::remove_file(full_path).await?;

		let packages = &mut self.get_mut(package_type).packages;
		packages.remove(package_path);

		Ok(())
	}

	/// sync all packages
	#[tracing::instrument(skip(self, dirs))]
	pub async fn sync_packages(&mut self, dirs: &Directories) {
		tracing::info!("syncing packages for cluster {}", self.cluster_path);
		let package_types = vec![
			PackageType::Mod,
			PackageType::DataPack,
			PackageType::ResourcePack,
			PackageType::ShaderPack,
		];

		for package_type in package_types {
			if let Err(err) = &self.sync_packages_by_type(dirs, package_type, None).await {
				tracing::error!("failed to sync package {}: {}", package_type.get_name(), err);
			}
		}
	}

	/// sync packages from the metafile
	#[tracing::instrument(skip(self, dirs))]
	async fn sync_from_file_by_type(
		&mut self,
		dirs: &Directories,
		package_type: PackageType,
	) -> crate::Result<()> {
		let packages_meta = self.get_from_file(dirs, package_type).await?;
		self.get_mut(package_type).packages = packages_meta.packages;

		Ok(())
	}

	/// sync packages from the manager to the metafile
	#[tracing::instrument(skip(self, dirs))]
	async fn sync_to_file_by_type(
		&mut self,
		dirs: &Directories,
		package_type: PackageType,
	) -> crate::Result<()> {
		let packages_meta = self.get(package_type).await;
		let packages_meta = serde_json::to_string(&packages_meta)?;

		io::write(self.get_meta_file(dirs, package_type).await?, packages_meta).await?;

		Ok(())
	}

	/// returns a list of packages that have a file but are not synced in the manager
	#[tracing::instrument(skip(self, dirs, packages))]
	async fn sync_packages_from_local(
		&self,
		dirs: &Directories,
		packages: &mut PackagesMap,
		package_type: PackageType,
	) -> crate::Result<()> {
		let mut files = io::read_dir(
			self.cluster_path
				.full_path_dirs(dirs)
				.await?
				.join(package_type.get_folder()),
		)
		.await?;

		let mut packages_to_keep = HashSet::<PackagePath>::new();
		let mut packages_to_infer = HashSet::<PathBuf>::new();

		while let Some(file) = files.next_entry().await? {
			// Skip .packages.json meta file
			if file.file_name().to_string_lossy().eq(&package_type.get_meta_file_name()) || file.file_type().await?.is_dir()
			{
				continue;
			}

			let package_path = PackagePath::new(&file.path());

			// Check if the file is in the packages list already
			if let None = packages.get(&package_path) {
				// Package path is not in the packages list but exists on file system, lets try to infer it later on
				packages_to_infer.insert(package_path.0.clone());
			}

			// Add the package to the set to not remove it from the manager
			packages_to_keep.insert(package_path);
		}

		// Infer any packages
		if packages_to_infer.len() > 0 {
			if let Some(not_found) = infer_provider(Providers::Modrinth, package_type, packages, packages_to_infer, crypto::sha1_file).await {
				if let Some(not_found) = infer_provider(Providers::SkyClient, package_type, packages, not_found, crypto::md5_file).await {
					if let Some(not_found) = infer_provider(Providers::Curseforge, package_type, packages, not_found, |path| {
						let hash = crypto::murmur2_file(path)?;
						Ok(hash.to_string())
					}).await {
						for path in not_found {
							tracing::warn!("failed to infer package: {:?}", path);

							let package_path = PackagePath::new(&path);
							let meta = PackageMetadata::Unknown;
							let package = Package::new(&package_path, meta)?;

							packages.insert(package_path, package);
						}
					}
				}
			}
		}

		if packages.len() != packages_to_keep.len() {
			packages.retain(|pkg_path, _| packages_to_keep.contains(pkg_path));
		}

		Ok(())
	}

	// sync packages in a cluster
	#[tracing::instrument(skip(self, dirs))]
	pub async fn sync_packages_by_type(
		&mut self,
		dirs: &Directories,
		package_type: PackageType,
		clear: Option<bool>,
	) -> crate::Result<()> {
		if clear.unwrap_or(false) {
			self.get_mut(package_type).packages.clear();
		}

		// Clone the current packages and merge them with the local package list
		let mut packages = self.get(package_type).await.packages.clone();
		let mut new_packages = self.get_from_file(dirs, package_type).await?.packages;
		packages.extend(new_packages.drain());

		// Sync the packages from FS
		self.sync_packages_from_local(dirs, &mut packages, package_type).await?;

		// Finally store the new list in memory and on disk
		let stored_packages = &self.get_mut(package_type).packages;
		if stored_packages.len() != packages.len() {
			self.get_mut(package_type).packages = packages;
			self.sync_to_file_by_type(dirs, package_type).await?;
		}

		Ok(())
	}
}

// #[tracing::instrument(skip(packages))]
// async fn infer_packages(
// 	packages: &mut PackagesMap,
// 	package_type: PackageType,
// 	to_infer: HashSet<PathBuf>,
// ) {
// 	let infer_order = vec![
// 		Providers::Modrinth,
// 		Providers::SkyClient,
// 		Providers::Curseforge,
// 	];


// 	// if let Some(not_found) = infer_provider(Providers::Modrinth, package_type, packages, packages_to_infer, crypto::sha1_file).await {
// 	// 	if let Some(not_found) = infer_provider(Providers::SkyClient, package_type, packages, not_found, crypto::md5_file).await {
// 	// 		if let Some(not_found) = infer_provider(Providers::Curseforge, package_type, packages, not_found, |path| {
// 	// 			let hash = crypto::murmur2_file(path)?;
// 	// 			Ok(hash.to_string())
// 	// 		}).await {
// 	// 		}
// 	// 	}
// 	// }

// 	let mut to_infer = to_infer;
// 	for provider in infer_order {
// 		if let Some(not_found) = infer_provider(provider, package_type, packages, to_infer, move|path| {
// 			Ok(provider.hash_file(path))
// 		}).await {
// 			to_infer = not_found;
// 		}
// 	}

// 	for path in to_infer {
// 		tracing::warn!("failed to infer package: {:?}", path);

// 		let package_path = PackagePath::new(&path);
// 		let meta = PackageMetadata::Unknown;
// 		if let Ok(package) = Package::new(&package_path, meta) {
// 			packages.insert(package_path, package);
// 		}
// 	}
// }

#[tracing::instrument(skip(packages, to_infer))]
async fn infer_provider(
	provider: Providers,
	package_type: PackageType,
	packages: &mut PackagesMap,
	to_infer: HashSet<PathBuf>,
	hash_fn: fn(&PathBuf) -> crate::Result<String>,
) -> Option<HashSet<PathBuf>> {
	// TODO: Add ingress

	let mut hash_map = HashMap::<String, PathBuf>::new();
	to_infer.iter().for_each(|path| {
		if let Ok(hash) = hash_fn(path) {
			hash_map.insert(hash, path.clone());
		}
	});


	let hash_len = hash_map.len();
	match provider.get_versions_by_hashes(hash_map.iter().map(|hash| hash.0.clone()).collect()).await {
		Ok(versions) => {

			let found_all = (hash_len - versions.len()) <= 0;

			let havent_found = if !found_all {
				Some(hash_map.iter().filter_map(|(hash, path)| {
					if versions.contains_key(hash) {
						None
					} else {
						Some(path.clone())
					}
				}).collect::<HashSet<PathBuf>>())
			} else {
				None
			};

			let package_ids = versions.iter().map(|(hash, version)| (version.package_id.clone(), hash.clone())).collect::<HashMap<String, String>>();

			match provider.get_multiple(package_ids.clone().into_keys().collect::<Vec<String>>().as_slice()).await {
				Ok(managed_packages) => {
					for managed in managed_packages {
						let hash = match package_ids.get(&managed.id) {
							Some(hash) => hash,
							None => continue,
						};

						let version = match versions.get(hash) {
							Some(version) => version,
							None => continue,
						};

						if let Some(package_path) = hash_map.get(hash) {
							let meta = PackageMetadata::Managed {
								package_id: managed.id,
								provider: provider.clone(),
								package_type,
								title: managed.title,
								version_id: version.id.clone(),
								version_formatted: version.version_display.clone(),
								mc_versions: Some(version.game_versions.clone()),
								icon_url: managed.icon_url
							};

							let package_path = &PackagePath::new(package_path);
							match Package::new(package_path, meta) {
								Ok(package) => {
									packages.insert(package_path.clone(), package);
								},
								Err(err) => {
									tracing::error!("failed to create package from {}: {}", provider, err);
								},
							};
						}
					}
				},
				Err(err) => {
					tracing::error!("failed to get versions from {}: {}", provider, err);
				}
			}

			return havent_found;
		},
		Err(err) => {
			tracing::error!("failed to get versions from {}: {}", provider, err);
		}
	};

	Some(to_infer)
}

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
		#[serde(skip_serializing_if = "Option::is_none", default)]
		mc_versions: Option<Vec<String>>,
		#[serde(skip_serializing_if = "Option::is_none", default)]
		icon_url: Option<String>,
	},
	Unknown,
}

impl PackageMetadata {
	#[must_use]
	pub fn from_managed_package(package: ManagedPackage, version: ManagedVersion) -> Self {
		Self::Managed {
			package_id: package.id,
			version_id: version.id,
			version_formatted: version.version_display,
			title: package.title,
			provider: package.provider,
			package_type: package.package_type,
			mc_versions: Some(version.game_versions),
			icon_url: package.icon_url,
		}
	}

	#[must_use]
	pub const fn get_package_type(&self) -> Option<PackageType> {
		match self {
			Self::Managed { package_type, .. } => Some(*package_type),
			_ => None,
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
	#[serde(default = "default_is_organization_user")]
	pub is_organization_user: bool,
	#[serde(default)]
	pub role: Option<String>,
}

const fn default_is_organization_user() -> bool {
	false
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

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PackageBody {
	Url(String),
	Markdown(String),
}

/// Universal metadata for any managed package from a Mod distribution platform.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManagedPackage {
	// Core Metadata
	pub provider: Providers,
	pub id: String,
	pub package_type: PackageType,
	pub title: String,
	pub description: String,
	pub body: PackageBody,
	pub main: String,
	pub versions: Vec<String>,
	pub game_versions: Vec<String>,
	pub loaders: Vec<Loader>,
	pub icon_url: Option<String>,

	pub created: Option<DateTime<Utc>>,
	pub updated: Option<DateTime<Utc>>,
	pub client: PackageSide,
	pub server: PackageSide,
	pub downloads: u64,
	pub followers: u32,
	pub categories: Vec<String>,
	pub optional_categories: Option<Vec<String>>,
	pub license: Option<License>,
	pub author: Author,

	pub is_archived: bool,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
	pub version_display: String,
	pub changelog: String,
	pub changelog_url: Option<String>,

	pub published: Option<DateTime<Utc>>,
	pub downloads: u32,
	pub version_type: ManagedVersionReleaseType,

	pub files: Vec<ManagedVersionFile>,
	pub is_available: bool,
	pub deps: Vec<ManagedDependency>,
	pub game_versions: Vec<String>,
	pub loaders: Vec<Loader>,
}

impl ManagedVersion {
	#[must_use]
	pub fn get_primary_file(&self) -> Option<&ManagedVersionFile> {
		self.files.iter().find(|f| f.primary)
	}
}

/// Universal version release type
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub enum ManagedVersionReleaseType {
	#[default]
	Release,
	Alpha,
	Beta,
	Snapshot,
}

impl From<String> for ManagedVersionReleaseType {
	fn from(s: String) -> Self {
		match s.to_lowercase().as_str() {
			"alpha" => Self::Alpha,
			"beta" => Self::Beta,
			"snapshot" => Self::Snapshot,
			_ => Self::Release,
		}
	}
}

impl ToString for ManagedVersionReleaseType {
	fn to_string(&self) -> String {
		match self {
			Self::Release => "Release",
			Self::Alpha => "Alpha",
			Self::Beta => "Beta",
			Self::Snapshot => "Snapshot",
		}.to_string()
	}
}

/// Universal interface for managed package files.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct ManagedVersionFile {
	pub url: String,
	pub file_name: String,
	pub primary: bool,

	pub size: u64,
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
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
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
