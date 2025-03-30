//! **`OneLauncher` Cluster**
//!
//! API for creating our managed Minecraft instances, Clusters.

// TODO: (pauline) fully implement the cluster::self APIs.

use crate::proxy::send::send_cluster;

use crate::prelude::{ClusterPath, JavaVersion, PackagePath};
use crate::proxy::ClusterPayloadType;
pub use crate::store::{Cluster, JavaOptions, State};
use crate::store::{MinecraftCredentials, ProcessorChild};

use onelauncher_utils::io::{self, IOError};

use std::collections::HashMap;
use std::future::Future;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::process::Command;
use tokio::sync::RwLock;

pub mod content;
pub mod create;
pub mod update;

/// get a cluster by its specified [`ClusterPath`].
#[tracing::instrument]
pub async fn get(path: &ClusterPath) -> crate::Result<Option<Cluster>> {
	let state = State::get().await?;
	let clusters = state.clusters.read().await;
	let cluster = clusters.0.get(path).cloned();

	Ok(cluster)
}

/// get a cluster by its specified [`ClusterPath`].
#[tracing::instrument]
pub async fn get_by_name(name: &str) -> crate::Result<Option<Cluster>> {
	let state = State::get().await?;
	let clusters = state.clusters.read().await;
	let cluster = clusters.0.values().find(|c| c.meta.name == name).cloned();

	Ok(cluster)
}

/// get a list of all [`Cluster`]s
#[tracing::instrument]
pub async fn list() -> crate::Result<Vec<Cluster>> {
	let state = State::get().await?;
	let clusters = state.clusters.read().await;
	Ok(clusters.0.clone().into_iter().map(|it| it.1).collect())
}

/// get a Map of [`Cluster`] group names containing lists of [`Cluster`]s
#[tracing::instrument]
pub async fn list_grouped() -> crate::Result<HashMap<String, Vec<Cluster>>> {
	// TODO: This can 100% be made better
	let list = list().await?;
	let mut map = HashMap::<String, Vec<Cluster>>::new();

	for cluster in list {
		let group = cluster
			.meta
			.group
			.clone()
			.unwrap_or_else(|| String::from("Ungrouped"));
		if let Some(items) = map.get_mut(&group) {
			items.push(cluster);
		} else {
			map.insert(group, vec![cluster]);
		}
	}

	Ok(map)
}

/// run a Minecraft [`Cluster`] using the default credentials.
#[tracing::instrument]
pub async fn run_default(path: &ClusterPath) -> crate::Result<Arc<RwLock<ProcessorChild>>> {
	let state = State::get().await?;
	let default = {
		let mut users = state.users.write().await;
		users
			.get_default()
			.await?
			.ok_or_else(|| anyhow::anyhow!("no default credentials found!"))?
	};

	run_credentials(path, &default).await
}

/// run a Minecraft [`Cluster`] using [`MinecraftCredentials`] for authentication.
/// returns an [`Arc`] pointer to [`RwLock`] to [`ProcessorChild`].
#[tracing::instrument(skip(creds))]
#[onelauncher_macros::memory]
pub async fn run(
	path: &ClusterPath,
	creds: Option<&MinecraftCredentials>,
) -> crate::Result<Arc<RwLock<ProcessorChild>>> {
	if let Some(creds) = creds {
		run_credentials(path, creds).await
	} else {
		run_default(path).await
	}
}

