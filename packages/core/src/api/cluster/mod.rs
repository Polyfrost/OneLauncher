use chrono::Utc;
use dao::ClusterId;
use interpulse::api::minecraft::VersionInfo;
use onelauncher_entity::cluster_stage::ClusterStage;
use onelauncher_entity::{clusters, java_versions};
use onelauncher_entity::icon::Icon;
use onelauncher_entity::loader::GameLoader;
use sea_orm::ActiveValue::Set;
use tokio::process::Command;

use crate::api::game::arguments;
use crate::api::game::metadata::{self, download_minecraft};
use crate::api::ingress::IngressSendExt;
use crate::api::{java, setting_profiles};
use crate::error::LauncherResult;
use crate::store::ingress::{IngressType, SubIngress};
use crate::store::{Dirs, State};
use crate::utils::io::{self, IOError};

pub mod dao;

mod sync;
pub use sync::*;

use super::ingress::init_ingress;

#[derive(Debug, thiserror::Error)]
pub enum ClusterError {
	#[error("version '{0}' was not found")]
	InvalidVersion(String),
	#[error("failed to imply java version")]
	MissingJavaVersion,
	#[error("cluster is in downloading stage")]
	ClusterDownloading,
	#[error("cluster is already running")]
	ClusterAlreadyRunning,
}

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
	let cluster_dir = Dirs::get_clusters_dir().await?;

	// Get the directory for the cluster
	let mut folder_name = name.clone();
	let mut path = cluster_dir.join(&folder_name);

	// Folder name conflict resolution
	if path.exists() {
		let mut which = 1;
		loop {
			let new_name = format!("{folder_name} ({which})");
			path = cluster_dir.join(&new_name);
			if !path.exists() {
				folder_name = new_name;
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
			&folder_name,
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

/// Make a cluster playable (installs all necessary game files and dependencies required to play)
pub async fn prepare_cluster(cluster: &mut clusters::Model, force: Option<bool>) -> LauncherResult<&clusters::Model> {
	const INGRESS_TOTAL: f64 = 100.0;
	const INGRESS_TASKS: f64 = 4.0;
	const INGRESS_STEP: f64 = INGRESS_TOTAL / INGRESS_TASKS;

	tracing::debug!("preparing cluster {}", cluster.name);

	// if cluster.stage == ClusterStage::Ready {
	// 	tracing::info!("cluster is already ready");
	// 	return Ok(cluster);
	// }

	let ingress_id = init_ingress(
		IngressType::PrepareCluster {
			cluster_name: cluster.name.clone(),
		},
		"preparing cluster",
		INGRESS_TOTAL,
	)
	.await?;

	dao::update_cluster(cluster, async |mut cluster| {
		cluster.stage = Set(ClusterStage::Downloading);
		Ok(cluster)
	})
	.await?;

	let result: LauncherResult<()> = {
		// TASK 1
		ingress_id.set_ingress_message("fetching data").await?;
		let setting_profile =
			setting_profiles::dao::get_profile_or_default(cluster.setting_profile_name.as_ref()).await?;

		let state = State::get().await?;
		let mut metadata = state.metadata.write().await;

		let version = metadata
			.get_vanilla_or_fetch()
			.await?
			.versions
			.iter()
			.find(|v| v.id == cluster.mc_version)
			.ok_or_else(|| ClusterError::InvalidVersion(cluster.mc_version.clone()))?
			.clone();

		drop(metadata);

		let loader_version = metadata::get_loader_version(
			&cluster.mc_version,
			cluster.mc_loader,
			cluster.mc_loader_version.as_deref(),
		)
		.await?;

		// TASK 2
		let mut version_info = metadata::download_version_info(
			&version,
			loader_version.as_ref(),
			Some(&SubIngress::new(&ingress_id, INGRESS_STEP)),
			force,
		)
		.await?;

		let java_version = if let Some(version) = java::get_recommended_java(&version_info, Some(&setting_profile)).await? {
			version
		} else {
			let Some(ver) = &version_info.java_version else {
				return Err(ClusterError::MissingJavaVersion.into());
			};

			java::prepare_java(ver.major_version).await?
		};

		// TASK 3
		download_minecraft(&version_info, &java_version.arch, Some(&SubIngress::new(&ingress_id, INGRESS_STEP)), force).await?;

		// TASK 4
		run_forge_processors(cluster, &mut version_info, java_version, &SubIngress::new(&ingress_id, INGRESS_STEP)).await?;

		Ok(())
	};

	if let Err(err) = result {
		tracing::error!("failed to prepare cluster: {}", err);
		dao::update_cluster(cluster, async |mut cluster| {
			cluster.stage = Set(ClusterStage::NotReady);
			Ok(cluster)
		})
		.await?;
		return Err(err);
	}

	dao::update_cluster(cluster, async |mut cluster| {
		cluster.stage = Set(ClusterStage::Ready);
		Ok(cluster)
	})
	.await?;
	tracing::debug!("cluster is ready");

	Ok(cluster)
}

/// Run forge processors
async fn run_forge_processors(
	cluster: &clusters::Model,
	version_info: &mut VersionInfo,
	java_version: java_versions::Model,
	ingress: &SubIngress<'_>,
) -> LauncherResult<()> {
	let Some(processors) = &version_info.processors else {
		return Ok(());
	};

	let dirs = Dirs::get().await?;
	let client = dirs
		.versions_dir()
		.join(format!("{}.jar", &version_info.id));
	let libraries = dirs.libraries_dir();

	let Some(data) = &mut version_info.data else {
		return Ok(());
	};

	macro_rules! data_entry {
		($dest:expr; $($name:literal: client => $client:expr, server => $server:expr;)+) => {
			$(std::collections::HashMap::insert(
				$dest,
				String::from($name),
				interpulse::api::modded::SidedDataEntry {
					client: String::from($client),
					server: String::from($server),
				},
			);)+
		}
	}

	data_entry! {
		data;
		"SIDE":
			client => "client",
			server => "";
		"MINECRAFT_JAR":
			client => client.to_string_lossy(),
			server => "";
		"MINECRAFT_VERSION":
			client => cluster.mc_version.clone(),
			server => "";
		"ROOT":
			client => dirs.clusters_dir().join(cluster.folder_name.clone()).to_string_lossy(),
			server => "";
		"LIBRARY_DIR":
			client => libraries.to_string_lossy(),
			server => "";
	}

	ingress.set_ingress_message("running forge processors").await?;
	let total_length = processors.len();
	for (index, processor) in processors.iter().enumerate() {
		if let Some(sides) = &processor.sides {
			if !sides.contains(&String::from("client")) {
				continue;
			}
		}

		let mut cp = processor.classpath.clone();
		cp.push(processor.jar.clone());

		let child = Command::new(&java_version.absolute_path)
			.arg("-cp")
			.arg(arguments::get_classpath_library(
				&libraries,
				&cp,
			)?)
			.arg(
				arguments::main_class(arguments::get_library(&libraries, &processor.jar, false)?)
					.await?
					.ok_or_else(|| {
						anyhow::anyhow!("failed to find processor main class for {}", processor.jar)
					})?,
			)
			.args(arguments::processor_arguments(
				&libraries,
				&processor.args,
				data,
			)?)
			.output()
			.await
			.map_err(IOError::from)
			.map_err(|err| anyhow::anyhow!("failed to run processor: {err}"))?;

		if !child.status.success() {
			return Err(anyhow::anyhow!(
				"error occured while running processor: {}",
				String::from_utf8_lossy(&child.stderr)
			)
			.into());
		}

		ingress.set_ingress_message(&format!("running forge processors {index}/{total_length}")).await?;
	}

	Ok(())
}

pub async fn update_playtime(id: ClusterId, duration: i64) -> LauncherResult<clusters::Model> {
	dao::update_cluster_by_id(id, async |mut cluster| {
		let overall_played = cluster.overall_played.take().unwrap_or_default().unwrap_or_default();

		cluster.overall_played = Set(Some(overall_played + duration));
		cluster.last_played = Set(Some(Utc::now()));

		Ok(cluster)
	}).await
}