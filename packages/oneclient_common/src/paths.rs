use directories::{BaseDirs, ProjectDirs};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::domain::{ContentType, ProviderId};
use crate::error::{PathsError, PathsResult};

const QUALIFIER: &str = "org";
const ORGANIZATION: &str = "Polyfrost";

#[cfg(not(debug_assertions))]
const APPLICATION: &str = "OneClient";

#[cfg(debug_assertions)]
const APPLICATION: &str = "OneClient-dev";

const SETTINGS_FILE: &str = "settings.json";

static DEFAULT_DIR: OnceLock<PathBuf> = OnceLock::new();
static CONFIG_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();
static DATA_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_launcher_dir(dir: PathBuf) {
	let _ = CONFIG_DIR_OVERRIDE.set(dir.clone());
	let _ = DATA_DIR_OVERRIDE.set(dir);
}

pub fn set_data_dir(dir: PathBuf) {
	let _ = DATA_DIR_OVERRIDE.set(dir);
}

fn organization_dir() -> PathsResult<PathBuf> {
	BaseDirs::new()
		.map(|dirs| dirs.data_dir().join(ORGANIZATION))
		.ok_or(PathsError::DataDirUnavailable)
}

fn legacy_dir() -> Option<PathBuf> {
	ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
		.map(|dirs| dirs.data_local_dir().to_path_buf())
}

fn resolve_default_dir() -> PathsResult<PathBuf> {
	if let Some(legacy) = legacy_dir()
		&& legacy.join(SETTINGS_FILE).is_file()
	{
		return Ok(legacy);
	}

	Ok(organization_dir()?.join(APPLICATION))
}

fn default_dir() -> PathsResult<&'static Path> {
	if let Some(dir) = DEFAULT_DIR.get() {
		return Ok(dir);
	}

	let _ = DEFAULT_DIR.set(resolve_default_dir()?);
	DEFAULT_DIR
		.get()
		.map(PathBuf::as_path)
		.ok_or(PathsError::DataDirUnavailable)
}

pub fn config_dir() -> PathsResult<&'static Path> {
	if let Some(dir) = CONFIG_DIR_OVERRIDE.get() {
		return Ok(dir);
	}

	default_dir()
}

pub fn data_dir() -> PathsResult<&'static Path> {
	if let Some(dir) = DATA_DIR_OVERRIDE.get() {
		return Ok(dir);
	}

	config_dir()
}

#[must_use]
pub fn picker_start_dir() -> Option<PathBuf> {
	organization_dir()
		.ok()
		.filter(|dir| dir.is_dir())
		.or_else(|| data_dir().ok().map(Path::to_path_buf))
}

pub fn database_file() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("user_data.db"))
}

pub fn settings_file() -> PathsResult<PathBuf> {
	Ok(config_dir()?.join(SETTINGS_FILE))
}

pub fn damaged_settings_file() -> PathsResult<PathBuf> {
	Ok(config_dir()?.join("settings.json.corrupt"))
}

pub fn auth_file() -> PathsResult<PathBuf> {
	Ok(config_dir()?.join("auth.json"))
}

pub fn logs_dir() -> PathsResult<PathBuf> {
	Ok(config_dir()?.join("logs"))
}

pub fn java_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("metadata").join("java"))
}

pub fn clusters_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("clusters"))
}

pub fn shared_minecraft_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join(".minecraft"))
}

/// Presence marks a cluster folder as its own game dir instead of the shared `.minecraft`
pub const DEDICATED_MARKER: &str = ".dedicated_directory";

pub fn cluster_dir(folder_name: &str) -> PathsResult<PathBuf> {
	Ok(clusters_dir()?.join(folder_name))
}

pub fn cluster_uses_dedicated_dir(folder_name: &str) -> bool {
	cluster_dir(folder_name).is_ok_and(|dir| dir.join(DEDICATED_MARKER).exists())
}

/// Lives here not on `Cluster` so the content layer can resolve it from a bare
/// `ClusterRow` without duplicating the marker-file rule
pub fn cluster_game_dir(folder_name: &str) -> PathsResult<PathBuf> {
	if cluster_uses_dedicated_dir(folder_name) {
		cluster_dir(folder_name)
	} else {
		shared_minecraft_dir()
	}
}

pub fn packages_cache_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("metadata").join("packages"))
}

pub fn caches_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("metadata").join("caches"))
}

pub fn bundles_dir() -> PathsResult<PathBuf> {
	Ok(caches_dir()?.join("bundles"))
}

pub fn images_cache_dir() -> PathsResult<PathBuf> {
	Ok(caches_dir()?.join("images"))
}

pub fn profiles_cache_dir() -> PathsResult<PathBuf> {
	Ok(caches_dir()?.join("profiles"))
}

pub fn versions_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("metadata").join("versions"))
}

pub fn libraries_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("metadata").join("libraries"))
}

pub fn natives_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("metadata").join("natives"))
}

pub fn assets_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("metadata").join("assets"))
}

pub fn assets_index_dir() -> PathsResult<PathBuf> {
	Ok(assets_dir()?.join("indexes"))
}

pub fn assets_object_dir() -> PathsResult<PathBuf> {
	Ok(assets_dir()?.join("objects"))
}

pub fn legacy_assets_dir() -> PathsResult<PathBuf> {
	Ok(data_dir()?.join("metadata").join("resources"))
}

pub fn package_version_dir(
	content_type: ContentType,
	provider: ProviderId,
	project_id: &str,
	version_id: &str,
) -> PathsResult<PathBuf> {
	Ok(packages_cache_dir()?
		.join(content_type.folder_name())
		.join(provider.dir_name())
		.join(project_id)
		.join(version_id))
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::ffi::OsStr;

	#[test]
	fn the_launcher_sits_one_level_under_the_organization_folder() {
		let organization = organization_dir().expect("a home directory");
		assert_eq!(organization.file_name(), Some(OsStr::new(ORGANIZATION)));

		let app = organization.join(APPLICATION);
		let tail: Vec<&OsStr> = app.iter().rev().take(2).collect();

		assert_eq!(
			tail,
			vec![OsStr::new(APPLICATION), OsStr::new(ORGANIZATION)],
			"every Polyfrost product shares one folder and takes a single name inside it"
		);
	}

	#[cfg(windows)]
	#[test]
	fn a_windows_install_roams_with_the_user() {
		let organization = organization_dir().expect("a home directory");

		assert!(
			organization.iter().any(|part| part == OsStr::new("Roaming")),
			"settings and saves follow the user between machines; Local would strand them"
		);
	}

	#[test]
	fn a_debug_build_never_shares_a_folder_with_a_release_one() {
		assert_eq!(APPLICATION, if cfg!(debug_assertions) {
			"OneClient-dev"
		} else {
			"OneClient"
		});
	}
}