/// run a Minecraft [`Cluster`] using [`MinecraftCredentials`] for authentication.
/// returns an [`Arc`] pointer to [`RwLock`] to [`ProcessorChild`].
#[tracing::instrument(skip(creds))]
#[onelauncher_macros::memory]
pub async fn run_credentials(
	path: &ClusterPath,
	creds: &MinecraftCredentials,
) -> crate::Result<Arc<RwLock<ProcessorChild>>> {
	let state = State::get().await?;
	let settings = state.settings.read().await;
	let cluster = get(path)
		.await?
		.ok_or_else(|| anyhow::anyhow!("failed to run a nonexistent cluster at path {}", path))?;

	let pre = &cluster
		.init_hooks
		.as_ref()
		.unwrap_or(&settings.init_hooks)
		.pre
		.as_ref()
		.and_then(|it| {
			if it.is_empty() {
				None
			} else {
				Some(it.clone())
			}
		});

	if let Some(hook) = pre {
		// TODO: hook parameters
		let mut cmd = hook.split(' ');
		if let Some(command) = cmd.next() {
			let full_path = path.full_path().await?;
			let result = Command::new(command)
				.args(cmd)
				.current_dir(&full_path)
				.spawn()
				.map_err(|e| IOError::with_path(e, &full_path))?
				.wait()
				.await
				.map_err(IOError::from)?;

			if !result.success() {
				return Err(anyhow::anyhow!(
					"non-zero exit code for pre-launch hook: {}",
					result.code().unwrap_or(-1)
				)
				.into());
			}
		}
	}

	let java_args = cluster
		.java
		.as_ref()
		.and_then(|it| it.custom_arguments.as_ref())
		.unwrap_or(&settings.custom_java_args);

	let wrapper = cluster
		.init_hooks
		.as_ref()
		.map_or(&settings.init_hooks.wrapper, |it| &it.wrapper)
		.as_ref()
		.and_then(|it| {
			if it.is_empty() {
				None
			} else {
				Some(it.clone())
			}
		});

	let memory = cluster.memory.unwrap_or(settings.memory);
	let resolution = cluster.resolution.unwrap_or(settings.resolution);
	let env_args = cluster
		.java
		.as_ref()
		.and_then(|it| it.custom_env_arguments.as_ref())
		.unwrap_or(&settings.custom_env_args);
	let post = cluster
		.init_hooks
		.as_ref()
		.unwrap_or(&settings.init_hooks)
		.post
		.clone();
	let mut mc_options: Vec<(String, String)> = vec![];
	if let Some(fullscreen) = cluster.force_fullscreen {
		mc_options.push(("fullscreen".to_string(), fullscreen.to_string()));
	} else if settings.force_fullscreen {
		mc_options.push(("fullscreen".to_string(), "true".to_string()));
	}

	let process = crate::game::launch_minecraft(
		&cluster,
		java_args,
		env_args,
		&mc_options,
		post,
		creds,
		&resolution,
		&memory,
		&wrapper,
	)
	.await?;

	Ok(process)
}

/// remove a specified cluster from it's [`ClusterPath`].
#[tracing::instrument]
pub async fn remove(path: &ClusterPath) -> crate::Result<()> {
	let state = State::get().await?;
	let mut clusters = state.clusters.write().await;

	if let Some(cluster) = clusters.remove(path).await? {
		send_cluster(
			cluster.uuid,
			path,
			&cluster.meta.name,
			ClusterPayloadType::Deleted,
		)
		.await?;
	}

	Ok(())
}

/// get a cluster by it's [`uuid::Uuid`]
#[tracing::instrument]
pub async fn get_by_uuid(uuid: uuid::Uuid) -> crate::Result<Option<Cluster>> {
	let state = State::get().await?;
	let clusters = state.clusters.read().await;
	let cluster = clusters.0.values().find(|c| c.uuid == uuid).cloned();

	Ok(cluster)
}

/// get a cluster's full path by it's [`ClusterPath`].
#[tracing::instrument]
pub async fn get_full_path(path: &ClusterPath) -> crate::Result<PathBuf> {
	let _ = get(path).await?.ok_or_else(|| {
		anyhow::anyhow!("failed to get the full path of cluster at path {}", path)
	})?;
	let full_path = io::canonicalize(path.full_path().await?)?;

	Ok(full_path)
}

/// get a specific mod's full path in the filesystem by it's [`ClusterPath`] and [`PackagePath`].
#[tracing::instrument]
pub async fn get_package_full_path(
	cluster_path: &ClusterPath,
	package_path: &PackagePath,
) -> crate::Result<PathBuf> {
	if get(cluster_path).await?.is_some() {
		let full_path = io::canonicalize(package_path.full_path(cluster_path).await?)?;

		return Ok(full_path);
	}

	Err(anyhow::anyhow!(
		"failed to get the full path of a cluster at path {}",
		package_path
			.full_path(cluster_path)
			.await?
			.display()
	)
	.into())
}

