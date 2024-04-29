//! Handles core directory management
use std::path::PathBuf;
use tokio::sync::RwLock;

use super::Settings;
use crate::constants;

/// The public `settings.json` file used to store the [`Settings`] state.
pub const SETTINGS_FILE: &str = "settings.json";
/// The constant core caches folder.
pub const CACHES_FOLDER: &str = "caches";
/// The constant core clusters folder.
pub const CLUSTERS_FOLDER: &str = "clusters";
/// The constant core metadata folder.
pub const METADATA_FOLDER: &str = "metadata";

/// Directory management and utilities.
#[derive(Debug)]
pub struct Directories {
	/// Settings: All small caches and the core settings.json
	pub settings_dir: PathBuf,
	/// Base Configuration: Clusters and all Assets (should be changable as a setting that can go across file systems)
	/// By default this should be the same as the `settings_dir`
	pub config_dir: RwLock<PathBuf>,
	/// The current working directory of the directory manager.
	pub cwd: PathBuf,
}

impl Directories {
	/// Bootstrap the initial settings directory on first startup.
	pub fn init_settings_dir() -> Option<PathBuf> {
		Self::env_path("ONELAUNCHER_CONFIG")
			.or_else(|| Some(dirs::config_dir()?.join(constants::APP_CONFIG_DIR)))
	}

	/// Bootstrap the initial settings directory on first startup.
	#[inline]
	pub fn init_settings_file() -> crate::Result<PathBuf> {
		let settings_dir = Self::init_settings_dir().ok_or(DirectoryError::ConfigDir)?;
		Ok(settings_dir.join("settings.json"))
	}

	/// Initialize the core directory manager
	#[tracing::instrument]
	pub fn initalize(settings: &Settings) -> crate::Result<Self> {
		let cwd = std::env::current_dir().map_err(|err| DirectoryError::WorkingDir(err))?;
		let settings_dir = Self::init_settings_dir().ok_or(DirectoryError::ConfigDir)?;
		let config_dir = settings
			.config_dir
			.clone()
			.ok_or(DirectoryError::ConfigDir)?;
		Ok(Self {
			settings_dir,
			config_dir: RwLock::new(config_dir),
			cwd,
		})
	}

	/// Get the `config_dir/metadata` folder within the core config directory.
	#[inline]
	pub async fn metadata_dir(&self) -> PathBuf {
		self.config_dir.read().await.join(METADATA_FOLDER)
	}

	/// Get the `config_dir/metadata/libraries` folder for Minecraft libraries.
	#[inline]
	pub async fn libraries_dir(&self) -> PathBuf {
		self.metadata_dir().await.join("libraries")
	}

	/// Get the `config_dir/metadata/java` directory.
	#[inline]
	pub async fn java_dir(&self) -> PathBuf {
		self.metadata_dir().await.join("java_versions")
	}

	/// Get the `config_dir/metadata/caches` directory.
	#[inline]
	pub async fn caches_dir(&self) -> PathBuf {
		self.metadata_dir().await.join("caches")
	}

	/// Bootstrap the core logs directory and get it as a [`PathBuf`].
	#[inline]
	pub fn logs_dir() -> Option<PathBuf> {
		Self::init_settings_dir().map(|d| d.join("logs"))
	}

	/// Get the `settings_dir/settings.json` file as a [`PathBuf`].
	#[inline]
	pub fn settings_file(&self) -> PathBuf {
		self.settings_dir.join(SETTINGS_FILE)
	}

	/// Get a [`PathBuf`] from a provided environment variable.
	#[inline]
	fn env_path(name: &str) -> Option<PathBuf> {
		std::env::var_os(name).map(PathBuf::from)
	}

	/// Get the `config_dir/metadata/assets` directory.
	#[inline]
	pub async fn assets_dir(&self) -> PathBuf {
		self.metadata_dir().await.join("assets")
	}

	/// Get the `config_dir/metadata/assets/indexes` directory.
	#[inline]
	pub async fn index_dir(&self) -> PathBuf {
		self.assets_dir().await.join("indexes")
	}

	/// Get the `config_dir/metadata/assets/objects` directory.
	#[inline]
	pub async fn objects_dir(&self) -> PathBuf {
		self.assets_dir().await.join("objects")
	}

	/// Get the `config_dir/metadata/assets/objects/{hash}` directory.
	#[inline]
	pub async fn object_dir(&self, hash: &str) -> PathBuf {
		self.objects_dir().await.join(&hash[..2]).join(hash)
	}

	/// Get the `config_dir/clusters` directory.
	#[inline]
	pub async fn clusters_dir(&self) -> PathBuf {
		self.config_dir.read().await.join("clusters")
	}

	/// Get the `config_dir/clusters/{uuid}` directory.
	#[inline]
	pub async fn cluster_dir(&self, uuid: uuid::Uuid) -> PathBuf {
		self.clusters_dir().await.join(uuid.to_string())
	}

	/// Get the `{cluster_path}/logs` directory.
	#[inline]
	pub async fn cluster_logs_dir(cluster_path: &super::ClusterPath) -> crate::Result<PathBuf> {
		Ok(cluster_path.full_path().await?.join("logs"))
	}

	/// Get the `config_dir/metadata/resources` directory.
	#[inline]
	pub async fn legacy_assets_dir(&self) -> PathBuf {
		self.metadata_dir().await.join("resources")
	}

	/// Get the `config_dir/metadata/natives` directory.
	#[inline]
	pub async fn natives_dir(&self) -> PathBuf {
		self.metadata_dir().await.join("natives")
	}

	/// Get the `config_dir/metadata/natives/{version}` directory.
	#[inline]
	pub async fn version_natives_dir(&self, version: &str) -> PathBuf {
		self.natives_dir().await.join(version)
	}

	/// Get the Minecraft `config_dir/metadata/versions` directory.
	#[inline]
	pub async fn versions_dir(&self) -> PathBuf {
		self.metadata_dir().await.join("versions")
	}

	/// Get the Minecraft `config_dir/metadata/versions/{version}` directory.
	#[inline]
	pub async fn version_dir(&self, version: &str) -> PathBuf {
		self.versions_dir().await.join(version)
	}

	/// Get the `config_dir/icons` directory.
	#[inline]
	pub async fn icons_dir(&self) -> PathBuf {
		self.config_dir.read().await.join("icons")
	}
}

/// Represents a core directory management error.
#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
	/// An error for when a config directory cannot be initialized.
	#[error("failed to get the user's config directory")]
	ConfigDir,
	/// A wrapper over [`std::io::Error`] for cwd fetching errors.
	#[error("could not open working directory {0}")]
	WorkingDir(std::io::Error),
}
