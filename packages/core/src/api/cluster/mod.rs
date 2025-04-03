use std::collections::HashSet;
use std::path::PathBuf;

use onelauncher_entity::clusters;
use onelauncher_entity::icon::Icon;
use onelauncher_entity::loader::GameLoader;
use serde::Serialize;

use crate::error::{DaoError, LauncherResult};
use crate::send_error;
use crate::store::Dirs;
use crate::utils::io;

pub mod dao;

#[must_use]
pub fn sanitize_name(name: &str) -> String {
	let mut name = name.to_string();
	name.retain(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')'));
	name
}

#[tracing::instrument]
pub async fn create_cluster(
	name: &str,
	mc_version: &str,
	mc_loader: GameLoader,
	mc_loader_version: Option<&str>,
	icon_url: Option<Icon>,
) -> LauncherResult<clusters::Model> {
	let name = sanitize_name(name);
	let cluster_dir = Dirs::get().await?.clusters_dir();

	// Get the directory for the cluster
	let mut path = cluster_dir.join(&name);

	// Folder name conflict resolution
	if path.exists() {
		let mut which = 1;
		loop {
			let new_name = format!("{name} ({which})");
			path = cluster_dir.join(&new_name);
			if !path.exists() {
				break;
			}
			which += 1;
		}

		tracing::warn!(
			"collision while creating new cluster: {}, renaming to {}",
			cluster_dir.display(),
			path.display()
		);
	}

	let result = async {
		io::create_dir_all(&path).await?;

		tracing::info!("creating cluster at path {}", path.display());

		// Finally add the cluster to the database
		dao::insert_cluster(
			name.as_str(),
			path.to_string_lossy().into_owned().as_str(),
			mc_version,
			mc_loader,
			mc_loader_version,
			icon_url,
		)
		.await
	}
	.await;

	match result {
		Ok(result) => Ok(result),
		Err(err) => {
			tracing::error!("failed to create cluster: {}", err);
			let _ = io::remove_dir_all(&path).await;
			Err(err)
		}
	}
}

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
pub async fn sync_clusters() -> LauncherResult<Vec<i32>> {
	let clusters = dao::get_all_clusters().await?;

	let mut missing_ids = Vec::new();
	let mut checked_paths = HashSet::new();

	for cluster in clusters {
		if sync_cluster(&cluster)
			.await
			.map(SyncAction::is_missing)
			.unwrap_or(false)
		{
			missing_ids.push(cluster.id);

			let path = cluster.path.clone();
			send_error!(
				"cluster {} is missing from the filesystem",
				path.clone()
			);
		}

		checked_paths.insert(PathBuf::from(cluster.path));
	}

	let cluster_dir = Dirs::get().await?.clusters_dir();
	let mut stream = io::read_dir(cluster_dir).await?;
	while let Ok(Some(entry)) = stream.next_entry().await {
		let path = entry.path();
		if !path.is_dir() || checked_paths.contains(&path) {
			// Skip if it's not a directory or if we've already checked it
			continue;
		}

		// We have a directory that is not in the database
		if let Err(err) = sync_from_fs_to_db(&path).await {
			tracing::error!("failed to sync cluster from fs to db: {}", err);
		};
	}

	Ok(missing_ids)
}

/// Syncs the cluster with the database.
/// Checks if the cluster in the database exists and if the directory exists.
#[tracing::instrument]
pub async fn sync_cluster(cluster: &clusters::Model) -> LauncherResult<SyncAction> {
	let path = PathBuf::from(cluster.path.clone());
	if !path.exists() {
		return Ok(SyncAction::MissingFs);
	}

	// TODO: sync packages

	Ok(SyncAction::NoAction)
}

/// Syncs the cluster with the database.
/// Checks if the cluster in the database exists and if the directory exists.
#[tracing::instrument]
pub async fn sync_cluster_by_id(id: i32) -> LauncherResult<SyncAction> {
	let cluster = dao::get_cluster_by_id(id)
		.await?
		.ok_or(DaoError::NotFound)?;

	sync_cluster(&cluster).await
}

/// Syncs the cluster with the database.
/// Checks if the cluster in the database exists and if the directory exists.
#[tracing::instrument]
pub async fn sync_cluster_by_path(path: &PathBuf) -> LauncherResult<SyncAction> {
	let Some(cluster) = dao::get_cluster_by_path(path).await? else {
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
	).into())
}
