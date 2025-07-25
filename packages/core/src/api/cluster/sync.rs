use std::collections::HashSet;
use std::path::PathBuf;

use onelauncher_entity::clusters;
use serde::Serialize;

use crate::error::{DaoError, LauncherResult};
use crate::send_error;
use crate::store::Dirs;
use crate::utils::io;

use super::dao::{self, ClusterId};

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum SyncAction {
	Created,
	Updated,
	MissingDb,
	MissingFs,
	NoAction,
}

impl SyncAction {
	#[must_use]
	pub const fn is_missing(self) -> bool {
		matches!(self, Self::MissingDb | Self::MissingFs)
	}
}

/// Syncs all clusters in the database. Returns a list of cluster IDs that are missing.
#[tracing::instrument]
pub async fn sync_clusters() -> LauncherResult<Vec<ClusterId>> {
	let clusters = dao::get_all_clusters().await?;

	let mut missing_ids = Vec::new();
	let mut checked_folders = HashSet::new();
	let dirs = Dirs::get().await?;

	for cluster in clusters {
		if sync_cluster(&cluster)
			.await
			.map(SyncAction::is_missing)
			.unwrap_or(false)
		{
			missing_ids.push(cluster.id);

			let path = cluster.folder_name.clone();
			send_error!("cluster {} is missing from the filesystem", path.clone());
		}

		checked_folders.insert(dirs.clusters_dir().join(cluster.folder_name.clone()));
	}

	// Check for clusters in the filesystem that are not in the database
	// It iterates over the directories in the cluster directory
	// and checks if they are in the database
	let cluster_dir = Dirs::get_clusters_dir().await?;
	if cluster_dir.exists() {
		let mut stream = io::read_dir(cluster_dir).await?;
		while let Ok(Some(entry)) = stream.next_entry().await {
			let path = entry.path();
			if !path.is_dir() || checked_folders.contains(&path) {
				// Skip if it's not a directory or if we've already checked it
				continue;
			}

			// We have a directory that is not in the database
			if let Err(err) = sync_from_fs_to_db(&path).await {
				tracing::error!("failed to sync cluster from fs to db: {}", err);
			}
		}
	} else {
		io::create_dir_all(cluster_dir).await?;
	}

	Ok(missing_ids)
}

/// Syncs the cluster with the database.
/// Checks if the cluster in the database exists and if the directory exists.
#[tracing::instrument]
pub async fn sync_cluster(cluster: &clusters::Model) -> LauncherResult<SyncAction> {
	let path = Dirs::get_clusters_dir()
		.await?
		.join(cluster.folder_name.clone());
	if !path.exists() {
		return Ok(SyncAction::MissingFs);
	}

	// TODO: sync packages

	Ok(SyncAction::NoAction)
}

/// Syncs the cluster with the database.
/// Checks if the cluster in the database exists and if the directory exists.
#[tracing::instrument]
pub async fn sync_cluster_by_id(id: ClusterId) -> LauncherResult<SyncAction> {
	let cluster = dao::get_cluster_by_id(id)
		.await?
		.ok_or(DaoError::NotFound)?;

	sync_cluster(&cluster).await
}

/// Syncs the cluster with the database.
/// Checks if the cluster in the database exists and if the directory exists.
#[tracing::instrument]
pub async fn sync_cluster_by_path(path: &PathBuf) -> LauncherResult<SyncAction> {
	let Some(cluster) = dao::get_cluster_by_folder_name(path).await? else {
		tracing::warn!("cluster with path {path:?} not found in database");
		return Ok(SyncAction::MissingDb);
	};

	sync_cluster(&cluster).await
}

#[tracing::instrument]
async fn sync_from_fs_to_db(path: &PathBuf) -> LauncherResult<clusters::Model> {
	// TODO: Create database entry from what can be inferred from the directory
	Err(anyhow::anyhow!(
		"TODO: failed to sync cluster from fs to db: {}",
		path.display()
	)
	.into())
}
