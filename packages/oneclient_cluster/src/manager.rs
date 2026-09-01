use oneclient_db::dao::{cluster as cluster_dao, setting_profile as profile_dao};
use oneclient_db::models::{ClusterId, ClusterPatch, NewCluster};

use oneclient_common::domain::ContentType;
use crate::profiles::{
	create_profile_from_global, delete_named_profile, resolve_cluster_profile,
	update_named_profile,
};
use oneclient_common::patch::Patch;
use crate::profile::GameSettingsProfile;
use crate::profiles::ProfileUpdate;
use crate::error::ClusterResult;

use oneclient_db::DbPool;
use tokio::sync::Mutex;

use crate::cluster::Cluster;
use crate::error::ClusterError;
use crate::identity::ClusterIdentity;
use crate::options::{ClusterUpdate, CreateClusterOptions};
use crate::stage::ClusterStage;

const MAX_NAME_CHARS: usize = 100;
const MAX_FOLDER_CHARS: usize = 40;
const FOLDER_FALLBACK: &str = "cluster";

const WINDOWS_RESERVED_STEMS: [&str; 24] = [
	"CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
	"COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

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

	pub fn normalize_name(name: &str) -> ClusterResult<String> {
		let cleaned: String = name.chars().filter(|c| !c.is_control()).collect();
		let trimmed = cleaned.trim();

		if trimmed.is_empty() {
			return Err(ClusterError::EmptyName);
		}

		let capped: String = trimmed.chars().take(MAX_NAME_CHARS).collect();
		Ok(capped.trim_end().to_string())
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

		let cluster = self.create_core(global, options).await?;
		mark_provisioned(&cluster).await;
		Ok(Some(cluster))
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn ensure_provisioned(&self, cluster_id: ClusterId) -> ClusterResult<()> {
		let cluster = self.get(cluster_id).await?;
		if !cluster.is_provisioned() {
			mark_provisioned(&cluster).await;
		}
		Ok(())
	}

	/// Callers MUST hold `self.provisioning` this does not lock
	#[tracing::instrument(level = "debug", skip(self, global))]
	async fn create_core(
		&self,
		global: &GameSettingsProfile,
		options: CreateClusterOptions,
	) -> ClusterResult<Cluster> {
		let name = Self::normalize_name(&options.name)?;

		let fallback = format!("{} {}", options.mc_version, options.mc_loader);
		let folder_name =
			resolve_unique_folder_name(&self.db, &folder_base(&name, &fallback)).await?;
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
		let existing = self.get(cluster_id).await?;

		if let Patch::Set(ref profile_name) = update.setting_profile_name {
			ensure_profile_exists(&self.db, profile_name).await?;
		}

		let name = update
			.name
			.as_deref()
			.map(Self::normalize_name)
			.transpose()?;

		if name.is_some() && existing.is_provisioned() {
			return Err(ClusterError::Provisioned);
		}

		let patch = ClusterPatch {
			name: name.clone(),
			setting_profile_name: update.setting_profile_name.into_db_patch(),
			mc_loader_version: update.mc_loader_version.into_db_patch(),
			linked_modpack_hash: update.linked_modpack_hash.into_db_patch(),
		};

		let row = cluster_dao::update(&self.db, cluster_id, &patch).await?;

		if let Some(name) = name {
			ClusterIdentity::amend(&existing.folder_name, |identity| {
				identity.name = name.clone();
			})
			.await;
		}

		Cluster::try_from_row(row)
	}

	#[tracing::instrument(skip(self))]
	pub async fn delete(
		&self,
		cluster_id: ClusterId,
		remove_files: bool,
	) -> ClusterResult<()> {
		let cluster = self.get(cluster_id).await?;

		if cluster.is_provisioned() {
			return Err(ClusterError::Provisioned);
		}

		if !cluster_dao::delete_by_id(&self.db, cluster_id).await? {
			return Err(ClusterError::NotFound(cluster_id));
		}

		if let Some(profile_name) = cluster.setting_profile_name.as_deref()
			&& profile_name == cluster.folder_name
		{
			discard_orphaned_profile(&self.db, profile_name).await;
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

		ClusterIdentity::amend(&cluster.folder_name, |identity| {
			identity.dedicated = dedicated;
		})
		.await;

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

	if options.dedicated {
		polyio::write(
			cluster_path.join(oneclient_common::paths::DEDICATED_MARKER),
			b"",
		)
		.await?;
	}

	ClusterIdentity {
		name: name.to_string(),
		mc_version: options.mc_version.clone(),
		mc_loader: options.mc_loader,
		mc_loader_version: options.mc_loader_version.clone(),
		dedicated: options.dedicated,
	}
	.write(folder_name)
	.await?;

	let profile = create_profile_from_global(
		db,
		global,
		folder_name,
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

async fn mark_provisioned(cluster: &Cluster) {
	let Ok(marker) = cluster.provisioned_marker() else {
		return;
	};

	if let Err(err) = polyio::write(&marker, b"").await {
		tracing::warn!(
			folder = %cluster.folder_name,
			error = %err,
			"could not mark the cluster as a default instance"
		);
	}
}

#[tracing::instrument(level = "debug", skip(db))]
async fn resolve_unique_folder_name(db: &DbPool, base: &str) -> ClusterResult<String> {
	let cluster_dir = oneclient_common::paths::clusters_dir()?;
	let mut candidate = base.to_string();
	let mut which = 1;

	loop {
		let free = !oneclient_common::paths::is_deleting_folder(&candidate)
			&& !cluster_dir.join(&candidate).exists()
			&& !profile_dao::is_reserved_global_name(&candidate)
			&& cluster_dao::get_by_folder_name(db, &candidate).await?.is_none();

		if free {
			return Ok(candidate);
		}

		candidate = format!("{base} ({which})");
		which += 1;
	}
}

fn folder_base(name: &str, fallback: &str) -> String {
	let from_name = sanitize_folder_component(name);
	if !from_name.is_empty() {
		return from_name;
	}

	let from_fallback = sanitize_folder_component(fallback);
	if !from_fallback.is_empty() {
		return from_fallback;
	}

	FOLDER_FALLBACK.to_string()
}

fn sanitize_folder_component(value: &str) -> String {
	let kept: String = value
		.chars()
		.filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')'))
		.take(MAX_FOLDER_CHARS)
		.collect();

	let mut trimmed = kept.trim().trim_end_matches(['.', ' ']).trim().to_string();

	if is_reserved_device_name(&trimmed) {
		trimmed.push('_');
	}

	trimmed
}

fn is_reserved_device_name(value: &str) -> bool {
	if value.is_empty() {
		return false;
	}

	let stem = value.split('.').next().unwrap_or(value).trim();
	WINDOWS_RESERVED_STEMS
		.iter()
		.any(|reserved| reserved.eq_ignore_ascii_case(stem))
}

#[tracing::instrument(level = "debug", skip(db))]
async fn discard_orphaned_profile(db: &DbPool, profile_name: &str) {
	if profile_dao::is_reserved_global_name(profile_name) {
		return;
	}

	match cluster_dao::list_all(db).await {
		Ok(rows) => {
			let still_used = rows
				.iter()
				.any(|row| row.setting_profile_name.as_deref() == Some(profile_name));
			if still_used {
				return;
			}
		}
		Err(err) => {
			tracing::warn!(
				profile = profile_name,
				error = %err,
				"could not confirm the profile is unused; leaving it in place"
			);
			return;
		}
	}

	if let Err(err) = delete_named_profile(db, profile_name).await {
		tracing::warn!(
			profile = profile_name,
			error = %err,
			"failed to remove the cluster's settings profile"
		);
	}
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
