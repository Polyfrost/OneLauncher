use std::collections::HashMap;
use std::path::{Path, PathBuf};

use onelauncher_core::api;
use onelauncher_core::api::cluster::dao::ClusterId;
use onelauncher_core::api::packages::modpack::data::{ModpackArchive, ModpackFileKind};
use onelauncher_core::entity::{clusters, packages};
use onelauncher_core::error::LauncherResult;

use crate::ext::updater::Update;
use crate::oneclient::bundle_updates::ApplyBundleUpdatesResult;
use crate::oneclient::bundles::BundlesManager;
use crate::oneclient::clusters::{OnlineClusterManifest, get_data_storage_versions};
use tauri::{AppHandle, Runtime};

const BUNDLE_DIRS: &[&str] = &["mods", "resourcepacks", "shaderpacks"];

/// Recursively copy all files from `src` into `dst`, creating directories as needed.
async fn copy_dir_all(src: &Path, dst: &Path) -> onelauncher_core::error::LauncherResult<()> {
	tokio::fs::create_dir_all(dst)
		.await
		.map_err(anyhow::Error::from)?;

	let mut entries = tokio::fs::read_dir(src)
		.await
		.map_err(anyhow::Error::from)?;
	while let Some(entry) = entries.next_entry().await.map_err(anyhow::Error::from)? {
		let src_path = entry.path();
		let dst_path = dst.join(entry.file_name());
		if src_path.is_dir() {
			Box::pin(copy_dir_all(&src_path, &dst_path)).await?;
		} else {
			tokio::fs::copy(&src_path, &dst_path)
				.await
				.map_err(anyhow::Error::from)?;
		}
	}

	Ok(())
}

#[taurpc::procedures(path = "oneclient", export_to = "../frontend/src/bindings.gen.ts")]
pub trait OneClientApi {
	#[taurpc(alias = "getClustersGroupedByMajor")]
	async fn get_clusters_grouped_by_major() -> LauncherResult<HashMap<u32, Vec<clusters::Model>>>;

	#[taurpc(alias = "getBundlesFor")]
	async fn get_bundles_for(cluster_id: ClusterId) -> LauncherResult<Vec<ModpackArchive>>;

	#[taurpc(alias = "getVersions")]
	async fn get_versions() -> LauncherResult<OnlineClusterManifest>;

	#[taurpc(alias = "extractBundleOverrides")]
	async fn extract_bundle_overrides(
		bundle_path: PathBuf,
		cluster_id: ClusterId,
	) -> LauncherResult<()>;

	#[taurpc(alias = "checkForUpdate")]
	async fn check_for_update<R: Runtime>(
		app_handle: AppHandle<R>,
	) -> LauncherResult<Option<Update>>;

	#[taurpc(alias = "installUpdate")]
	async fn install_update<R: Runtime>(app_handle: AppHandle<R>) -> LauncherResult<()>;

	#[taurpc(alias = "downloadPackageFromBundle")]
	async fn download_package_from_bundle(
		package: ModpackFileKind,
		cluster_id: ClusterId,
		bundle_name: String,
		skip_compatibility: Option<bool>,
	) -> LauncherResult<packages::Model>;

	#[taurpc(alias = "updateBundlePackages")]
	async fn update_bundle_packages(
		cluster_id: ClusterId,
	) -> LauncherResult<ApplyBundleUpdatesResult>;

	#[taurpc(alias = "isBundleSyncing")]
	async fn is_bundle_syncing() -> LauncherResult<bool>;

	#[taurpc(alias = "cacheArt")]
	async fn cache_art(path: String) -> LauncherResult<String>;

	#[taurpc(alias = "refreshArt")]
	async fn refresh_art(path: String) -> LauncherResult<()>;

	#[taurpc(alias = "copyClusterContent")]
	async fn copy_cluster_content(
		source_id: ClusterId,
		target_id: ClusterId,
	) -> LauncherResult<CopyClusterResult>;
}

#[taurpc::ipc_type]
pub struct CopyClusterResult {
	/// Files that had no compatible provider version and were copied verbatim from the source.
	pub fallback_files: Vec<String>,
}

#[taurpc::ipc_type]
pub struct OneClientApiImpl;

