use oneclient_db::dao::{cluster as cluster_dao, setting_profile as profile_dao};
use oneclient_db::models::{ClusterId, ClusterPatch, NewCluster};

use oneclient_common::domain::ContentType;
use crate::profiles::{
	create_profile_from_global, resolve_cluster_profile, update_named_profile,
};
use oneclient_common::patch::Patch;
use crate::profile::GameSettingsProfile;
use crate::profiles::ProfileUpdate;
use crate::error::ClusterResult;

use oneclient_db::DbPool;
use tokio::sync::Mutex;

use crate::cluster::{Cluster, remove_mods_link};
use crate::error::ClusterError;
use crate::options::{ClusterUpdate, CreateClusterOptions};
use crate::stage::ClusterStage;

pub struct ClusterManager {
	db: DbPool,
	/// Serialises creation so two concurrent creates cannot resolve to the same
	/// folder name and race each other onto disk
	provisioning: Mutex<()>,
}

impl ClusterManager {
	#[must_use]
	pub fn new(db: DbPool) -> Self {
		Self {
			db,
			provisioning: Mutex::new(()),
		}
	}

	/// Orphan-folder recovery must hold this across its whole scan or a
	/// concurrent create's folder is adopted into a duplicate row
	pub async fn provisioning_guard(&self) -> tokio::sync::MutexGuard<'_, ()> {
		self.provisioning.lock().await
	}

	pub fn sanitize_name(name: &str) -> String {
		let mut name = name.to_string();
		name.retain(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')'));
		name.trim().to_string()
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn get(&self, cluster_id: ClusterId) -> ClusterResult<Cluster> {
		let row = cluster_dao::get_by_id(&self.db, cluster_id)
			.await?
			.ok_or(ClusterError::NotFound(cluster_id))?;
		Cluster::try_from_row(row)
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn list(&self) -> ClusterResult<Vec<Cluster>> {
		let rows = cluster_dao::list_all(&self.db).await?;
		rows.into_iter()
			.map(Cluster::try_from_row)
			.collect::<Result<Vec<_>, _>>()}

	#[tracing::instrument(skip(self))]
	pub async fn create(
		&self,
		global: &GameSettingsProfile,
		options: CreateClusterOptions,
	) -> ClusterResult<Cluster> {
		let _guard = self.provisioning.lock().await;
		self.create_core(global, options).await
	}

	/// Returns `None` if a cluster for this version/loader already exists
	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn create_provisioned(
		&self,
		global: &GameSettingsProfile,
		options: CreateClusterOptions,
	) -> ClusterResult<Option<Cluster>> {
		let _guard = self.provisioning.lock().await;
		if cluster_dao::find_by_version_loader(
			&self.db,
			&options.mc_version,
			options.mc_loader as i64,
		)
		.await?
		.is_some()
		{
			return Ok(None);
		}
		self.create_core(global, options).await.map(Some)
	}

	/// Callers MUST hold `self.provisioning` this does not lock
	#[tracing::instrument(level = "debug", skip(self, global))]
	async fn create_core(
		&self,
		global: &GameSettingsProfile,
		options: CreateClusterOptions,
	) -> ClusterResult<Cluster> {
		let name = Self::sanitize_name(&options.name);
		if name.is_empty() {
			return Err(ClusterError::EmptyName);
		}

		let folder_name = resolve_unique_folder_name(&name).await?;
		let cluster_path = oneclient_common::paths::clusters_dir()?.join(&folder_name);

		match create_inner(&self.db, global, &options, &name, &folder_name, &cluster_path).await {
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

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn update(
		&self,
		cluster_id: ClusterId,
		update: ClusterUpdate,
	) -> ClusterResult<Cluster> {
		let _ = self.get(cluster_id).await?;

		if let Patch::Set(ref profile_name) = update.setting_profile_name {
			ensure_profile_exists(&self.db, profile_name).await?;
		}

		let name = update.name.as_deref().map(Self::sanitize_name);
		if name.as_deref().is_some_and(str::is_empty) {
			return Err(ClusterError::EmptyName);
		}

		let patch = ClusterPatch {
			name,
			setting_profile_name: update.setting_profile_name.into_db_patch(),
			mc_loader_version: update.mc_loader_version.into_db_patch(),
			linked_modpack_hash: update.linked_modpack_hash.into_db_patch(),
		};

		let row = cluster_dao::update(&self.db, cluster_id, &patch).await?;
		Cluster::try_from_row(row)
	}

	#[tracing::instrument(skip(self))]
	pub async fn delete(
		&self,
		cluster_id: ClusterId,
		remove_files: bool,
	) -> ClusterResult<()> {
		let cluster = self.get(cluster_id).await?;

		if !cluster_dao::delete_by_id(&self.db, cluster_id).await? {
			return Err(ClusterError::NotFound(cluster_id));
		}

		remove_mods_link(&cluster.folder_name).await;

		if remove_files {
			let path = cluster.dir()?;
			if path.exists() {
				polyio::remove_dir_all(&path).await?;
			}
		}

		tracing::info!(cluster_id, remove_files, "deleted cluster");
		Ok(())
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn set_stage(
		&self,
		cluster_id: ClusterId,
		stage: ClusterStage,
	) -> ClusterResult<Cluster> {
		let row = cluster_dao::set_stage(&self.db, cluster_id, stage as i64).await?;
		Cluster::try_from_row(row)
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn uses_dedicated_dir(&self, cluster_id: ClusterId) -> ClusterResult<bool> {
		Ok(self.get(cluster_id).await?.uses_dedicated_dir())
	}

	#[tracing::instrument(level = "debug", skip(self))]
	/// `is_running` is passed in because process liveness belongs to the game
	/// lifecycle not to cluster records
	pub async fn set_dedicated_dir(
		&self,
		cluster_id: ClusterId,
		dedicated: bool,
		is_running: bool,
	) -> ClusterResult<()> {
		if is_running {
			return Err(ClusterError::AlreadyRunning(cluster_id));
		}

		let cluster = self.get(cluster_id).await?;
		let marker = cluster.dedicated_marker()?;
		if dedicated {
			polyio::create_dir_all(cluster.dir()?).await?;
			polyio::write(&marker, b"").await.ok();
		} else if marker.exists() {
			polyio::remove_file(&marker).await.ok();
		}
		Ok(())
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn add_playtime(
		&self,
		cluster_id: ClusterId,
		duration: std::time::Duration,
	) -> ClusterResult<Cluster> {
		let seconds = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
		let row = cluster_dao::add_playtime(&self.db, cluster_id, seconds).await?;

		Cluster::try_from_row(row)
	}

	#[tracing::instrument(level = "debug", skip(self, global, cluster), fields(cluster_id = cluster.id))]
	pub async fn resolve_settings(
		&self,
		global: &GameSettingsProfile,
		cluster: &Cluster,
	) -> ClusterResult<GameSettingsProfile> {
		resolve_cluster_profile(
			&self.db,
			global,
			cluster.setting_profile_name.as_deref(),
		)
		.await
	}

	#[tracing::instrument(level = "debug", skip(self, update))]
	pub async fn update_profile(
		&self,
		cluster_id: ClusterId,
		update: ProfileUpdate,
	) -> ClusterResult<GameSettingsProfile> {
		let cluster = self.get(cluster_id).await?;
		let profile_name = cluster
			.setting_profile_name
			.ok_or(ClusterError::NoProfile)?;

		update_named_profile(&self.db, &profile_name, update).await
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn create_and_assign_profile(
		&self,
		global: &GameSettingsProfile,
		cluster_id: ClusterId,
		profile_name: &str,
	) -> ClusterResult<GameSettingsProfile> {
		let profile = create_profile_from_global(
			&self.db,
			global,
			profile_name,
			None,
			None,
		)
		.await?;

		self.update(
			cluster_id,
			ClusterUpdate::default().setting_profile(&profile.name),
		)
		.await?;

		Ok(profile)
	}
}

#[tracing::instrument(level = "debug", skip(db, global, options))]
async fn create_inner(
	db: &DbPool,
	global: &GameSettingsProfile,
	options: &CreateClusterOptions,
	name: &str,
	folder_name: &str,
	cluster_path: &std::path::Path,
) -> ClusterResult<Cluster> {
	polyio::create_dir_all(cluster_path).await?;
	ensure_content_dirs(cluster_path).await?;

	let profile = create_profile_from_global(
		db,
		global,
		name,
		options.mem_max,
		None,
	)
	.await?;

	let row = cluster_dao::insert(
		db,
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

	Cluster::try_from_row(row)
}

#[tracing::instrument(level = "debug", skip(pool))]
async fn ensure_profile_exists(pool: &oneclient_db::DbPool, name: &str) -> ClusterResult<()> {
	if profile_dao::get_by_name(pool, name).await?.is_none() {
		return Err(ClusterError::ProfileNotFound(name.to_string()));
	}
	Ok(())
}

#[tracing::instrument(level = "debug")]
async fn resolve_unique_folder_name(name: &str) -> ClusterResult<String> {
	let cluster_dir = oneclient_common::paths::clusters_dir()?;
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
async fn ensure_content_dirs(cluster_path: &std::path::Path) -> ClusterResult<()> {
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
