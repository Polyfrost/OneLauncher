use crate::{data::{Loader, ManagedPackage, PackageType}, prelude::PackagePath, processor::Cluster, store::{ManagedVersionFile, Package}, utils::{http, io}, Result, State};

// TODO: Implement proper error handling

/// Download a package to a cluster. Supports filtering by:
/// - Game Version (Default: Cluster's MC Version)
/// - Loader (Default: Cluster's chosen Loader)
/// - Package Version (Default: Latest version available)
#[tracing::instrument]
pub async fn download_package(
	package: ManagedPackage,
	cluster: &Cluster,
	game_version: Option<String>,
	loader: Option<Loader>,
	package_version: Option<String>
) -> Result<()> {
	tracing::info!("downloading package '{}' to cluster '{}'", package.title, cluster.meta.name);

	let loader = loader.unwrap_or(cluster.meta.loader);
	let game_version = game_version.unwrap_or(cluster.meta.mc_version.clone());
	let provider = package.provider;

	let versions = provider.get_versions(&package.id).await?;

	let managed_version = versions.iter().find(|v| {
		let check_game_version = v.game_versions.iter().any(|gv| gv == &game_version);
		let check_loader = v.loaders.iter().any(|l| l == &loader);
		let check_package_version = package_version.as_ref().map_or(true, |pv| pv == &v.version_id);

		check_game_version && check_loader && check_package_version
	}).ok_or(anyhow::anyhow!("no matching version found"))?;

	let file = managed_version.get_primary_file().ok_or(anyhow::anyhow!("no primary file found"))?;
	tracing::info!("downloading file '{}' from version '{}'", file.file_name, managed_version.name);

	Ok(())
}

/// Download a file to a cluster from a managed version file.
#[tracing::instrument]
async fn download_file(file: &ManagedVersionFile, package_type: &PackageType, cluster: &Cluster) -> Result<()> {
	let path = PackagePath::new(&cluster
		.get_full_path()
		.await?
		.join(package_type.get_folder())
		.join(&file.file_name));

	let state = State::get().await?;
	let bytes = http::fetch(&file.url, file.hashes.get("sha1").map(|s| s.as_str()), &state.fetch_semaphore).await?;
	if let Err(err) = http::write(&path.0, &bytes, &state.io_semaphore).await {
		tracing::error!("failed to write file to cluster: {err}");
		if path.0.exists() {
			let _ = io::remove_file(&path.0).await;
		}

		return Err(err);
	};

	Ok(())
}

#[tracing::instrument]
pub async fn add_package_to_cluster(cluster: &mut Cluster, package: Package) -> Result<()> {
	let path = cluster.get_full_path().await?.join("mods");
	let path = PackagePath::new(&path);

	cluster.packages.insert(path, package);

	Ok(())
}