/// edit a cluster with an async closure and it's [`ClusterPath`]
pub async fn edit<FutFn>(
	path: &ClusterPath,
	action: impl Fn(&mut Cluster) -> FutFn,
) -> crate::Result<()>
where
	FutFn: Future<Output = crate::Result<()>>,
{
	let state = State::get().await?;
	let mut clusters = state.clusters.write().await;

	match clusters.0.get_mut(path) {
		Some(ref mut cluster) => {
			action(cluster).await?;

			send_cluster(
				cluster.uuid,
				path,
				&cluster.meta.name,
				ClusterPayloadType::Edited,
			)
			.await?;

			Ok(())
		}
		None => Err(anyhow::anyhow!("unmanaged cluster edited at {}", path.to_string()).into()),
	}
}

/// update a [`Cluster`]'s icon
pub async fn edit_icon(path: &ClusterPath, icon_path: Option<&Path>) -> crate::Result<()> {
	let state = State::get().await?;
	let result = if let Some(icon) = icon_path {
		let bytes = io::read(icon).await?;
		let mut clusters = state.clusters.write().await;
		match clusters.0.get_mut(path) {
			Some(ref mut cluster) => {
				cluster
					.set_icon_bytes(
						&state.directories.caches_dir().await,
						&state.io_semaphore,
						bytes::Bytes::from(bytes),
						&icon.to_string_lossy(),
					)
					.await?;

				send_cluster(
					cluster.uuid,
					path,
					&cluster.meta.name,
					ClusterPayloadType::Edited,
				)
				.await?;
				Ok(())
			}
			None => Err(anyhow::anyhow!(
				"failed to update unmanaged cluster at {}",
				path.to_string()
			)
			.into()),
		}
	} else {
		edit(path, |cluster| {
			cluster.meta.icon = None;
			async { Ok(()) }
		})
		.await?;
		State::sync().await?;

		Ok(())
	};

	State::sync().await?;
	result
}

/// gets the optimal java version for a given [`Cluster`].
pub async fn get_optimal_java_version(path: &ClusterPath) -> crate::Result<Option<JavaVersion>> {
	let state = State::get().await?;
	if let Some(cluster) = get(path).await? {
		let metadata = state.metadata.read().await;
		let minecraft_metadata = metadata
			.minecraft
			.clone()
			.ok_or_else(|| anyhow::anyhow!("couldn't get minecraft metadata"))?;

		let version = minecraft_metadata
			.versions
			.iter()
			.find(|it| it.id == cluster.meta.mc_version)
			.ok_or_else(|| {
				anyhow::anyhow!(
					"invalid or unknown Minecraft version {}",
					cluster.meta.mc_version
				)
			})?;

		let version_info = crate::game::metadata::download_version_info(
			&state,
			version,
			cluster.meta.loader_version.as_ref(),
			None,
			None,
		)
		.await?;

		let version = crate::game::java_version_from_cluster(&cluster, &version_info).await?;

		Ok(version)
	} else {
		Err(anyhow::anyhow!(
			"failed to get the java version of unmanaged cluster at {}",
			path.to_string()
		)
		.into())
	}
}

/// Try to update a [`Cluster`]'s playtime.
#[tracing::instrument]
#[onelauncher_macros::memory]
pub async fn update_playtime(path: &ClusterPath) -> crate::Result<()> {
	let state = State::get().await?;
	let cluster = get(path)
		.await?
		.ok_or_else(|| anyhow::anyhow!("failed to update playtime at path {}", path))?;
	let recent_playtime = cluster.meta.recently_played;

	/*
	 * todo
	 */

	let mut clusters = state.clusters.write().await;
	if let Some(cluster) = clusters.0.get_mut(path) {
		cluster.meta.overall_played += recent_playtime;
		cluster.meta.recently_played = 0;
	}
	drop(clusters);

	State::sync().await?;

	Ok(())
}

/// This forces the cluster to be verified (and possibly reinstalled).
pub async fn repair_cluster(path: &ClusterPath) -> crate::Result<()> {
	let cluster = get(path)
		.await?
		.ok_or_else(|| anyhow::anyhow!("failed to repair cluster at path {}", path))?;

	crate::game::install_minecraft(&cluster, None, true).await
}

/// Sanitize a user-inputted [`Cluster`] name.
#[must_use]
pub fn sanitize_cluster_name(input: &str) -> String {
	input.replace(['/', '\\', '?', '*', ':', '\'', '\"', '|', '<', '>'], "_")
}
