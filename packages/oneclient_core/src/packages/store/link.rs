use std::path::Path;

use oneclient_db::models::{ArtifactRow, ClusterRow};

use super::paths::artifact_absolute_path;
use crate::crypto::sha1_file;
use crate::packages::domain::ContentType;
use crate::packages::error::PackageError;
use crate::paths;
use crate::{LauncherError, LauncherResult};

pub async fn link_artifact_to_cluster(
	artifact: &ArtifactRow,
	cluster: &ClusterRow,
	cluster_file_name: Option<&str>,
) -> LauncherResult<()> {
	let src = artifact_absolute_path(&artifact.path)?;
	if !src.exists() {
		return Err(PackageError::ArtifactMissing(src.display().to_string()).into());
	}

	let content_type = ContentType::from_repr(artifact.content_type as u8)
		.ok_or_else(|| LauncherError::InvalidSettingsProfile {
			reason: format!("unknown content type {}", artifact.content_type),
		})?;

	let cluster_root = paths::clusters_dir()?.join(&cluster.folder_name);
	let dest_dir = cluster_root.join(content_type.folder_name());
	polyio::create_dir_all(&dest_dir).await?;

	let file_name = cluster_file_name.unwrap_or(&artifact.file_name);
	let dest = dest_dir.join(file_name);

	link_or_copy(&src, &dest).await?;

	if dest.exists() {
		let actual = sha1_file(&dest).await?;
		if actual != artifact.hash {
			return Err(PackageError::HashMismatch {
				expected: artifact.hash.clone(),
				actual,
			}
			.into());
		}
	}

	Ok(())
}

async fn link_or_copy(src: &Path, dest: &Path) -> LauncherResult<()> {
	if dest.exists() {
		tokio::fs::remove_file(dest).await?;
	}

	#[cfg(unix)]
	{
		if tokio::fs::symlink(src, dest).await.is_ok() {
			return Ok(());
		}
	}

	if tokio::fs::hard_link(src, dest).await.is_ok() {
		return Ok(());
	}

	tokio::fs::copy(src, dest).await?;
	Ok(())
}

pub async fn unlink_cluster_file(
	cluster: &ClusterRow,
	content_type: ContentType,
	file_name: &str,
) -> LauncherResult<()> {
	let path = paths::clusters_dir()?
		.join(&cluster.folder_name)
		.join(content_type.folder_name())
		.join(file_name);

	if path.exists() {
		tokio::fs::remove_file(&path).await?;
	}
	Ok(())
}
