use std::path::{Path, PathBuf};

use crate::LauncherResult;
use crate::packages::domain::{ContentType, ProviderId};
use crate::paths;

pub fn cache_file_path(
    content_type: ContentType,
    provider: ProviderId,
    project_id: &str,
    version_id: &str,
    file_name: &str,
) -> LauncherResult<PathBuf> {
    Ok(paths::package_version_dir(content_type, provider, project_id, version_id)?.join(file_name))
}

pub fn artifact_absolute_path(stored_path: &str) -> LauncherResult<PathBuf> {
    let root = paths::launcher_dir()?;
    Ok(root.join(stored_path))
}

pub fn relative_cache_path(abs: &Path) -> LauncherResult<String> {
    let root = paths::launcher_dir()?;

    let rel = abs
        .strip_prefix(root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| abs.to_path_buf());

    Ok(rel.to_string_lossy().replace('\\', "/"))
}