#[taurpc::resolvers]
impl OneClientApi for OneClientApiImpl {
	async fn get_clusters_grouped_by_major(
		self,
	) -> LauncherResult<HashMap<u32, Vec<clusters::Model>>> {
		let clusters = onelauncher_core::api::cluster::dao::get_all_clusters().await?;

		let mut mapped: HashMap<u32, Vec<clusters::Model>> = HashMap::new();

		for cluster in clusters {
			let mut parts = cluster.mc_version.splitn(3, '.');
			let major: u32 = match parts.next() {
				// Old format: 1.X[.Y]
				Some("1") => match parts.next().and_then(|v| v.parse().ok()) {
					Some(v) => v,
					None => continue,
				},
				// New format: YY.N[.P]
				Some(year) => match year.parse() {
					Ok(v) => v,
					Err(_) => continue,
				},
				None => continue,
			};
			mapped.entry(major).or_default().push(cluster);
		}

		Ok(mapped)
	}

	async fn get_bundles_for(self, cluster_id: ClusterId) -> LauncherResult<Vec<ModpackArchive>> {
		let cluster = onelauncher_core::api::cluster::dao::get_cluster_by_id(cluster_id)
			.await?
			.ok_or_else(|| {
				onelauncher_core::error::LauncherError::from(anyhow::anyhow!(
					"cluster with id {} not found",
					cluster_id
				))
			})?;

		let bundles = BundlesManager::get()
			.await
			.get_bundles_for(&cluster.mc_version, cluster.mc_loader)
			.await?;

		Ok(bundles)
	}

	async fn get_versions(self) -> LauncherResult<OnlineClusterManifest> {
		get_data_storage_versions().await
	}

	async fn extract_bundle_overrides(
		self,
		bundle_path: PathBuf,
		cluster_id: ClusterId,
	) -> LauncherResult<()> {
		let cluster = onelauncher_core::api::cluster::dao::get_cluster_by_id(cluster_id)
			.await?
			.ok_or_else(|| {
				onelauncher_core::error::LauncherError::from(anyhow::anyhow!(
					"cluster with id {} not found",
					cluster_id
				))
			})?;

		onelauncher_core::api::packages::modpack::mrpack::copy_overrides_folder(
			&cluster,
			&bundle_path,
			&None,
		)
		.await?;

		Ok(())
	}

	async fn check_for_update<R: Runtime>(
		self,
		app_handle: AppHandle<R>,
	) -> LauncherResult<Option<Update>> {
		crate::ext::updater::check_for_update(app_handle)
			.await
			.map_err(|e| onelauncher_core::error::LauncherError::from(anyhow::anyhow!(e)))
	}

	async fn install_update<R: Runtime>(self, app_handle: AppHandle<R>) -> LauncherResult<()> {
		crate::ext::updater::install_update(app_handle)
			.await
			.map_err(|e| onelauncher_core::error::LauncherError::from(anyhow::anyhow!(e)))
	}

