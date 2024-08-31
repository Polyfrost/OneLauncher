//! Core cluster state management

use chrono::{DateTime, Utc};
use futures::prelude::*;
use interpulse::api::modded::LoaderVersion;
use notify::RecommendedWatcher;
use notify_debouncer_mini::Debouncer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::constants::CLUSTER_FILE;
use crate::proxy::send::{send_cluster, send_message};
use crate::proxy::ClusterPayloadType;
use crate::utils::http::{write_icon, IoSemaphore};
use crate::utils::io::{self, IOError};
use crate::utils::java::JavaVersion;
use crate::State;

use super::{Directories, InitHooks, Memory, PackageType, Resolution};

/// Core Cluster state manager with a [`HashMap<ClusterPath, Cluster>`].
pub(crate) struct Clusters(pub HashMap<ClusterPath, Cluster>);

/// Core Cluster stages used in package logic.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClusterStage {
	/// Installed with no downloads left.
	Installed,
	/// Downloading any sort of core metadata or mod file.
	Downloading,
	/// Downloading a full pack file, which is granted lower priority due to the high network demand.
	PackDownloading,
	/// Not installed at all.
	#[default]
	NotInstalled,
}

impl ClusterStage {
	pub fn as_str(&self) -> &'static str {
		match *self {
			Self::Installed => "installed",
			Self::Downloading => "downloading",
			Self::PackDownloading => "pack_downloading",
			Self::NotInstalled => "not_installed",
		}
	}
}

impl std::str::FromStr for ClusterStage {
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"installed" => Self::Installed,
			"downloading" => Self::Downloading,
			"pack_downloading" => Self::PackDownloading,
			"not_installed" => Self::NotInstalled,
			_ => Self::NotInstalled,
		})
	}

	type Err = crate::Error;
}

/// Relative Path wrapper to be used as an identifer for a cluster path.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
#[serde(transparent)]
pub struct ClusterPath(pub PathBuf);

impl ClusterPath {
	/// Get a new [`ClusterPath`] instance from an [`Into<PathBuf>`].
	pub fn new(path: impl Into<PathBuf>) -> Self {
		ClusterPath(path.into())
	}

	pub async fn find_by_uuid(uuid: Uuid) -> crate::Result<Self> {
		Ok(crate::cluster::get_by_uuid(uuid)
			.await?
			.ok_or(anyhow::anyhow!("Cluster does not exist"))?
			.cluster_path())
	}

	/// Get the full [`PathBuf`] of the current cluster path.
	pub async fn full_path(&self) -> crate::Result<PathBuf> {
		let state = State::get().await?;
		let clusters_dir = state.directories.clusters_dir().await;
		Ok(clusters_dir.join(&self.0))
	}

	/// Get the full [`PathBuf`] of the current cluster path.
	pub async fn full_path_dirs(&self, dirs: &Directories) -> crate::Result<PathBuf> {
		let clusters_dir = dirs.clusters_dir().await;
		Ok(clusters_dir.join(&self.0))
	}

	/// Validate the UTF of a cluster path.
	pub fn validate(&self) -> crate::Result<&Self> {
		self.0.to_str().ok_or(anyhow::anyhow!(
			"invalid file path string {}!",
			self.0.clone().to_string_lossy()
		))?;
		Ok(self)
	}

	/// Validate the cluster and clone the current [`ClusterPath`].
	pub async fn cluster_path(&self) -> crate::Result<ClusterPath> {
		if let Some(c) = crate::cluster::get(self).await? {
			Ok(c.cluster_path())
		} else {
			Err(anyhow::anyhow!(
				"failed to get path of unmanaged or corrupted cluster {}",
				self.to_string()
			)
			.into())
		}
	}

	/// Create a [`ClusterPath`] from a full [`PathBuf`].
	pub async fn from_path(path: PathBuf) -> crate::Result<Self> {
		let path: PathBuf = io::canonicalize(path)?;
		let clusters_dir = io::canonicalize(State::get().await?.directories.clusters_dir().await)?;
		path.strip_prefix(clusters_dir)
			.ok()
			.and_then(|f| f.file_name())
			.ok_or_else(|| {
				anyhow::anyhow!("path {} is not a cluster path", path.to_string_lossy())
			})?;

		Ok(Self(path))
	}
}

