#![allow(clippy::significant_drop_tightening)]

use crate::data::{Loader, ManagedPackage, ManagedVersion, PackageType};
use crate::prelude::PackagePath;
use crate::processor::Cluster;
use crate::proxy::send::send_internet;
use crate::store::{ClusterPath, ManagedVersionFile, Package, PackageMetadata};
use crate::utils::http;
use crate::{Result, State};
use onelauncher_utils::io;
// TODO: Implement proper error handling

/// Find a managed version using filters
/// - Game Version (Default: Cluster's MC Version)
/// - Loader (Default: Cluster's chosen Loader)
/// - Package Version (Default: Latest version available)
#[tracing::instrument]
pub async fn find_managed_version(
	package: &ManagedPackage,
	game_version: Option<String>,
	loader: Option<Loader>,
	package_version: Option<String>,
) -> Result<ManagedVersion> {
	let provider = package.provider.clone();

	let versions = provider
		.get_all_versions(
			&package.id,
			game_version.as_ref().map(|v| vec![v.to_owned()]).clone(),
			loader.map(|l| vec![l]).clone(),
		)
		.await?;

	Ok(versions
		.iter()
		.find(|v| {
			let check_game_version = game_version
				.as_ref()
				.is_some_and(|gv| v.game_versions.iter().any(|gv2| *gv2 == *gv));

			let check_loader = loader
				.as_ref()
				.is_some_and(|loader| v.loaders.iter().any(|l| *l == *loader));

			let check_package_version = package_version
				.as_ref()
				.map_or(true, |pv| pv == &v.version_id);

			check_game_version && check_loader && check_package_version
		})
		.ok_or_else(|| anyhow::anyhow!("no matching version found"))
		.cloned()?)
}

/// Download a package to a cluster. Supports filtering by:
/// - Game Version (Default: Cluster's MC Version)
/// - Loader (Default: Cluster's chosen Loader)
/// - Package Version (Default: Latest version available)
#[tracing::instrument(skip(package, cluster))]
pub async fn download_package(
	package: &ManagedPackage,
	cluster: &mut Cluster,
	game_version: Option<String>,
	loader: Option<Loader>,
	package_version: Option<String>,
) -> Result<(PackagePath, Package)> {
	tracing::info!(
		"preparing package '{}' for cluster '{}'",
		package.title,
		cluster.meta.name
	);
	send_internet(crate::proxy::InternetPayload::InstallPackage {
		id: package.id.clone(),
	})
	.await?;

	let loader = loader.unwrap_or(cluster.meta.loader);
	let game_version = game_version.unwrap_or_else(|| cluster.meta.mc_version.clone());

	let managed_version =
		find_managed_version(package, Some(game_version), Some(loader), package_version).await?;

	let file = managed_version
		.get_primary_file()
		.ok_or_else(|| anyhow::anyhow!("no primary file found"))?;
	tracing::info!(
		"downloading file '{}' version '{}'",
		file.file_name,
		managed_version.name
	);

	let package_path = download_file(file, &package.package_type, cluster).await?;
	let sha512 = file
		.hashes
		.get("sha512")
		.unwrap_or(&"unknown".to_string())
		.to_owned();

	let package = Package {
		file_name: file.file_name.clone(),
		sha512,
		meta: PackageMetadata::from_managed_package(package.clone(), managed_version),
		disabled: false,
	};

	Ok((package_path, package))
}

/// Download a file to a cluster from a managed version file.
#[tracing::instrument(skip(file, cluster))]
async fn download_file(
	file: &ManagedVersionFile,
	package_type: &PackageType,
	cluster: &Cluster,
) -> Result<PackagePath> {
	// TODO: Implement hash checking
	let path = PackagePath::new(
		&cluster
			.get_full_path()
			.await?
			.join(package_type.get_folder())
			.join(&file.file_name),
	);

	let state = State::get().await?;
	let bytes = http::fetch(
		&file.url,
		file.hashes.get("sha1").map(String::as_str),
		&state.fetch_semaphore,
	)
	.await?;
	if let Err(err) = http::write(&path.0, &bytes, &state.io_semaphore).await {
		tracing::error!("failed to write file to cluster: {err}");
		if path.0.exists() {
			let _ = io::remove_file(&path.0).await;
		}

		return Err(err);
	};

	drop(state);

	Ok(path)
}

/// Add a package to a cluster.
#[tracing::instrument]
pub async fn add_package(
	cluster_path: &ClusterPath,
	package_path: PackagePath,
	package: Package,
	package_type: Option<PackageType>,
) -> Result<()> {
	let state = State::get().await?;
	let mut manager = state.packages.write().await;
	let manager = manager
		.get_mut(cluster_path)
		.ok_or_else(|| anyhow::anyhow!("cluster not found in packages map"))?;

	manager
		.add_package(package_path, package, package_type)
		.await?;

	Ok(())
}

/// Remove a package from a cluster.
#[tracing::instrument]
pub async fn remove_package(
	cluster_path: &ClusterPath,
	package_path: &PackagePath,
	package_type: PackageType,
) -> Result<()> {
	let state = State::get().await?;
	let mut manager = state.packages.write().await;
	let manager = manager
		.get_mut(cluster_path)
		.ok_or_else(|| anyhow::anyhow!("cluster not found in packages map"))?;

	manager.remove_package(package_path, package_type).await?;

	Ok(())
}

/// Get a package from a cluster.
#[tracing::instrument]
pub async fn get_package(
	cluster_path: &ClusterPath,
	package_path: &PackagePath,
	package_type: PackageType,
) -> Result<Package> {
	let state = State::get().await?;
	let manager = state.packages.read().await;
	let manager = manager
		.get(cluster_path)
		.ok_or_else(|| anyhow::anyhow!("cluster not found in packages map"))?;

	Ok(manager
		.get(package_type)
		.packages
		.get(package_path)
		.cloned()
		.ok_or_else(|| anyhow::anyhow!("package not found"))?)
}

/// Get packages from a cluster.
#[tracing::instrument]
pub async fn get_packages(
	cluster_path: &ClusterPath,
	package_type: PackageType,
) -> Result<Vec<Package>> {
	let state = State::get().await?;
	let manager = state.packages.read().await;
	let manager = manager
		.get(cluster_path)
		.ok_or_else(|| anyhow::anyhow!("cluster not found in packages map"))?;

	Ok(manager
		.get(package_type)
		.packages
		.values()
		.cloned()
		.collect())
}

/// Sync packages from a cluster.
#[tracing::instrument]
pub async fn sync_packages(cluster_path: &ClusterPath) -> Result<()> {
	let state = State::get().await?;
	let mut manager = state.packages.write().await;
	let manager = manager
		.get_mut(cluster_path)
		.ok_or_else(|| anyhow::anyhow!("cluster not found in packages map"))?;

	manager.sync_packages(&state.directories).await;

	Ok(())
}

/// Sync packages from a cluster.
#[tracing::instrument]
pub async fn sync_packages_by_type(
	cluster_path: &ClusterPath,
	package_type: PackageType,
) -> Result<()> {
	let state = State::get().await?;
	let mut manager = state.packages.write().await;
	let manager = manager
		.get_mut(cluster_path)
		.ok_or_else(|| anyhow::anyhow!("cluster not found in packages map"))?;

	manager
		.sync_packages_by_type(&state.directories, package_type)
		.await?;

	Ok(())
}
