use std::path::{Path, PathBuf};

use crate::error::{ContentError, ContentResult};
use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_common::paths;

pub fn cache_file_path(
    content_type: ContentType,
    provider: ProviderId,
    project_id: &str,
    version_id: &str,
    file_name: &str,
) -> ContentResult<PathBuf> {
    let name = safe_file_name(file_name)?;
    Ok(paths::package_version_dir(content_type, provider, project_id, version_id)?.join(name))
}

fn safe_file_name(file_name: &str) -> ContentResult<std::ffi::OsString> {
    let last = file_name.rsplit(['/', '\\']).next().unwrap_or_default();
    let unusable = || ContentError::InvalidData {
        reason: format!("'{file_name}' is not a usable file name"),
    };
    if matches!(last, "" | "." | "..") {
        return Err(unusable());
    }
    let cleaned = |name: &str| {
        polyio::sanitize_path(name)
            .file_name()
            .map(std::ffi::OsStr::to_os_string)
            .filter(|name| name != "." && name != "..")
    };
    cleaned(last)
        .or_else(|| cleaned(&format!("_{last}")))
        .ok_or_else(unusable)
}

pub fn artifact_absolute_path(stored_path: &str) -> ContentResult<PathBuf> {
    let root = paths::data_dir()?;
    Ok(root.join(stored_path))
}

pub fn relative_cache_path(abs: &Path) -> ContentResult<String> {
    let root = paths::data_dir()?;

    let rel = abs
        .strip_prefix(root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| abs.to_path_buf());

    Ok(rel.to_string_lossy().replace('\\', "/"))
}