impl std::fmt::Display for ClusterPath {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.0.display().fmt(f)
	}
}

/// Used for backwards compatibility for modpacks which handle windows paths strangely.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
#[serde(into = "RawPackagePath", from = "RawPackagePath")]
pub struct InnerPathLinux(pub String);

impl InnerPathLinux {
	/// Get the first 2 components of the inner path.
	pub fn get_components(&self) -> String {
		self.to_string()
			.split('/')
			.take(2)
			.collect::<Vec<_>>()
			.join("/")
	}
}

impl std::fmt::Display for InnerPathLinux {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl From<RawPackagePath> for InnerPathLinux {
	fn from(value: RawPackagePath) -> Self {
		InnerPathLinux(value.0.replace('\\', "/"))
	}
}

/// Used for backwards compatibility for modpacks which handle windows paths strangely.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RawPackagePath(pub String);

impl From<InnerPathLinux> for RawPackagePath {
	fn from(value: InnerPathLinux) -> Self {
		RawPackagePath(value.0)
	}
}

// TODO: make it not a pathbuf, just get rid of all packagepath and clusterpath into str. maybe even just go straight to prisma with strings

/// Relative [`PathBuf`] for a specific [`Package`] of a [`Cluster`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
#[serde(transparent)]
pub struct PackagePath(pub PathBuf);

impl PackagePath {
	/// Transform a full [`PathBuf`] into a relative [`PackagePath`].
	pub async fn from_path(path: &PathBuf) -> crate::Result<Self> {
		let clusters_dir: PathBuf =
			std::fs::canonicalize(State::get().await?.directories.clusters_dir().await)?;
		let path: PathBuf = std::fs::canonicalize(path)?;
		let path = path
			.strip_prefix(clusters_dir)
			.ok()
			.map(|p| p.components().skip(1).collect::<PathBuf>())
			.ok_or_else(|| {
				anyhow::anyhow!("path {path:?} does not exist in a cluster!", path = path)
			})?;

		Ok(Self(path))
	}

	/// Get the full [`PathBuf`] of the current package path.
	pub async fn full_path(&self, cluster: ClusterPath) -> crate::Result<PathBuf> {
		let cluster_dir = cluster.full_path().await?;
		Ok(cluster_dir.join(&self.0))
	}

	/// Get the [`InnerPathLinux`] of a [`PackagePath`].
	pub fn inner_path(&self) -> InnerPathLinux {
		InnerPathLinux(
			self.0
				.components()
				.map(|c| c.as_os_str().to_string_lossy().to_string())
				.collect::<Vec<_>>()
				.join("/"),
		)
	}

	/// Create a new PackagePath from a relative path.
	pub fn new(path: &Path) -> Self {
		PackagePath(PathBuf::from(path))
	}
}

/// Represents a single Instance and installation of Minecraft
/// Contains settings and identifiers on a per-Cluster basis, falling back to default settings for Options<>
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Cluster {
	/// The Uuid of a specific cluster.
	pub uuid: Uuid,
	/// The download stage that of a cluster.
	#[serde(default)]
	pub stage: ClusterStage,
	/// The core path of the cluster.
	#[serde(default)]
	pub path: PathBuf,
	/// The associated cluster metadata.
	pub meta: ClusterMeta,
	/// The per-cluster JVM memory allocation options.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub memory: Option<Memory>,
	/// The per-cluster JVM options.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub java: Option<JavaOptions>,
	/// The per-cluster Minecraft window default resolution.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub resolution: Option<Resolution>,
	/// The per-cluster Minecraft window fullscreen status.
	pub force_fullscreen: Option<bool>,
	// The per-cluster initialization hooks.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub init_hooks: Option<InitHooks>,
}

/// Represents core Cluster metadata ([`Cluster#meta`]).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClusterMeta {
	/// The user-facing name of the cluster stored as a string.
	pub name: String,

