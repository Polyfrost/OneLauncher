use std::collections::HashMap;
use std::path::PathBuf;

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
			let split = &mut cluster.mc_version.split('.');
			split.next();
			if let Some(major) = split.next() {
				let major: u32 = match major.parse() {
					Ok(v) => v,
					Err(_) => continue,
				};

				mapped.entry(major).or_default().push(cluster);
			}
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
}