	async fn download_package_from_bundle(
		self,
		package: ModpackFileKind,
		cluster_id: ClusterId,
		bundle_name: String,
		skip_compatibility: Option<bool>,
	) -> LauncherResult<packages::Model> {
		let cluster = api::cluster::dao::get_cluster_by_id(cluster_id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", cluster_id))?;

		match package {
			ModpackFileKind::Managed(box_) => {
				let (pkg, version) = *box_;
				let model = api::packages::download_package(&pkg, &version, None, None).await?;
				api::packages::link_package(&model, &cluster, skip_compatibility).await?;
				api::packages::bundle_dao::track_bundle_package(
					&cluster,
					&model,
					&bundle_name,
					&version.version_id,
				)
				.await?;
				Ok(model)
			}

			ModpackFileKind::External(ext_package) => {
				let model = api::packages::download_external_package(
					&ext_package,
					&cluster,
					None,
					skip_compatibility.or(Some(true)),
					None,
				)
				.await?
				.ok_or_else(|| anyhow::anyhow!("Failed to download external package"))?;

				api::packages::link_package(&model, &cluster, skip_compatibility.or(Some(true)))
					.await?;

				api::packages::bundle_dao::track_bundle_package(
					&cluster,
					&model,
					&bundle_name,
					&ext_package.sha1,
				)
				.await?;

				Ok(model)
			}
		}
	}

	async fn update_bundle_packages(
		self,
		cluster_id: ClusterId,
	) -> LauncherResult<ApplyBundleUpdatesResult> {
		crate::oneclient::bundle_updates::apply_bundle_updates(cluster_id).await
	}

	async fn is_bundle_syncing(self) -> LauncherResult<bool> {
		Ok(crate::oneclient::is_bundle_syncing())
	}

	async fn cache_art(self, path: String) -> LauncherResult<String> {
		crate::oneclient::clusters::cache_art_image(&path).await
	}

	async fn refresh_art(self, path: String) -> LauncherResult<()> {
		tokio::spawn(async move {
			crate::oneclient::clusters::refresh_art_cache(&path).await;
		});
		Ok(())
	}

	async fn copy_cluster_content(
		self,
		source_id: ClusterId,
		target_id: ClusterId,
	) -> LauncherResult<CopyClusterResult> {
		let source = api::cluster::dao::get_cluster_by_id(source_id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("source cluster {} not found", source_id))?;
		let target = api::cluster::dao::get_cluster_by_id(target_id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("target cluster {} not found", target_id))?;

		let clusters_dir = onelauncher_core::store::Dirs::get_clusters_dir().await?;
		let source_path = clusters_dir.join(&source.folder_name);
		let target_path = clusters_dir.join(&target.folder_name);

		// Mods, resource packs, and shader packs are version-dependent and bundle-managed.
		// Detect the source cluster's bundle subscriptions, map them to the target version,
		// copy overrides, and install the correctly-versioned packages into the target.
		crate::oneclient::bundle_updates::migrate_bundles_to_cluster(source_id, target_id).await?;

		// Migrate non-bundle (custom) packages: update via provider for the target MC version
		// where possible, and get back a list of files that had no compatible version and
		// need to be copied verbatim.
		let fallback_files: std::collections::HashSet<String> =
			crate::oneclient::bundle_updates::migrate_non_bundle_packages(source_id, target_id)
				.await?
				.into_iter()
				.collect();

		// File-copy everything from the source folder. For bundle-managed dirs, only copy
		// files in the fallback list (everything else was handled version-correctly above).
		if source_path.exists() {
			let mut entries = tokio::fs::read_dir(&source_path)
				.await
				.map_err(anyhow::Error::from)?;
			while let Some(entry) = entries.next_entry().await.map_err(anyhow::Error::from)? {
				let name = entry.file_name();
				let name_str = name.to_string_lossy();
				let src = entry.path();
				let dst = target_path.join(&name);
				if BUNDLE_DIRS.contains(&name_str.as_ref()) {
					// Only copy files that weren't handled by migrate_non_bundle_packages
					// or migrate_bundles_to_cluster (i.e., provider lookup failed).
					if src.is_dir() {
						let mut sub = tokio::fs::read_dir(&src)
							.await
							.map_err(anyhow::Error::from)?;
						while let Some(sub_entry) =
							sub.next_entry().await.map_err(anyhow::Error::from)?
						{
							let sub_name = sub_entry.file_name();
							if !fallback_files.contains(sub_name.to_string_lossy().as_ref()) {
								continue;
							}
							tokio::fs::create_dir_all(&dst)
								.await
								.map_err(anyhow::Error::from)?;
							let sub_src = sub_entry.path();
							// Copy as disabled — no compatible version exists for the new MC
							// version, so mark it disabled rather than silently breaking the game.
							let disabled_name = {
								let s = sub_name.to_string_lossy();
								if s.ends_with(".disabled") {
									sub_name.clone()
								} else {
									std::ffi::OsString::from(format!("{s}.disabled"))
								}
							};
							let sub_dst = dst.join(&disabled_name);
							if sub_src.is_dir() {
								copy_dir_all(&sub_src, &sub_dst).await?;
							} else {
								tokio::fs::copy(&sub_src, &sub_dst)
									.await
									.map_err(anyhow::Error::from)?;
							}
						}
					}
				} else if src.is_dir() {
					copy_dir_all(&src, &dst).await?;
				} else {
					tokio::fs::copy(&src, &dst)
						.await
						.map_err(anyhow::Error::from)?;
				}
			}
		}

		onelauncher_core::api::cluster::sync_cluster_by_id(target_id).await?;

		// Stamp the target cluster's created_at with the actual migration time so the
		// home page can correctly identify it as a newly-migrated cluster.
		onelauncher_core::api::cluster::dao::touch_cluster_created_at(target_id).await?;

		Ok(CopyClusterResult {
			fallback_files: fallback_files.into_iter().collect(),
		})
	}
}