	/// The user-facing group of the cluster stored as a string.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub group: Option<String>,
	/// The associated Minecraft version of the cluster as last updated.
	pub mc_version: String,

	/// The associated mod [`Loader`] as specified in the cluster.
	#[serde(default)]
	pub loader: Loader,
	/// The associated mod [`LoaderVersion`] if available.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub loader_version: Option<LoaderVersion>,

	/// The time that the cluster was created in [`DateTime<Utc>`].
	#[serde(default)]
	pub created_at: DateTime<Utc>,
	/// The time the cluster was last modified in [`DateTime<Utc>`].
	#[serde(default)]
	pub modified_at: DateTime<Utc>,
	/// The last time the cluster was played in [`DateTime<Utc>`].
	/// (Defaults to None if the cluster has never been played)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub played_at: Option<DateTime<Utc>>,
	/// The overall time played in seconds stored as a [`u64`].
	#[serde(default)]
	pub overall_played: u64,
	/// The recent time played stored as a [`u64`].
	#[serde(default)]
	pub recently_played: u64,

	/// The associated [`PackageData`] and modpack data for the cluster.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub package_data: Option<PackageData>,

	/// The user-facing cluster icon as a [`PathBuf`].
	#[serde(skip_serializing_if = "Option::is_none")]
	pub icon: Option<PathBuf>,
	/// The user-facing cluster icon as a URL string.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub icon_url: Option<String>,
}

/// Optional data used to link a specific cluster to a package project.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PackageData {
	/// The package ID as a String.
	pub package_id: Option<String>,
	/// The version of the package as a String.
	pub version_id: Option<String>,
	/// Whether or not the current package is locked (for legacy modpack support).
	#[serde(default = "default_locked")]
	pub locked: Option<bool>,
}

pub fn default_locked() -> Option<bool> {
	Some(true)
}

/// Available mod loaders to be used for a cluster.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Eq, PartialEq, Clone, Copy, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
	/// The default vanilla loader, no modding supported.
	#[default]
	Vanilla,
	/// The MinecraftForge Minecraft mod loader.
	Forge,
	/// The FabircMC Minecraft mod loader.
	Fabric,
	/// The NeoForge Minecraft mod loader.
	NeoForge,
	/// The Quilt Minecraft mod loader.
	Quilt,
	/// The Legacy Fabric port mod loader.
	LegacyFabric,
}

impl Loader {
	pub fn as_str(&self) -> &'static str {
		match *self {
			Self::Vanilla => "vanilla",
			Self::Forge => "forge",
			Self::Fabric => "fabric",
			Self::Quilt => "quilt",
			Self::NeoForge => "neoforge",
			Self::LegacyFabric => "legacyfabric",
		}
	}

	pub fn as_meta_str(&self) -> &'static str {
		match *self {
			Self::Vanilla => "vanilla",
			Self::Forge => "forge",
			Self::Fabric => "fabric",
			Self::Quilt => "quilt",
			Self::NeoForge => "neo",
			Self::LegacyFabric => "legacyfabric",
		}
	}

	pub fn from_string(val: &str) -> Self {
		match val {
			"vanilla" => Self::Vanilla,
			"forge" => Self::Forge,
			"fabric" => Self::Fabric,
			"quilt" => Self::Quilt,
			"neoforge" => Self::NeoForge,
			_ => Self::Vanilla,
		}
	}

	pub fn supports_mods(&self) -> bool {
		!matches!(self, Self::Vanilla)
	}
}

impl TryFrom<String> for Loader {
	type Error = crate::ErrorKind;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		Ok(match value.to_lowercase().as_str() {
			"vanilla" => Self::Vanilla,
			"forge" => Self::Forge,
			"fabric" => Self::Fabric,
			"neoforge" => Self::NeoForge,
			"quilt" => Self::Quilt,
			"legacyfabric" | "legacy_fabric" => Self::LegacyFabric,
			_ => return Err(anyhow::anyhow!("invalid loader type").into()),
		})
	}
}

