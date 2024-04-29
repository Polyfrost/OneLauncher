//! Utilities for creating Clusters

use crate::data::Loader;
use crate::package::from::CreatePackCluster;
use crate::proxy::send::send_cluster;
use crate::proxy::ClusterPayloadType;
pub use crate::store::{Cluster, ClusterPath, JavaOptions, PackageData, State};
use crate::utils::io::{self, canonicalize};
use crate::{cluster, package};
use interpulse::api::modded::LoaderVersion;
use std::path::PathBuf;

/// Creates a [`Cluster`] and adds it to the memory [`State`].
/// Returns a relative filepath ([`ClusterPath`]) which can be used to access the cluster.
#[tracing::instrument]
#[onelauncher_debug::debugger]
#[allow(clippy::too_many_arguments)]
pub async fn create_cluster(
	mut name: String,
	mc_version: String,
	mod_loader: Loader,
	loader_version: Option<String>,
	icon: Option<PathBuf>,
	icon_url: Option<String>,
	package_data: Option<PackageData>,
	skip: Option<bool>,
	skip_watch: Option<bool>,
) -> crate::Result<ClusterPath> {
	name = cluster::sanitize_cluster_name(&name);
	tracing::trace!("creating new cluster {}", name);
	let state = State::get().await?;
	let uuid = uuid::Uuid::new_v4();
	let mut path = state.directories.clusters_dir().await.join(&name);

	// i hate it here
	if path.exists() {
		let mut new_name;
		let mut new_path;
		let mut which = 1;
		loop {
			new_name = format!("{name} ({which})");
			new_path = state.directories.clusters_dir().await.join(&new_name);
			if !new_path.exists() {
				break;
			}
			which += 1;
		}

		tracing::debug!(
			"collision while creating new cluster: {}, renaming to {}",
			path.display(),
			new_path.display()
		);
		path = new_path;
		name = new_name;
	}

	io::create_dir_all(&path).await?;
	tracing::info!(
		"creating cluster at path {}",
		&canonicalize(&path)?.display()
	);
	let loader = if mod_loader != Loader::Vanilla {
		get_loader_version(mc_version.clone(), mod_loader, loader_version).await?
	} else {
		None
	};

	let mut cluster = Cluster::new(uuid, name, mc_version).await?;
	let result = async {
		if let Some(ref icon) = icon {
			let bytes = io::read(state.directories.caches_dir().await.join(icon)).await?;
			cluster
				.set_icon(
					&state.directories.caches_dir().await,
					&state.io_semaphore,
					bytes::Bytes::from(bytes),
					&icon.to_string_lossy(),
				)
				.await?;
		}

		cluster.meta.icon_url = icon_url;
		if let Some(loader_version) = loader {
			cluster.meta.loader = mod_loader;
			cluster.meta.loader_version = Some(loader_version);
		}

		cluster.meta.package_data = package_data;
		if let Some(package_data) = &mut cluster.meta.package_data {
			package_data.locked =
				Some(package_data.package_id.is_some() && package_data.version_id.is_some());
		}

		send_cluster(
			uuid,
			&cluster.cluster_path(),
			&cluster.meta.name,
			ClusterPayloadType::Created,
		)
		.await?;

		{
			let mut clusters = state.clusters.write().await;
			clusters
				.insert(cluster.clone(), skip_watch.unwrap_or_default())
				.await?;
		}

		if !skip.unwrap_or(false) {
			crate::game::install_minecraft(&cluster, None, false).await?;
		}

		State::sync().await?;

		Ok(cluster.cluster_path())
	}
	.await;

	match result {
		Ok(cluster) => Ok(cluster),
		Err(err) => {
			let _ = crate::api::cluster::remove(&cluster.cluster_path()).await;

			Err(err)
		}
	}
}

/// Create a [`Cluster`] from a [`CreatePackCluster`] cluster, returning a [`ClusterPath`].
pub async fn create_cluster_from_package(cluster: CreatePackCluster) -> crate::Result<ClusterPath> {
	create_cluster(
		cluster.name,
		cluster.mc_version,
		cluster.mod_loader,
		cluster.loader_version,
		cluster.icon,
		cluster.icon_url,
		cluster.package_data,
		cluster.skip,
		cluster.skip_watch,
	)
	.await
}

