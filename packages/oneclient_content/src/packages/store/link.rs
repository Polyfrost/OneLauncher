use std::path::Path;

use oneclient_db::models::{ArtifactRow, ClusterRow};

use super::paths::artifact_absolute_path;
use polyio::sha1_file;
use oneclient_common::domain::ContentType;
use crate::packages::error::PackageError;
use oneclient_common::paths;
use crate::error::{ContentError, ContentResult};

#[tracing::instrument(level = "debug", skip(artifact, cluster))]
pub async fn link_artifact_to_cluster(
	artifact: &ArtifactRow,
	cluster: &ClusterRow,
	cluster_file_name: Option<&str>,
) -> ContentResult<()> {
	let src = artifact_absolute_path(&artifact.path)?;
	if !src.exists() {
		return Err(PackageError::ArtifactMissing(src.display().to_string()).into());
	}

	let content_type = ContentType::from_repr(artifact.content_type as u8)
		.ok_or_else(|| ContentError::InvalidData {
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

#[tracing::instrument(level = "debug")]
pub async fn link_or_copy(src: &Path, dest: &Path) -> ContentResult<()> {
	if dest.exists() {
		polyio::remove_file(dest).await?;
	}

	if polyio::symlink_file(src, dest).await.is_ok() {
		return Ok(());
	}

	polyio::copy(src, dest).await?;
	Ok(())
}

/// Whether the cluster folder is currently holding `file_name`, following the
/// same symlink-tolerant rule as [`unlink_cluster_file`].
pub async fn cluster_file_present(
	cluster: &ClusterRow,
	content_type: ContentType,
	file_name: &str,
) -> bool {
	let Ok(clusters) = paths::clusters_dir() else {
		return false;
	};

	let path = clusters
		.join(&cluster.folder_name)
		.join(content_type.folder_name())
		.join(file_name);

	polyio::symlink_metadata(&path).await.is_ok()
}

#[tracing::instrument(level = "debug", skip(cluster))]
pub async fn unlink_cluster_file(
	cluster: &ClusterRow,
	content_type: ContentType,
	file_name: &str,
) -> ContentResult<()> {
	let dir = paths::clusters_dir()?
		.join(&cluster.folder_name)
		.join(content_type.folder_name());

	// Both spellings: the database always stores the bare name, but the folder
	// can still be holding a `<name>.disabled` left by an older launcher or by
	// the user renaming it by hand. Removing a package has to take both.
	remove_entry(&dir.join(file_name)).await?;
	remove_entry(&dir.join(format!("{file_name}.disabled"))).await?;
	Ok(())
}

/// Deletes `path` if anything is there, symlink or not.
///
/// `Path::exists` resolves symlinks, so a link whose artifact was evicted from
/// the cache reads as absent and the dead link survives every unlink; the
/// package then keeps showing up in the folder it was removed from.
async fn remove_entry(path: &Path) -> ContentResult<()> {
	if polyio::symlink_metadata(path).await.is_err() {
		return Ok(());
	}

	polyio::remove_file(path).await?;
	Ok(())
}