impl std::fmt::Display for Loader {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match *self {
			Self::Vanilla => "Vanilla",
			Self::Forge => "Forge",
			Self::Fabric => "Fabric",
			Self::NeoForge => "NeoForge",
			Self::Quilt => "Quilt",
			Self::LegacyFabric => "LegacyFabric",
		})
	}
}

/// Custom Java arguments on a per-cluster basis.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct JavaOptions {
	/// A custom java version from the global java store, if specified.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub custom_version: Option<JavaVersion>,
	/// Custom runtime arguments when running the cluster.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub custom_arguments: Option<Vec<String>>,
	/// Custom environment variables when running the cluster.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub custom_env_arguments: Option<Vec<(String, String)>>,
}

impl Cluster {
	/// Create a new Cluster
	#[tracing::instrument]
	pub async fn new(uuid: Uuid, name: String, version: String) -> crate::Result<Self> {
		if name.trim().is_empty() {
			return Err(anyhow::anyhow!("invalid instance name (empty)").into());
		}

		Ok(Self {
			uuid,
			stage: ClusterStage::NotInstalled,
			path: PathBuf::new().join(&name),
			meta: ClusterMeta {
				name,
				group: None,
				mc_version: version,
				loader: Loader::Vanilla,
				loader_version: None,
				created_at: Utc::now(),
				modified_at: Utc::now(),
				package_data: None,
				played_at: None,
				overall_played: 0,
				recently_played: 0,
				icon: None,
				icon_url: None,
			},
			java: None,
			memory: None,
			resolution: None,
			force_fullscreen: None,
			init_hooks: None,
		})
	}

	/// Get the [`ClusterPath`] of the specified [`Cluster`].
	#[inline]
	pub fn cluster_path(&self) -> ClusterPath {
		ClusterPath::new(&self.path)
	}

