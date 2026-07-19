use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::LauncherError;
use crate::LauncherResult;

const QUALIFIER: &str = "org";
const ORGANIZATION: &str = "Polyfrost";
#[cfg(not(debug_assertions))]
const APPLICATION: &str = "OneClient";
// Dev builds use a separate launcher dir (e.g. `oneclient-dev`) so a running
// dev environment never touches prod data.
#[cfg(debug_assertions)]
const APPLICATION: &str = "OneClient-dev";

static PROJECT_DIRS: OnceLock<ProjectDirs> = OnceLock::new();
static LAUNCHER_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_launcher_dir(dir: PathBuf) {
	let _ = LAUNCHER_DIR_OVERRIDE.set(dir);
}

fn project_dirs() -> LauncherResult<&'static ProjectDirs> {
	if let Some(dirs) = PROJECT_DIRS.get() {
		return Ok(dirs);
	}

	let dirs = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
		.ok_or(LauncherError::DataDirUnavailable)?;

	let _ = PROJECT_DIRS.set(dirs);
	PROJECT_DIRS.get().ok_or(LauncherError::DataDirUnavailable)
}

pub fn launcher_dir() -> LauncherResult<&'static Path> {
	if let Some(dir) = LAUNCHER_DIR_OVERRIDE.get() {
		return Ok(dir);
	}

	Ok(project_dirs()?.data_local_dir())
}

pub fn database_file() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("user_data.db"))
}

pub fn settings_file() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("settings.json"))
}

pub fn auth_file() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("auth.json"))
}

pub fn logs_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("logs"))
}

pub fn java_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("metadata").join("java"))
}

pub fn clusters_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("clusters"))
}

pub fn shared_minecraft_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join(".minecraft"))
}

pub fn packages_cache_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("metadata").join("packages"))
}

pub fn caches_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("metadata").join("caches"))
}

pub fn bundles_dir() -> LauncherResult<PathBuf> {
	Ok(caches_dir()?.join("bundles"))
}

pub fn images_cache_dir() -> LauncherResult<PathBuf> {
	Ok(caches_dir()?.join("images"))
}

pub fn profiles_cache_dir() -> LauncherResult<PathBuf> {
	Ok(caches_dir()?.join("profiles"))
}

pub fn versions_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("metadata").join("versions"))
}

pub fn libraries_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("metadata").join("libraries"))
}

pub fn natives_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("metadata").join("natives"))
}

pub fn assets_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("metadata").join("assets"))
}

pub fn assets_index_dir() -> LauncherResult<PathBuf> {
	Ok(assets_dir()?.join("indexes"))
}

pub fn assets_object_dir() -> LauncherResult<PathBuf> {
	Ok(assets_dir()?.join("objects"))
}

pub fn legacy_assets_dir() -> LauncherResult<PathBuf> {
	Ok(launcher_dir()?.join("metadata").join("resources"))
}

pub fn package_version_dir(
	content_type: crate::packages::domain::ContentType,
	provider: crate::packages::domain::ProviderId,
	project_id: &str,
	version_id: &str,
) -> LauncherResult<PathBuf> {
	Ok(packages_cache_dir()?
		.join(content_type.folder_name())
		.join(provider.dir_name())
		.join(project_id)
		.join(version_id))
}
