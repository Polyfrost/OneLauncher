use std::path::PathBuf;

use tokio::sync::OnceCell;

use crate::{constants, LauncherResult};

/// The static [`OnceCell<RwLock<Dirs>>`] for storing the global directory state.
/// Should be initialized as soon as possible (preferably before logging)
static DIRS_GLOBAL: OnceCell<Dirs> = OnceCell::const_new();

pub struct Dirs {
	base_dir: PathBuf,
}

impl Dirs {
	pub async fn get() -> LauncherResult<&'static Self> {
		DIRS_GLOBAL.get_or_try_init(async || {
			Self::initialize()
		}).await
	}

	fn initialize() -> LauncherResult<Self> {
		let base_dir = env_path("LAUNCHER_DIR")
			.or_else(|| Some(dirs::data_dir()?.join(constants::NAME)))
			.ok_or(DirectoryError::BaseDir)?;

		Ok(Self {
			base_dir
		})
	}

	/// Get the base directory for the launcher.
	#[must_use]
	pub const fn base_dir(&self) -> &PathBuf {
		&self.base_dir
	}

	/// Get the launcher's logs directory.
	#[must_use]
	pub fn launcher_logs_dir(&self) -> PathBuf {
		self.base_dir().join("logs")
	}

	#[must_use]
	pub fn db_file(&self) -> PathBuf {
		self.base_dir().join("user_data.db")
	}


	/// Get the `config_dir/metadata` folder within the core config directory.
	#[inline]
	#[must_use]
	pub fn metadata_dir(&self) -> PathBuf {
		self.base_dir().join("metadata")
	}

	/// Get the `config_dir/metadata/libraries` folder for Minecraft libraries.
	#[must_use]
	pub fn libraries_dir(&self) -> PathBuf {
		self.metadata_dir().join("libraries")
	}

	/// Get the `config_dir/metadata/natives` folder for Minecraft natives.
	#[must_use]
	pub fn natives_dir(&self) -> PathBuf {
		self.metadata_dir().join("natives")
	}

	/// Get the `config_dir/metadata/java` directory.
	#[must_use]
	pub fn java_dir(&self) -> PathBuf {
		self.metadata_dir().join("java")
	}

	/// Get the `config_dir/metadata/caches` directory.
	#[must_use]
	pub fn caches_dir(&self) -> PathBuf {
		self.metadata_dir().join("caches")
	}
}

#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
	#[error("Failed to get the base directory for the launcher.")]
	BaseDir,
}

pub fn env_path(name: &str) -> Option<PathBuf> {
	std::env::var_os(name).map(PathBuf::from)
}