use std::collections::HashMap;
use std::path::PathBuf;

use onelauncher_core::api::cluster::dao::ClusterId;
use onelauncher_core::api::packages::modpack::data::ModpackArchive;
use onelauncher_core::entity::clusters;
use onelauncher_core::error::LauncherResult;

use crate::ext::updater::Update;
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
			// Assuming `mc_version` is a String and you have a function to parse major version
			let split = &mut cluster.mc_version.split('.');
			split.next();
			if let Some(major) = split.next() {
				// Convert the major version to a u32
				let major: u32 = match major.parse() {
					Ok(v) => v,
					Err(_) => continue, // Skip if parsing fails
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
}
