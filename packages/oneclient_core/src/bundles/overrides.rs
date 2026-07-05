use std::path::{Path, PathBuf};

use oneclient_db::models::ClusterRow;

use crate::LauncherResult;

pub async fn extract_bundle_overrides(
    archive_path: &Path,
    cluster: &ClusterRow,
) -> LauncherResult<()> {
    extract_overrides(archive_path, &cluster_root(cluster)?, false).await
}

pub async fn extract_bundle_overrides_no_overwrite(
    archive_path: &Path,
    cluster: &ClusterRow,
) -> LauncherResult<()> {
    extract_overrides(archive_path, &cluster_root(cluster)?, true).await
}

fn cluster_root(cluster: &ClusterRow) -> LauncherResult<PathBuf> {
    Ok(crate::paths::clusters_dir()?.join(&cluster.folder_name))
}

async fn extract_overrides(
    archive_path: &Path,
    dest_path: &Path,
    skip_existing: bool,
) -> LauncherResult<()> {
    if skip_existing {
        extract_overrides_no_overwrite(archive_path, dest_path).await?;
        return Ok(());
    }

    polyio::extract_zip_filtered(
        archive_path,
        dest_path,
        Some(|entry: &async_zip::StoredZipEntry| {
            entry
                .filename()
                .as_str()
                .is_ok_and(|name| name.starts_with("overrides/"))
        }),
        Some(|name: &str| name.trim_start_matches("overrides/").to_string()),
    )
    .await?;

    Ok(())
}

async fn extract_overrides_no_overwrite(
    archive_path: &Path,
    dest_path: &Path,
) -> LauncherResult<()> {
    polyio::extract_zip_filtered(
        archive_path,
        dest_path,
        Some(|entry: &async_zip::StoredZipEntry| {
            if let Ok(name) = entry.filename().as_str() {
                if !name.starts_with("overrides/") {
                    return false;
                }
                
                let stripped = name.trim_start_matches("overrides/");
                if stripped.is_empty() {
                    return false;
                }

                let target_path = dest_path.join(polyio::sanitize_path(stripped));
                if target_path.exists() {
                    return false;
                }

                if let Ok(is_dir) = entry.dir() {
                    return !is_dir;
                }
            }
            false
        }),
        Some(|old_name: &str| {
            old_name.trim_start_matches("overrides/").to_string()
        }),
    )
    .await
    .map_err(Into::into) 
}