use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chrono::Utc;
use futures::future::join_all;
use onelauncher_entity::package::{PackageType, Provider};
use onelauncher_entity::{clusters, packages};
use sea_orm::{ActiveValue, Iterable};
use serde::Serialize;

use crate::api::packages::provider::ProviderExt;
use crate::error::{DaoError, LauncherResult};
use crate::send_error;
use crate::store::Dirs;
use crate::utils::crypto::HashAlgorithm;
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

	tracing::info!("sync_cluster: scanning path={}", path.display());

	let mut missing_hashes = HashMap::new();
	// Directory-based packages (resource packs, shader packs, data packs can be folders).
	// These skip provider lookups and are always registered as local packages.
	let mut missing_dir_hashes: Vec<(String, PathBuf, PackageType)> = Vec::new();
	let mut found_hashes = HashSet::new();

	for package_type in PackageType::iter() {
		let dir = path.join(package_type.folder_name());
		if !dir.exists() {
			continue;
		}

		let mut stream = io::read_dir(&dir).await?;
		while let Ok(Some(entry)) = stream.next_entry().await {
			let file_path = entry.path();
			let is_dir = file_path.is_dir();
			if !file_path.is_file() && !is_dir {
				continue;
			}
			// Skip hidden files/folders
			if file_path
				.file_name()
				.is_some_and(|s| s.to_string_lossy().starts_with('.'))
			{
				continue;
			}

			if is_dir {
				// Mods are never folders — skip directories in the mods directory.
				if package_type == PackageType::Mod {
					continue;
				}
				// Directories get a deterministic hash from their name so they have a stable DB identity.
				let folder_name = file_path
					.file_name()
					.unwrap_or_default()
					.to_string_lossy()
					.to_string();
				let hash = HashAlgorithm::Sha1
					.hash(format!("dir:{folder_name}").as_bytes())
					.await?;

				if let Some(package) =
					crate::api::packages::dao::get_package_by_hash(hash.clone()).await?
				{
					if !crate::api::packages::dao::is_package_linked_to_cluster(&package, cluster)
						.await?
					{
						crate::api::packages::dao::link_package_to_cluster(&package, cluster)
							.await?;
					}
					found_hashes.insert(hash);
				} else {
					missing_dir_hashes.push((hash, file_path, package_type.clone()));
				}
				continue;
			}

			let hash = HashAlgorithm::Sha1.hash_file(&file_path).await?;

			if let Some(package) =
				crate::api::packages::dao::get_package_by_hash(hash.clone()).await?
			{
				if !crate::api::packages::dao::is_package_linked_to_cluster(&package, cluster)
					.await?
				{
					crate::api::packages::dao::link_package_to_cluster(&package, cluster).await?;
				}
				found_hashes.insert(hash);
			} else {
				missing_hashes.insert(hash, (file_path, package_type.clone()));
			}
		}
	}

	// Track dir packages as on-disk before the vec is consumed.
	found_hashes.extend(missing_dir_hashes.iter().map(|(h, _, _)| h.clone()));

	// Register directory-based packages as local (no provider lookup possible for folders).
	for (hash, dir_path, package_type) in missing_dir_hashes {
		let file_name = dir_path
			.file_name()
			.unwrap_or_default()
			.to_string_lossy()
			.to_string();

		let model = packages::ActiveModel {
			hash: ActiveValue::Set(hash.clone()),
			file_name: ActiveValue::Set(file_name.clone()),
			version_id: ActiveValue::Set(hash.clone()),
			published_at: ActiveValue::Set(Utc::now()),
			display_name: ActiveValue::Set(file_name.clone()),
			display_version: ActiveValue::Set("Unknown".to_string()),
			package_type: ActiveValue::Set(package_type),
			provider: ActiveValue::Set(Provider::Local),
			package_id: ActiveValue::Set(hash.clone()),
			mc_versions: ActiveValue::Set(vec![].into()),
			mc_loader: ActiveValue::Set(vec![].into()),
			icon: ActiveValue::Set(None),
		};

		let package = match crate::api::packages::dao::insert_package(model).await {
			Ok(package) => package,
			Err(err) => {
				tracing::error!("failed to insert local directory package {}: {}", hash, err);
				continue;
			}
		};
		if let Err(err) =
			crate::api::packages::dao::link_package_to_cluster(&package, cluster).await
		{
			tracing::error!(
				"failed to link local directory package {} to cluster: {}",
				hash,
				err
			);
		}
	}

	if !missing_hashes.is_empty() {
		let hashes: Vec<String> = missing_hashes.keys().cloned().collect();

		let futures = Provider::get_providers().iter().map(|&provider| {
			let hashes = hashes.clone();
			async move {
				match provider.get_versions_by_hashes(&hashes).await {
					Ok(versions) => Some((provider, versions)),
					Err(err) => {
						tracing::error!(
							"failed to get versions from provider {}: {}",
							provider,
							err
						);
						None
					}
				}
			}
		});

		let results = join_all(futures).await;

		let mut found_versions = HashMap::new();
		for result in results.into_iter().flatten() {
			let (provider, versions) = result;
			for (hash, version) in versions {
				found_versions.entry(hash).or_insert((version, provider));
			}
		}

		for (hash, (file_path, package_type)) in &missing_hashes {
			if let Some((version, provider)) = found_versions.get(hash) {
				// Find the file in version.files
				let file = version
					.files
					.iter()
					.find(|f| f.sha1 == *hash)
					.ok_or_else(|| anyhow::anyhow!("File hash mismatch"))?;

				let model = packages::ActiveModel {
					hash: ActiveValue::Set(hash.clone()),
					file_name: ActiveValue::Set(file.file_name.clone()),
					version_id: ActiveValue::Set(version.version_id.clone()),
					published_at: ActiveValue::Set(version.published),
					display_name: ActiveValue::Set(version.display_name.clone()),
					display_version: ActiveValue::Set(version.display_version.clone()),
					package_type: ActiveValue::Set(package_type.clone()),
					provider: ActiveValue::Set(*provider),
					package_id: ActiveValue::Set(version.project_id.clone()),
					mc_versions: ActiveValue::Set(version.mc_versions.clone().into()),
					mc_loader: ActiveValue::Set(version.loaders.clone().into()),
					icon: ActiveValue::Set(None), // TODO: Fetch icon
				};

				let package = match crate::api::packages::dao::insert_package(model).await {
					Ok(package) => package,
					Err(err) => {
						tracing::error!("failed to insert package {}: {}", hash, err);
						continue;
					}
				};
				if let Err(err) =
					crate::api::packages::dao::link_package_to_cluster(&package, cluster).await
				{
					tracing::error!("failed to link package {} to cluster: {}", hash, err);
				}
			} else {
				// Save as external package
				let file_name = file_path
					.file_name()
					.unwrap_or_default()
					.to_string_lossy()
					.to_string();

				let model = packages::ActiveModel {
					hash: ActiveValue::Set(hash.clone()),
					file_name: ActiveValue::Set(file_name.clone()),
					version_id: ActiveValue::Set(hash.clone()),
					published_at: ActiveValue::Set(Utc::now()),
					display_name: ActiveValue::Set(file_name.clone()),
					display_version: ActiveValue::Set("Unknown".to_string()),
					package_type: ActiveValue::Set(package_type.clone()),
					provider: ActiveValue::Set(Provider::Local),
					package_id: ActiveValue::Set(hash.clone()),
					mc_versions: ActiveValue::Set(vec![].into()),
					mc_loader: ActiveValue::Set(vec![].into()),
					icon: ActiveValue::Set(None),
				};

				let package = match crate::api::packages::dao::insert_package(model).await {
					Ok(package) => package,
					Err(err) => {
						tracing::error!("failed to insert local package {}: {}", hash, err);
						continue;
					}
				};
				if let Err(err) =
					crate::api::packages::dao::link_package_to_cluster(&package, cluster).await
				{
					tracing::error!("failed to link local package {} to cluster: {}", hash, err);
				}
			}
		}
	}

	// Reconcile: unlink any packages that are in the DB but no longer on disk.
	// Extend found_hashes with all file packages seen on disk (whether provider-matched or local).
	found_hashes.extend(missing_hashes.keys().cloned());

	tracing::info!("sync_cluster: on-disk hashes count={}", found_hashes.len());

	let linked_packages = crate::api::packages::dao::get_linked_packages(cluster).await?;
	tracing::info!(
		"sync_cluster: db-linked packages count={}",
		linked_packages.len()
	);
	for package in linked_packages {
		if !found_hashes.contains(&package.hash) {
			tracing::info!(
				"sync_cluster: unlinking missing package file_name={} hash={}",
				package.file_name,
				package.hash
			);
			if let Err(err) =
				crate::api::packages::dao::unlink_package_from_cluster(&package.hash, cluster.id)
					.await
			{
				tracing::error!(
					"failed to unlink deleted package {} from cluster: {}",
					package.hash,
					err
				);
			}
		}
	}

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
