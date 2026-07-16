use oneclient_db::dao::{cluster as cluster_dao, setting_profile as profile_dao};
use oneclient_db::models::{ClusterId, ClusterPatch, NewCluster};

use crate::packages::domain::ContentType;
use crate::settings::store::{
	create_profile_from_global, resolve_cluster_profile, update_named_profile,
};
use crate::patch::Patch;
use crate::settings::{GameSettingsProfile, ProfileUpdate};
use crate::state::LauncherState;
use crate::LauncherResult;

use std::sync::Arc;

use super::cluster::Cluster;
use super::error::ClusterError;
use super::options::{ClusterUpdate, CreateClusterOptions};
use super::prepare;
use super::stage::ClusterStage;

pub struct ClusterManager;

impl ClusterManager {
	pub fn sanitize_name(name: &str) -> String {
		let mut name = name.to_string();
		name.retain(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')'));
		name.trim().to_string()
	}

	#[tracing::instrument(level = "debug", skip(state))]
	pub async fn get(state: &LauncherState, cluster_id: ClusterId) -> LauncherResult<Cluster> {
		let row = cluster_dao::get_by_id(&state.services.db, cluster_id)
			.await?
			.ok_or(ClusterError::NotFound(cluster_id))?;
		Ok(Cluster::try_from_row(row)?)
	}

	#[tracing::instrument(level = "debug", skip(state))]
	pub async fn list(state: &LauncherState) -> LauncherResult<Vec<Cluster>> {
		let rows = cluster_dao::list_all(&state.services.db).await?;
		rows.into_iter()
			.map(Cluster::try_from_row)
			.collect::<Result<Vec<_>, _>>()
			.map_err(Into::into)
	}

	#[tracing::instrument(skip(state))]
	pub async fn create(
		state: &LauncherState,
		options: CreateClusterOptions,
	) -> LauncherResult<Cluster> {
		let name = Self::sanitize_name(&options.name);
		if name.is_empty() {
			return Err(ClusterError::EmptyName.into());
		}

		let folder_name = resolve_unique_folder_name(&name).await?;
		let cluster_path = crate::paths::clusters_dir()?.join(&folder_name);

		match create_inner(state, &options, &name, &folder_name, &cluster_path).await {
			Ok(cluster) => {
				tracing::info!(cluster_id = cluster.id, name = %cluster.name, "created cluster");
				Ok(cluster)
			}
			Err(err) => {
				tracing::warn!(name = %name, error = %err, "cluster creation failed, cleaning up directory");
				let _ = polyio::remove_dir_all(&cluster_path).await;
				Err(err)
			}
		}
	}

	#[tracing::instrument(level = "debug", skip(state))]
	pub async fn update(
		state: &LauncherState,
		cluster_id: ClusterId,
		update: ClusterUpdate,
	) -> LauncherResult<Cluster> {
		let _ = Self::get(state, cluster_id).await?;

		if let Patch::Set(ref profile_name) = update.setting_profile_name {
			ensure_profile_exists(&state.services.db, profile_name).await?;
		}

		let name = update.name.as_deref().map(Self::sanitize_name);
		if name.as_deref().is_some_and(str::is_empty) {
			return Err(ClusterError::EmptyName.into());
		}

		let patch = ClusterPatch {
			name,
			setting_profile_name: update.setting_profile_name.into_db_patch(),
			mc_loader_version: update.mc_loader_version.into_db_patch(),
			linked_modpack_hash: update.linked_modpack_hash.into_db_patch(),
		};

		let row = cluster_dao::update(&state.services.db, cluster_id, &patch).await?;
		Ok(Cluster::try_from_row(row)?)
	}

	#[tracing::instrument(skip(state))]
	pub async fn delete(
		state: &LauncherState,
		cluster_id: ClusterId,
		remove_files: bool,
	) -> LauncherResult<()> {
		let cluster = Self::get(state, cluster_id).await?;

		if !cluster_dao::delete_by_id(&state.services.db, cluster_id).await? {
			return Err(ClusterError::NotFound(cluster_id).into());
		}

		if remove_files {
			let path = cluster.dir()?;
			if path.exists() {
				polyio::remove_dir_all(&path).await?;
			}
		}

		tracing::info!(cluster_id, remove_files, "deleted cluster");
		Ok(())
	}

	#[tracing::instrument(level = "debug", skip(state))]
	pub async fn set_stage(
		state: &LauncherState,
		cluster_id: ClusterId,
		stage: ClusterStage,
	) -> LauncherResult<Cluster> {
		let row = cluster_dao::set_stage(&state.services.db, cluster_id, stage as i64).await?;
		Ok(Cluster::try_from_row(row)?)
	}

	#[tracing::instrument(level = "debug", skip(state))]
	pub async fn uses_dedicated_dir(
		state: &LauncherState,
		cluster_id: ClusterId,
	) -> LauncherResult<bool> {
		Ok(Self::get(state, cluster_id).await?.uses_dedicated_dir())
	}

	#[tracing::instrument(level = "debug", skip(state))]
	pub async fn set_dedicated_dir(
		state: &LauncherState,
		cluster_id: ClusterId,
		dedicated: bool,
	) -> LauncherResult<()> {
		if state.games.is_active(cluster_id) {
			return Err(crate::game::GameError::AlreadyRunning(cluster_id).into());
		}

		let cluster = Self::get(state, cluster_id).await?;
		let marker = cluster.dedicated_marker()?;
		if dedicated {
			polyio::create_dir_all(cluster.dir()?).await?;
			polyio::write(&marker, b"").await.ok();
		} else if marker.exists() {
			polyio::remove_file(&marker).await.ok();
		}
		Ok(())
	}

	#[tracing::instrument(level = "debug", skip(state))]
	pub async fn add_playtime(
		state: &LauncherState,
		cluster_id: ClusterId,
		duration: std::time::Duration,
	) -> LauncherResult<Cluster> {
		let seconds = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
		let row = cluster_dao::add_playtime(&state.services.db, cluster_id, seconds).await?;

		Ok(Cluster::try_from_row(row)?)
	}

	#[tracing::instrument(level = "debug", skip(state, cluster), fields(cluster_id = cluster.id))]
	pub async fn resolve_settings(
		state: &LauncherState,
		cluster: &Cluster,
	) -> LauncherResult<GameSettingsProfile> {
        let settings = state.settings.read().clone();

		resolve_cluster_profile(
			&state.services.db,
			&settings,
			cluster.setting_profile_name.as_deref(),
		)
		.await
	}

	#[tracing::instrument(level = "debug", skip(state, update))]
	pub async fn update_profile(
		state: &LauncherState,
		cluster_id: ClusterId,
		update: ProfileUpdate,
	) -> LauncherResult<GameSettingsProfile> {
		let cluster = Self::get(state, cluster_id).await?;
		let profile_name = cluster
			.setting_profile_name
			.ok_or(ClusterError::NoProfile)?;

		update_named_profile(&state.services.db, &profile_name, update).await
	}

	#[tracing::instrument(skip(state, shared_progress))]
	pub async fn prepare(
		state: &Arc<LauncherState>,
		cluster_id: ClusterId,
		force: bool,
		search_for_java: bool,
		auto_install_java: bool,
		shared_progress: Option<&crate::notification::GroupedProgressSession>,
	) -> LauncherResult<Cluster> {
		let mut metadata = state.metadata.lock().await;
		prepare::prepare_cluster(
			state,
			&mut metadata,
			cluster_id,
			force,
			search_for_java,
			auto_install_java,
			shared_progress,
		)
		.await
	}

	#[tracing::instrument(level = "debug", skip(state))]
	pub async fn create_and_assign_profile(
		state: &LauncherState,
		cluster_id: ClusterId,
		profile_name: &str,
	) -> LauncherResult<GameSettingsProfile> {
        let settings = state.settings.read().clone();

		let profile = create_profile_from_global(
			&state.services.db,
			&settings,
			profile_name,
			None,
			None,
		)
		.await?;

		Self::update(
			state,
			cluster_id,
			ClusterUpdate::default().setting_profile(&profile.name),
		)
		.await?;

		Ok(profile)
	}
}