	/// Set the icon [`bytes::Bytes`] for this cluster.
	#[tracing::instrument(skip(self, io_semaphore, icon))]
	pub async fn set_icon_path<'a>(
		&'a mut self,
		cache_path: &Path,
		io_semaphore: &IoSemaphore,
		icon: &Path,
		file_name: &str,
	) -> crate::Result<()> {
		let icon = io::read(icon).await?;
		self.set_icon_bytes(cache_path, io_semaphore, icon.into(), file_name)
			.await?;
		Ok(())
	}

	/// Set the icon [`bytes::Bytes`] for this cluster.
	#[tracing::instrument(skip(self, io_semaphore, icon))]
	pub async fn set_icon_bytes<'a>(
		&'a mut self,
		cache_path: &Path,
		io_semaphore: &IoSemaphore,
		icon: bytes::Bytes,
		file_name: &str,
	) -> crate::Result<()> {
		let file = write_icon(file_name, cache_path, icon, io_semaphore).await?;
		self.meta.icon = Some(file);
		self.meta.modified_at = Utc::now();
		Ok(())
	}

	/// Handle a cluster crash.
	pub fn handle_crash(path: ClusterPath) {
		tokio::task::spawn(async move {
			let res = async {
				let cluster = crate::api::cluster::get(&path).await?;
				if let Some(cluster) = cluster {
					if cluster.stage == ClusterStage::Installed {
						send_message(&format!(
							"cluster {} has crashed! visit the logs page for a crash report.",
							cluster.meta.name
						))
						.await?;
					}
				}

				Ok::<(), crate::Error>(())
			}
			.await;

			match res {
				Ok(()) => {}
				Err(err) => tracing::warn!("failed to send crash report to frontend {err}"),
			};
		});
	}

	/// get the full path to the current cluster.
	pub async fn get_full_path(&self) -> crate::Result<PathBuf> {
		let state = State::get().await?;
		let clusters_dir = state.directories.clusters_dir().await;
		Ok(clusters_dir.join(&self.path))
	}

	/// get full paths and subpaths
	pub async fn get_full_subs(&self) -> crate::Result<Vec<PathBuf>> {
		let mut files = Vec::new();
		let cluster_path = self.get_full_path().await?;
		let mut paths = |path: &str| {
			let new = cluster_path.join(path);
			if new.exists() {
				for sub in std::fs::read_dir(&new).map_err(|e| IOError::with_path(e, &new))? {
					let sub = sub.map_err(IOError::from)?.path();
					if sub.is_file() {
						files.push(sub);
					}
				}
			}
			Ok::<(), crate::Error>(())
		};

		paths(PackageType::Mod.get_folder())?;
		paths(PackageType::ShaderPack.get_folder())?;
		paths(PackageType::ResourcePack.get_folder())?;
		paths(PackageType::DataPack.get_folder())?;

		Ok(files)
	}

	/// watch the filesystem for changes with [`notify`].
	#[tracing::instrument(skip(watcher))]
	#[onelauncher_macros::memory]
	pub async fn watch(
		cluster_path: &Path,
		watcher: &mut Debouncer<RecommendedWatcher>,
	) -> crate::Result<()> {
		async fn watch_path(
			cluster_path: &Path,
			watcher: &mut Debouncer<RecommendedWatcher>,
			path: &str,
		) -> crate::Result<()> {
			let path = cluster_path.join(path);
			io::create_dir_all(&path).await?;
			watcher
				.watcher()
				.watch(&cluster_path.join(path), notify::RecursiveMode::Recursive)?;
			Ok(())
		}

		watch_path(cluster_path, watcher, PackageType::Mod.get_folder()).await?;
		watch_path(cluster_path, watcher, PackageType::ShaderPack.get_folder()).await?;
		watch_path(
			cluster_path,
			watcher,
			PackageType::ResourcePack.get_folder(),
		)
		.await?;
		watch_path(cluster_path, watcher, PackageType::DataPack.get_folder()).await?;
		watch_path(cluster_path, watcher, "crash-reports").await?;

		Ok(())
	}
}

impl Clusters {
	/// Initialize the cluster manager and HashMap.
	#[tracing::instrument(skip(watcher))]
	#[onelauncher_macros::memory]
	pub async fn initialize(
		dirs: &Directories,
		watcher: &mut Debouncer<RecommendedWatcher>,
	) -> crate::Result<Self> {
		let mut clusters = HashMap::new();
		let clusters_dir = dirs.clusters_dir().await;
		io::create_dir_all(&&clusters_dir).await?;
		watcher
			.watcher()
			.watch(&clusters_dir, notify::RecursiveMode::NonRecursive)?;
		let mut files = io::read_dir(&dirs.clusters_dir().await).await?;

		while let Some(file) = files.next_entry().await.map_err(IOError::from)? {
			let path = file.path();
			if path.is_dir() {
				let cluster = match Self::from_dir(&path, dirs).await {
					Ok(cluster) => Some(cluster),
					Err(err) => {
						tracing::warn!("failed to load cluster {err}. skipping");
						None
					}
				};

				if let Some(cluster_) = cluster {
					let path = io::canonicalize(path)?;
					Cluster::watch(&path, watcher).await?;
					clusters.insert(cluster_.cluster_path(), cluster_);
				}
			}
		}

		Ok(Self(clusters))
	}

	/// update registered packages
	#[tracing::instrument]
	#[onelauncher_macros::memory]
	pub async fn update_packages() {
		// TODO: Pacakges
	}

	/// update all available package versions
	#[tracing::instrument]
	#[onelauncher_macros::memory]
	pub async fn update_versions() {
		let res = async {
			let state = State::get().await?;
			let mut updateable: Vec<(ClusterPath, String)> = Vec::new();

			{
				let clusters = state.clusters.read().await;
				for (cluster_path, cluster) in clusters.0.iter() {
					if let Some(package_data) = &cluster.meta.package_data {
						if let Some(linked_package) = &package_data.package_id {
							updateable.push((cluster_path.clone(), linked_package.clone()));
						}
					}
				}
			}

			// TODO

			{
				let clusters = state.clusters.read().await;
				clusters.sync().await?;
			}

			Ok::<(), crate::Error>(())
		}
		.await;

		match res {
			Ok(()) => {}
			Err(err) => tracing::warn!("failed to update managed packages: {err}"),
		};
	}

