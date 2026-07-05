use std::path::{Path, PathBuf};

use oneclient_db::dao::{
	artifact as artifact_dao, cluster as cluster_dao, cluster_bundle as cluster_bundle_dao,
};
use oneclient_db::models::NewCluster;
use oneclient_db::DbPool;
use strum::IntoEnumIterator;
use tracing::instrument;

use crate::clusters::{ClusterManager, ClusterStage};
use crate::crypto::sha1_file;
use crate::packages::domain::{ContentType, GameLoader, ProviderId};
use crate::paths;
use crate::settings::store::create_profile_from_global;
use crate::state::LauncherState;
use crate::version::parse_mc_version;
use crate::LauncherResult;

const FILE_CONTENT_TYPES: [ContentType; 4] = [
	ContentType::Mod,
	ContentType::ResourcePack,
	ContentType::Shader,
	ContentType::DataPack,
];

#[derive(Debug, Default, Clone, Copy)]
pub struct RecoveryReport {
	pub adopted_clusters: usize,
	pub indexed_artifacts: usize,
	pub relinked_files: usize,
}

impl RecoveryReport {
	pub fn did_recover(&self) -> bool {
		self.adopted_clusters > 0
	}
}

#[instrument(skip(state))]
pub async fn reconstruct_from_disk(state: &LauncherState) -> LauncherResult<RecoveryReport> {
	let mut report = RecoveryReport::default();

	let clusters_dir = paths::clusters_dir()?;
	let orphans = orphan_cluster_folders(&state.services.db, &clusters_dir).await?;
	if orphans.is_empty() {
		return Ok(report);
	}

	tracing::info!(
		orphan_folders = orphans.len(),
		"user_data.db appears reset; reconstructing from disk"
	);

	if let Err(err) = crate::java::JavaManager::rescan(&state.services.db).await {
		tracing::warn!("java rescan during recovery failed: {err:#}");
	}

	report.indexed_artifacts = rebuild_artifact_cache(&state.services.db).await?;

	for folder in orphans {
		match adopt_cluster(state, &folder, &mut report).await {
			Ok(()) => {}
			Err(err) => {
				tracing::warn!(folder = %folder, "failed to adopt cluster folder: {err:#}");
			}
		}
	}

	tracing::info!(
		adopted = report.adopted_clusters,
		artifacts = report.indexed_artifacts,
		files = report.relinked_files,
		"database reconstruction complete"
	);

	Ok(report)
}

async fn orphan_cluster_folders(
	pool: &DbPool,
	clusters_dir: &Path,
) -> LauncherResult<Vec<String>> {
	let mut orphans = Vec::new();
	for name in list_dir_names(clusters_dir).await? {
		if cluster_dao::get_by_folder_name(pool, &name).await?.is_none() {
			orphans.push(name);
		}
	}
	Ok(orphans)
}

async fn rebuild_artifact_cache(pool: &DbPool) -> LauncherResult<usize> {
	let cache_root = paths::packages_cache_dir()?;
	if !cache_root.exists() {
		return Ok(0);
	}

	let launcher_dir = paths::launcher_dir()?;
	let mut indexed = 0;

	for content_name in list_dir_names(&cache_root).await? {
		let Some(content_type) = ContentType::from_folder_name(&content_name) else {
			continue;
		};
		let content_dir = cache_root.join(&content_name);

		for provider_name in list_dir_names(&content_dir).await? {
			let Some(provider) = provider_from_dir_name(&provider_name) else {
				continue;
			};
			let provider_dir = content_dir.join(&provider_name);

			for project_id in list_dir_names(&provider_dir).await? {
				let project_dir = provider_dir.join(&project_id);

				for version_id in list_dir_names(&project_dir).await? {
					let version_dir = project_dir.join(&version_id);

					for file in list_files(&version_dir).await? {
						if let Err(err) = index_cache_file(
							pool,
							launcher_dir,
							content_type,
							provider,
							&project_id,
							&version_id,
							&file,
						)
						.await
						{
							tracing::warn!(path = %file.display(), "failed to index cached file: {err:#}");
							continue;
						}
						indexed += 1;
					}
				}
			}
		}
	}

	Ok(indexed)
}

#[allow(clippy::too_many_arguments)]
async fn index_cache_file(
	pool: &DbPool,
	launcher_dir: &Path,
	content_type: ContentType,
	provider: ProviderId,
	project_id: &str,
	version_id: &str,
	file: &Path,
) -> LauncherResult<()> {
	let hash = sha1_file(file).await?;
	let stored_path = relative_launcher_path(launcher_dir, file);
	let file_name = file
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default();
	let size = tokio::fs::metadata(file).await.ok().map(|m| m.len() as i64);

	artifact_dao::insert_artifact(
		pool,
		&hash,
		content_type as i64,
		&stored_path,
		&file_name,
		size,
	)
	.await?;

	if matches!(provider, ProviderId::Modrinth | ProviderId::CurseForge) {
		artifact_dao::upsert_provider_release(
			pool,
			provider as i64,
			project_id,
			version_id,
			&hash,
			&file_name,
			version_id,
			None,
			"[]",
			"[]",
		)
		.await?;
	}

	Ok(())
}

