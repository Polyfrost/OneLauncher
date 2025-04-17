use std::sync::Arc;

use onelauncher_entity::{cluster_stage::ClusterStage, prelude::model::*};
use tokio::sync::RwLock;
use merge::Merge;

use crate::{api::{cluster::{prepare_cluster, ClusterError}, game::metadata, java::{self, JavaError}, setting_profiles::{dao::get_profile_by_name, get_global_profile}}, error::LauncherResult, store::{credentials::MinecraftCredentials, processes::Process, State}, utils::io};

#[tracing::instrument]
pub async fn launch_minecraft(
	cluster: &mut Cluster,
	creds: &MinecraftCredentials,
) -> LauncherResult<Arc<RwLock<Process>>> {
	if cluster.stage.is_downloading() {
		return Err(ClusterError::ClusterDownloading.into());
	} else if cluster.stage == ClusterStage::NotReady {
		prepare_cluster(cluster).await?;
	}

	let mut settings = get_global_profile().await;
	if let Some(name) = &cluster.setting_profile_name {
		if let Some(profile) = get_profile_by_name(name).await? {
			settings.merge(profile);
		}
	}

	let state = State::get().await?;
	let instance_path = &io::canonicalize(&cluster.path)?;

	let mut metadata = state.metadata.write().await;
	let versions = &metadata.get_vanilla_or_fetch().await?.versions;

	let version_index = versions
		.iter()
		.position(|it| it.id == cluster.mc_version)
		.ok_or_else(|| anyhow::anyhow!("invalid game version {}", cluster.mc_version))?;


	let version = &versions[version_index];
	let updated = version_index <= versions.iter().position(|x| x.id == "22w16a").unwrap_or(0);

	let version_jar_name = cluster
		.mc_loader_version
		.as_ref()
		.map_or(version.id.clone(), |it| {
			format!("{}-{}", version.id.clone(), it.clone())
		});

	let loader_version = metadata::get_loader_version(&cluster.mc_version, cluster.mc_loader, cluster.mc_loader_version.as_deref()).await?;

	let version_info = metadata::download_version_info(
		version,
		loader_version.as_ref(),
		None,
		None,
	)
	.await?;

	drop(metadata);

	let java = java::get_recommended_java(&version_info, Some(&settings)).await?;
	if java.is_none() {
		return Err(JavaError::MissingJava.into());
	}



	todo!()
}