	/// insert a cluster into the HashMap
	#[tracing::instrument(skip(self, cluster))]
	#[onelauncher_macros::memory]
	pub async fn insert(&mut self, cluster: Cluster, dont_watch: bool) -> crate::Result<&Self> {
		send_cluster(
			cluster.uuid,
			&cluster.cluster_path(),
			&cluster.meta.name,
			crate::proxy::ClusterPayloadType::Inserted,
		)
		.await?;

		if !dont_watch {
			let state = State::get().await?;
			let mut watcher = state.watcher.write().await;
			Cluster::watch(&cluster.get_full_path().await?, &mut watcher).await?;
		}

		let cluster_name = cluster.cluster_path();
		cluster_name.validate()?;
		self.0.insert(cluster_name, cluster);
		Ok(self)
	}

	/// remove a cluster from the HashMap
	#[tracing::instrument(skip(self))]
	pub async fn remove(&mut self, cluster_path: &ClusterPath) -> crate::Result<Option<Cluster>> {
		let cluster = self.0.remove(cluster_path);
		let path = cluster_path.full_path().await?;
		if path.exists() {
			io::remove_dir_all(&path).await?;
		}

		Ok(cluster)
	}

	/// sync all available clusters
	#[tracing::instrument(skip_all)]
	pub async fn sync(&self) -> crate::Result<&Self> {
		let _state = State::get().await?;
		stream::iter(self.0.iter())
			.map(Ok::<_, crate::Error>)
			.try_for_each_concurrent(None, |(_, cluster)| async move {
				let json = serde_json::to_vec(&cluster)?;
				let json_path = cluster.get_full_path().await?.join(CLUSTER_FILE);
				io::write(&json_path, &json).await?;
				Ok::<_, crate::Error>(())
			})
			.await?;

		Ok(self)
	}

	/// read a cluster from a directory
	async fn from_dir(path: &Path, dirs: &Directories) -> crate::Result<Cluster> {
		let json = io::read(&path.join(CLUSTER_FILE)).await?;
		let mut cluster = serde_json::from_slice::<Cluster>(&json)?;
		cluster.path = PathBuf::from(
			path.strip_prefix(dirs.clusters_dir().await)
				.map_err(|err| anyhow::anyhow!("failed to strip prefix {err}"))?,
		);
		Ok(cluster)
	}

	/// sync a cluster
	pub fn sync_cluster(cluster_path: ClusterPath) {
		tokio::task::spawn(async move {
			let span = tracing::span!(tracing::Level::INFO, "sync_cluster");
			let res = async {
				let _span = span.enter();
				let state = State::get().await?;
				let dirs = &state.directories;
				let mut clusters = state.clusters.write().await;

				if let Some(cluster) = clusters.0.get_mut(&cluster_path) {
					if !cluster.get_full_path().await?.exists() {
						send_cluster(
							cluster.uuid,
							&cluster_path,
							&cluster.meta.name,
							ClusterPayloadType::Deleted,
						)
						.await?;
						tracing::debug!("removed non-existant fs cluster!");
						clusters.0.remove(&cluster_path);
					}
				} else if cluster_path.full_path().await?.exists() {
					clusters
						.insert(
							Self::from_dir(&cluster_path.full_path().await?, dirs).await?,
							false,
						)
						.await?;

					tokio::task::spawn(async move {
						// Cluster::sync_packages(&cluster_path).await;
					});
				}
				Ok::<(), crate::Error>(())
			}
			.await;

			match res {
				Ok(()) => {}
				Err(err) => tracing::warn!("failed to fetch a cluster: {err}"),
			};
		});
	}
}