async fn adopt_cluster(
	state: &LauncherState,
	folder_name: &str,
	report: &mut RecoveryReport,
) -> LauncherResult<()> {
	let (name, mc_version, loader) = parse_folder_identity(folder_name);

	let settings = state.settings.read().clone();
	let profile = create_profile_from_global(&state.services.db, &settings, &name, None, None).await?;

	let row = cluster_dao::insert(
		&state.services.db,
		&NewCluster {
			name: &name,
			folder_name,
			mc_version: &mc_version,
			mc_loader: loader as i64,
			mc_loader_version: None,
			setting_profile_name: Some(&profile.name),
			stage: ClusterStage::NotReady as i64,
		},
	)
	.await?;

	report.adopted_clusters += 1;
	tracing::info!(
		cluster_id = row.id,
		folder = folder_name,
		mc_version = %mc_version,
		loader = %loader,
		"adopted cluster folder from disk"
	);

	relink_cluster_files(state, row.id, folder_name, report).await?;
	Ok(())
}

async fn relink_cluster_files(
	state: &LauncherState,
	cluster_id: i64,
	folder_name: &str,
	report: &mut RecoveryReport,
) -> LauncherResult<()> {
	let pool = &state.services.db;
	let launcher_dir = paths::launcher_dir()?;
	let cluster_root = paths::clusters_dir()?.join(folder_name);

	for content_type in FILE_CONTENT_TYPES {
		let dir = cluster_root.join(content_type.folder_name());
		let files = match list_files(&dir).await {
			Ok(files) => files,
			Err(_) => continue,
		};

		for file in files {
			let file_name = file
				.file_name()
				.map(|n| n.to_string_lossy().into_owned())
				.unwrap_or_default();
			let enabled = !file_name.ends_with(".disabled");
			let base_name = file_name.trim_end_matches(".disabled").to_string();

			let hash = match sha1_file(&file).await {
				Ok(hash) => hash,
				Err(err) => {
					tracing::warn!(path = %file.display(), "failed to hash cluster file: {err:#}");
					continue;
				}
			};

			if artifact_dao::get_artifact_by_hash(pool, &hash).await?.is_none() {
				let stored_path = relative_launcher_path(launcher_dir, &file);
				let size = tokio::fs::metadata(&file).await.ok().map(|m| m.len() as i64);
				artifact_dao::insert_artifact(
					pool,
					&hash,
					content_type as i64,
					&stored_path,
					&base_name,
					size,
				)
				.await?;
			}

			artifact_dao::link_cluster_artifact(pool, cluster_id, &hash, &base_name).await?;
			if !enabled {
				artifact_dao::update_cluster_artifact(pool, cluster_id, &hash, &base_name, 0).await?;
			}
			report.relinked_files += 1;
		}
	}

	Ok(())
}

#[instrument(skip(state))]
pub async fn restore_bundle_tracking(state: &LauncherState) -> LauncherResult<()> {
	let clusters = ClusterManager::list(state).await?;

	for cluster in clusters {
		let archives = match state
			.bundles
			.archives_for(&state.services, &cluster.mc_version, cluster.mc_loader)
			.await
		{
			Ok(archives) => archives,
			Err(err) => {
				tracing::warn!(
					cluster_id = cluster.id,
					"failed to load bundle archives during recovery: {err:#}"
				);
				continue;
			}
		};

		for archive in archives {
			let bundle_name = &archive.manifest.name;
			for file in &archive.manifest.files {
				use crate::bundles::BundleFileKind;
				let BundleFileKind::Managed {
					provider,
					project_id,
					version_id,
				} = &file.kind
				else {
					continue;
				};

				let Some(release) = artifact_dao::get_provider_release(
					&state.services.db,
					*provider as i64,
					project_id,
					version_id,
				)
				.await?
				else {
					continue;
				};

				if artifact_dao::is_cluster_linked(&state.services.db, cluster.id, &release.hash)
					.await?
				{
					cluster_bundle_dao::track_bundle_artifact(
						&state.services.db,
						cluster.id,
						&release.hash,
						bundle_name,
						version_id,
						project_id,
					)
					.await?;
				}
			}
		}
	}

	Ok(())
}

fn parse_folder_identity(folder_name: &str) -> (String, String, GameLoader) {
	let tokens: Vec<&str> = folder_name.split_whitespace().collect();

	for take in [2usize, 1] {
		if tokens.len() > take {
			let split = tokens.len() - take;
			let loader_str = tokens[split..].join(" ");
			if let Ok(loader) = loader_str.parse::<GameLoader>() {
				let version = tokens[..split].join(" ");
				if parse_mc_version(&version).is_some() {
					return (folder_name.to_string(), version, loader);
				}
			}
		}
	}

	let mc_version = parse_mc_version(folder_name)
		.map(|_| folder_name.to_string())
		.unwrap_or_else(|| "unknown".to_string());
	(folder_name.to_string(), mc_version, GameLoader::Vanilla)
}

fn provider_from_dir_name(name: &str) -> Option<ProviderId> {
	ProviderId::iter().find(|p| p.dir_name() == name)
}

fn relative_launcher_path(launcher_dir: &Path, abs: &Path) -> String {
	abs.strip_prefix(launcher_dir)
		.unwrap_or(abs)
		.to_string_lossy()
		.replace('\\', "/")
}

async fn list_dir_names(dir: &Path) -> LauncherResult<Vec<String>> {
	let mut names = Vec::new();
	let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
		return Ok(names);
	};
	while let Some(entry) = entries.next_entry().await? {
		if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
			names.push(entry.file_name().to_string_lossy().into_owned());
		}
	}
	Ok(names)
}

async fn list_files(dir: &Path) -> LauncherResult<Vec<PathBuf>> {
	let mut files = Vec::new();
	let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
		return Ok(files);
	};
	while let Some(entry) = entries.next_entry().await? {
		if entry.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
			files.push(entry.path());
		}
	}
	Ok(files)
}
