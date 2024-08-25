use crate::data::{Loader, ManagedPackage, ManagedVersion, PackageType};
use crate::prelude::PackagePath;
use crate::processor::Cluster;
use crate::proxy::send::send_internet;
use crate::store::{ClusterPath, ManagedVersionFile, Package, PackageMetadata, PackagesMap};
use crate::utils::{http, io};
use crate::{cluster, Result, State};
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

	let versions = provider.get_versions(&package.id).await?;

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
		.ok_or(anyhow::anyhow!("no matching version found"))
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
	let game_version = game_version.unwrap_or(cluster.meta.mc_version.clone());

	let managed_version =
		find_managed_version(package, Some(game_version), Some(loader), package_version).await?;

	let file = managed_version
		.get_primary_file()
		.ok_or(anyhow::anyhow!("no primary file found"))?;
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
		.to_owned(); // TODO: Figure out sha1

	let package = Package {
		file_name: file.file_name.clone(),
		sha512,
		meta: PackageMetadata::from_managed_package(package.clone(), managed_version),
		disabled: false,
	};

	Ok((package_path, package))
}

/// Add a package to a cluster.
#[tracing::instrument]
pub async fn add_package_to_cluster(
	package_path: PackagePath,
	package: Package,
	cluster: &Cluster,
	package_type: Option<PackageType>,
) -> Result<()> {
	let package_type = match package_type {
		Some(pt) => pt,
		None => package.get_package_type()?,
	};

	let mut packages = get_packages_by_type(&cluster.cluster_path(), package_type).await?;
	packages.insert(package_path, package);

	cluster::sync_packages(&cluster.cluster_path()).await;

	Ok(())
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
		file.hashes.get("sha1").map(|s| s.as_str()),
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

	Ok(path)
}