#[tracing::instrument(level = "debug", skip(state, options))]
async fn create_inner(
	state: &LauncherState,
	options: &CreateClusterOptions,
	name: &str,
	folder_name: &str,
	cluster_path: &std::path::Path,
) -> LauncherResult<Cluster> {
	polyio::create_dir_all(cluster_path).await?;
	ensure_content_dirs(cluster_path).await?;

    let settings = state.settings.read().clone();

	let profile = create_profile_from_global(
		&state.services.db,
		&settings,
		name,
		options.mem_max,
		None,
	)
	.await?;

	let row = cluster_dao::insert(
		&state.services.db,
		&NewCluster {
			name,
			folder_name,
			mc_version: &options.mc_version,
			mc_loader: options.mc_loader as i64,
			mc_loader_version: options.mc_loader_version.as_deref(),
			setting_profile_name: Some(&profile.name),
			stage: ClusterStage::NotReady as i64,
		},
	)
	.await?;

	Ok(Cluster::try_from_row(row)?)
}

#[tracing::instrument(level = "debug", skip(pool))]
async fn ensure_profile_exists(pool: &oneclient_db::DbPool, name: &str) -> LauncherResult<()> {
	if profile_dao::get_by_name(pool, name).await?.is_none() {
		return Err(ClusterError::ProfileNotFound(name.to_string()).into());
	}
	Ok(())
}

#[tracing::instrument(level = "debug")]
async fn resolve_unique_folder_name(name: &str) -> LauncherResult<String> {
	let cluster_dir = crate::paths::clusters_dir()?;
	let mut folder_name = name.to_string();
	let mut path = cluster_dir.join(&folder_name);

	if path.exists() {
		let mut which = 1;
		loop {
			let candidate = format!("{folder_name} ({which})");
			path = cluster_dir.join(&candidate);
			if !path.exists() {
				folder_name = candidate;
				break;
			}
			which += 1;
		}
	}

	Ok(folder_name)
}

#[tracing::instrument(level = "debug")]
async fn ensure_content_dirs(cluster_path: &std::path::Path) -> LauncherResult<()> {
	for content_type in [
		ContentType::Mod,
		ContentType::ResourcePack,
		ContentType::Shader,
		ContentType::DataPack,
		ContentType::World,
	] {
		polyio::create_dir_all(cluster_path.join(content_type.folder_name())).await?;
	}
	Ok(())
}