/// Create a duplicate [`Cluster`] from another [`ClusterPath`], returning a new [`ClusterPath`]
pub async fn create_cluster_from_duplicate(from: ClusterPath) -> crate::Result<ClusterPath> {
	let cluster = cluster::get(&from, None).await?.ok_or_else(|| {
		anyhow::anyhow!("failed to get unmanaged cluster from {}", from.to_string())
	})?;
	let cluster_path = create_cluster(
		cluster.meta.name.clone(),
		cluster.meta.mc_version.clone(),
		cluster.meta.loader,
		cluster.meta.loader_version.clone().map(|it| it.id),
		cluster.meta.icon.clone(),
		cluster.meta.icon_url.clone(),
		cluster.meta.package_data.clone(),
		Some(true),
		Some(true),
	)
	.await?;

	let state = State::get().await?;
	let copied = package::import::copy_minecraft(
		cluster_path.clone(),
		from.full_path().await?,
		&state.io_semaphore,
		None,
	)
	.await?;

	let duplicated = cluster::get(&cluster_path, None).await?.ok_or_else(|| {
		anyhow::anyhow!(
			"failed to get unmanaged cluster from {}",
			cluster_path.to_string()
		)
	})?;

	crate::game::install_minecraft(&duplicated, Some(copied), false).await?;
	{
		let state = State::get().await?;
		let mut watcher = state.watcher.write().await;
		Cluster::watch(&cluster.get_full_path().await?, &mut watcher).await?;
	}

	send_cluster(
		cluster.uuid,
		&cluster.cluster_path(),
		&cluster.meta.name,
		ClusterPayloadType::Edited,
	)
	.await?;

	State::sync().await?;
	Ok(cluster_path)
}

/// Get the latest [`LoaderVersion`] from a [`Loader`].
#[tracing::instrument]
#[onelauncher_debug::debugger]
pub(crate) async fn get_loader_version(
	mc_version: String,
	loader: Loader,
	loader_version: Option<String>,
) -> crate::Result<Option<LoaderVersion>> {
	let state = State::get().await?;
	let metadata = state.metadata.read().await;
	let version = loader_version.unwrap_or_else(|| "latest".to_string());
	let filter = |it: &LoaderVersion| match version.as_str() {
		"latest" => true,
		"stable" => it.stable,
		id => {
			it.id == *id
				|| format!("{}-{}", mc_version, id) == it.id
				|| format!("{}-{}-{}", mc_version, id, mc_version) == it.id
		}
	};

	let loader_meta = match loader {
		Loader::Forge => &metadata.forge,
		Loader::Fabric => &metadata.fabric,
		Loader::Quilt => &metadata.quilt,
		Loader::NeoForge => &metadata.neoforge,
		Loader::LegacyFabric => &metadata.legacy_fabric,
		_ => return Err(CreateClusterError::MissingManifest(loader.to_string()).into()),
	};

	let loaders = &loader_meta
		.game_versions
		.iter()
		.find(|it| {
			it.id
				.replace(interpulse::api::modded::DUMMY_REPLACE_STRING, &mc_version)
				== mc_version
		})
		.ok_or_else(|| {
			CreateClusterError::UnsupportedLoader(loader.to_string(), mc_version.clone())
		})?
		.loaders;

	let loader_version = loaders
		.iter()
		.find(|&it| filter(it))
		.cloned()
		.or(if version == "stable" {
			loaders.iter().next().cloned()
		} else {
			None
		})
		.ok_or_else(|| CreateClusterError::InvalidLoaderVersion(version, loader.to_string()))?;

	Ok(Some(loader_version))
}

#[derive(thiserror::Error, Debug)]
pub enum CreateClusterError {
	#[error("Loader {0} is unsupported on Minecraft version {1}")]
	UnsupportedLoader(String, String),
	#[error("Invalid Loader version {0} for Loader {1}")]
	InvalidLoaderVersion(String, String),
	#[error("Failed to get Loader manifest for {0}.")]
	MissingManifest(String),
	#[error("Failed to handle I/O operations: {0}")]
	IOError(#[from] std::io::Error),
}
