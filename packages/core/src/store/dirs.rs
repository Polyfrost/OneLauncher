use std::path::PathBuf;

use onelauncher_entity::package::{PackageType, Provider};
use tokio::sync::OnceCell;

use crate::{utils, LauncherResult};

use super::Core;

/// The static [`OnceCell<RwLock<Dirs>>`] for storing the global directory state.
/// Should be initialized as soon as possible (preferably before logging)
static DIRS_GLOBAL: OnceCell<Dirs> = OnceCell::const_new();

pub struct Dirs {
	base_dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
	#[error("Failed to get the base directory for the launcher.")]
	BaseDir,
}

impl Dirs {
	pub async fn get() -> LauncherResult<&'static Self> {
		DIRS_GLOBAL.get_or_try_init(async || {
			Self::initialize()
		}).await
	}

	fn initialize() -> LauncherResult<Self> {
		let base_dir = utils::io::env_path("LAUNCHER_DIR")
			.or_else(|| Some(dirs::data_dir()?.join(Core::get().launcher_name.clone())))
			.ok_or(DirectoryError::BaseDir)?;

		tracing::info!("using base directory '{}'", base_dir.display());

		Ok(Self {
			base_dir
		})
	}

	/// Get the base directory for the launcher.
	#[must_use]
	pub const fn base_dir(&self) -> &PathBuf {
		&self.base_dir
	}

}

macro_rules! dirs_impl {
	($($name:ident = |$self:ident$(, $param:ident : $type:ty )*| $path:expr),*$(,)?) => {
		paste::paste! {
			impl Dirs {
				$(
					#[must_use]
					pub fn $name(&self $(, $param: $type)*) -> PathBuf {
						let $self = self;
						$path
					}

					pub async fn [<get _ $name>]($($param: $type),*) -> LauncherResult<PathBuf> {
						Dirs::get().await.map(|$self| {
							$self.$name($($param),*)
						})
					}
				)*
			}
		}
	};
}

dirs_impl! {
	db_file = |this| this.base_dir.join("user_data.db"),
	settings_file = |this| this.base_dir.join("settings.json"),
	auth_file = |this| this.base_dir.join("auth.json"),

	launcher_logs_dir = |this| this.base_dir.join("logs"),
	clusters_dir = |this| this.base_dir.join("clusters"),
	metadata_dir = |this| this.base_dir.join("metadata"),

	versions_dir = |this| this.metadata_dir().join("versions"),
	libraries_dir = |this| this.metadata_dir().join("libraries"),
	natives_dir = |this| this.metadata_dir().join("natives"),
	java_dir = |this| this.metadata_dir().join("java"),
	caches_dir = |this| this.metadata_dir().join("caches"),
	packages_dir = |this| this.metadata_dir().join("packages"),

	assets_dir = |this| this.metadata_dir().join("assets"),
	legacy_assets_dir = |this| this.metadata_dir().join("resources"),
	assets_index_dir = |this| this.assets_dir().join("indexes"),
	assets_object_dir = |this| this.assets_dir().join("objects"),

	package_dir = |this, package_type: &PackageType, provider: &Provider, project_id: &str| {
		this.packages_dir()
			.join(package_type.folder_name())
			.join(provider.name())
			.join(project_id)
	}
}
