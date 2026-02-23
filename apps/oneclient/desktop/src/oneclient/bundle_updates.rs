use chrono::{DateTime, Utc};
use onelauncher_core::api::cluster::dao::ClusterId;
use onelauncher_core::api::packages::bundle_dao;
use onelauncher_core::api::packages::modpack::data::{
	ModpackArchive, ModpackFile, ModpackFileKind,
};
use onelauncher_core::entity::cluster_packages;
use onelauncher_core::error::LauncherResult;
use onelauncher_core::{api, send_error};

use crate::oneclient::bundles::BundlesManager;

#[taurpc::ipc_type]
pub struct BundlePackageUpdate {
	pub cluster_id: ClusterId,
	pub installed_package_hash: String,
	pub installed_version_id: String,
	pub bundle_name: String,
	pub new_version_id: String,
	pub new_file: ModpackFile,
	pub installed_at: DateTime<Utc>,
}

#[taurpc::ipc_type]
pub struct BundlePackageRemoval {
	pub cluster_id: ClusterId,
	pub package_hash: String,
	pub package_id: String,
	pub bundle_name: String,
	pub installed_at: DateTime<Utc>,
}

#[taurpc::ipc_type]
pub struct BundlePackageAddition {
	pub cluster_id: ClusterId,
	pub bundle_name: String,
	pub new_file: ModpackFile,
}

#[taurpc::ipc_type]
pub struct BundleUpdateCheckResult {
	pub cluster_id: ClusterId,
	pub updates_available: Vec<BundlePackageUpdate>,
	pub removals_available: Vec<BundlePackageRemoval>,
	pub additions_available: Vec<BundlePackageAddition>,
	pub checked_at: DateTime<Utc>,
}

pub async fn check_bundle_updates(
	cluster_id: ClusterId,
) -> LauncherResult<BundleUpdateCheckResult> {
	tracing::debug!(cluster_id = %cluster_id, "Starting bundle update check");

	let cluster = onelauncher_core::api::cluster::dao::get_cluster_by_id(cluster_id)
		.await?
		.ok_or_else(|| {
			onelauncher_core::error::LauncherError::from(anyhow::anyhow!(
				"cluster with id {} not found",
				cluster_id
			))
		})?;

	tracing::debug!(
		cluster_id = %cluster_id,
		cluster_name = %cluster.name,
		mc_version = %cluster.mc_version,
		mc_loader = ?cluster.mc_loader,
		"Found cluster for update check"
	);

	let bundle_packages = bundle_dao::get_bundle_packages_for_cluster(cluster_id).await?;

	tracing::debug!(
		cluster_id = %cluster_id,
		package_count = %bundle_packages.len(),
		"Retrieved bundle packages from database"
	);

	if bundle_packages.is_empty() {
		tracing::debug!(cluster_id = %cluster_id, "No bundle packages found, checking for new additions only");
	}

	let bundles = BundlesManager::get()
		.await
		.get_bundles_for(&cluster.mc_version, cluster.mc_loader)
		.await?;

	tracing::debug!(
		cluster_id = %cluster_id,
		bundle_count = %bundles.len(),
		bundle_names = ?bundles.iter().map(|b| &b.manifest.name).collect::<Vec<_>>(),
		"Retrieved available bundles"
	);

	let mut bundle_versions: std::collections::HashMap<String, (String, String, ModpackFile)> =
		std::collections::HashMap::new();

	for bundle in &bundles {
		let mut enabled_count = 0;
		let mut disabled_count = 0;
		for file in &bundle.manifest.files {
			if !file.enabled {
				disabled_count += 1;
				continue;
			}
			enabled_count += 1;
			match &file.kind {
				ModpackFileKind::Managed((pkg, version)) => {
					tracing::trace!(
						bundle_name = %bundle.manifest.name,
						package_id = %pkg.id,
						version_id = %version.version_id,
						"Indexed managed bundle package"
					);
					bundle_versions.insert(
						pkg.id.clone(),
						(
							bundle.manifest.name.clone(),
							version.version_id.clone(),
							file.clone(),
						),
					);
				}
				ModpackFileKind::External(ext) => {
					tracing::trace!(
						bundle_name = %bundle.manifest.name,
						sha1 = %ext.sha1,
						"Indexed external bundle package"
					);
					bundle_versions.insert(
						ext.sha1.clone(),
						(bundle.manifest.name.clone(), ext.sha1.clone(), file.clone()),
					);
				}
			}
		}
		tracing::debug!(
			bundle_name = %bundle.manifest.name,
			enabled_packages = %enabled_count,
			disabled_packages = %disabled_count,
			total_files = %bundle.manifest.files.len(),
			"Indexed bundle"
		);
	}

	tracing::debug!(
		total_indexed_packages = %bundle_versions.len(),
		"Finished indexing all bundle versions"
	);

	let mut updates_available = Vec::new();
	let mut removals_available = Vec::new();
	let mut skipped_no_package_id = 0;
	let mut skipped_no_version_id = 0;
	let mut not_in_bundle = 0;

	for bundle_pkg in &bundle_packages {
		let Some(ref pkg_id) = bundle_pkg.package_id else {
			skipped_no_package_id += 1;
			tracing::debug!(
				package_hash = %bundle_pkg.package_hash,
				"Skipping package: missing package_id"
			);
			continue;
		};
		let Some(ref installed_version_id) = bundle_pkg.bundle_version_id else {
			skipped_no_version_id += 1;
			tracing::debug!(
				package_hash = %bundle_pkg.package_hash,
				package_id = %pkg_id,
				"Skipping package: missing bundle_version_id"
			);
			continue;
		};

		if let Some((bundle_name, new_version_id, new_file)) = bundle_versions.get(pkg_id) {
			tracing::debug!(
				package_id = %pkg_id,
				installed_version = %installed_version_id,
				bundle_version = %new_version_id,
				bundle_name = %bundle_name,
				"Checking bundle package for updates"
			);

			if installed_version_id != new_version_id {
				tracing::info!(
					package_id = %pkg_id,
					installed_version = %installed_version_id,
					bundle_version = %new_version_id,
					bundle_name = %bundle_name,
					"Update available for bundle package"
				);
				updates_available.push(BundlePackageUpdate {
					cluster_id,
					installed_package_hash: bundle_pkg.package_hash.clone(),
					installed_version_id: installed_version_id.clone(),
					bundle_name: bundle_name.clone(),
					new_version_id: new_version_id.clone(),
					new_file: new_file.clone(),
					installed_at: bundle_pkg.installed_at.unwrap_or_else(Utc::now),
				});
			} else {
				tracing::debug!(
					package_id = %pkg_id,
					version = %installed_version_id,
					"Bundle package is up to date"
				);
			}
		} else {
			not_in_bundle += 1;
			if let Some(ref bundle_name) = bundle_pkg.bundle_name {
				tracing::info!(
					package_id = %pkg_id,
					package_hash = %bundle_pkg.package_hash,
					bundle_name = %bundle_name,
					"Package no longer in bundle, marking for removal"
				);
				removals_available.push(BundlePackageRemoval {
					cluster_id,
					package_hash: bundle_pkg.package_hash.clone(),
					package_id: pkg_id.clone(),
					bundle_name: bundle_name.clone(),
					installed_at: bundle_pkg.installed_at.unwrap_or_else(Utc::now),
				});
			} else {
				tracing::debug!(
					package_id = %pkg_id,
					package_hash = %bundle_pkg.package_hash,
					"Package not found in any bundle (no bundle name tracked)"
				);
			}
		}
	}

	let subscribed_bundles: std::collections::HashSet<String> = bundle_packages
		.iter()
		.filter_map(|bp| bp.bundle_name.clone())
		.collect();

	tracing::debug!(
		cluster_id = %cluster_id,
		subscribed_bundles = ?subscribed_bundles,
		"Bundles this cluster is subscribed to"
	);

	let installed_package_ids: std::collections::HashSet<String> = bundle_packages
		.iter()
		.filter_map(|bp| bp.package_id.clone())
		.collect();

	let installed_external_hashes: std::collections::HashSet<String> = bundle_packages
		.iter()
		.filter(|bp| bp.bundle_name.is_some())
		.map(|bp| bp.package_hash.clone())
		.collect();

	let mut additions_available = Vec::new();
	for bundle in &bundles {
		if !subscribed_bundles.contains(&bundle.manifest.name) {
			tracing::debug!(
				bundle_name = %bundle.manifest.name,
				"Skipping bundle - not subscribed"
			);
			continue;
		}

		for file in &bundle.manifest.files {
			if !file.enabled {
				continue;
			}

			let is_new = match &file.kind {
				ModpackFileKind::Managed((pkg, _)) => !installed_package_ids.contains(&pkg.id),
				ModpackFileKind::External(ext) => !installed_external_hashes.contains(&ext.sha1),
			};

			if is_new {
				let file_id = match &file.kind {
					ModpackFileKind::Managed((pkg, _)) => pkg.id.clone(),
					ModpackFileKind::External(ext) => ext.sha1.clone(),
				};
				tracing::info!(
					bundle_name = %bundle.manifest.name,
					file_id = %file_id,
					"New package found in subscribed bundle, marking for addition"
				);
				additions_available.push(BundlePackageAddition {
					cluster_id,
					bundle_name: bundle.manifest.name.clone(),
					new_file: file.clone(),
				});
			}
		}
	}

	tracing::info!(
		cluster_id = %cluster_id,
		total_packages_checked = %bundle_packages.len(),
		updates_found = %updates_available.len(),
		removals_found = %removals_available.len(),
		additions_found = %additions_available.len(),
		skipped_no_package_id = %skipped_no_package_id,
		skipped_no_version_id = %skipped_no_version_id,
		not_in_bundle = %not_in_bundle,
		"Bundle update check completed"
	);

	Ok(BundleUpdateCheckResult {
		cluster_id,
		updates_available,
		removals_available,
		additions_available,
		checked_at: Utc::now(),
	})
}

pub async fn get_bundles_with_update_status(
	cluster_id: ClusterId,
) -> LauncherResult<Vec<BundleWithUpdateStatus>> {
	let cluster = onelauncher_core::api::cluster::dao::get_cluster_by_id(cluster_id)
		.await?
		.ok_or_else(|| {
			onelauncher_core::error::LauncherError::from(anyhow::anyhow!(
				"cluster with id {} not found",
				cluster_id
			))
		})?;

	let bundle_packages = bundle_dao::get_bundle_packages_for_cluster(cluster_id).await?;

	let installed_map: std::collections::HashMap<String, &cluster_packages::Model> =
		bundle_packages
			.iter()
			.filter_map(|bp| bp.package_id.as_ref().map(|pid| (pid.clone(), bp)))
			.collect();

	let bundles = BundlesManager::get()
		.await
		.get_bundles_for(&cluster.mc_version, cluster.mc_loader)
		.await?;

	let mut results = Vec::new();

	for bundle in bundles {
		let mut files_with_status = Vec::new();
		let mut has_updates = false;

		for file in &bundle.manifest.files {
			let update_status = match &file.kind {
				ModpackFileKind::Managed((pkg, version)) => {
					if let Some(installed) = installed_map.get(&pkg.id) {
						let installed_version =
							installed.bundle_version_id.as_deref().unwrap_or("");
						if installed_version != version.version_id {
							has_updates = true;
							FileUpdateStatus::UpdateAvailable {
								installed_version_id: installed_version.to_string(),
								new_version_id: version.version_id.clone(),
							}
						} else {
							FileUpdateStatus::UpToDate
						}
					} else {
						FileUpdateStatus::NotInstalled
					}
				}
				ModpackFileKind::External(ext) => {
					if bundle_packages.iter().any(|bp| bp.package_hash == ext.sha1) {
						FileUpdateStatus::UpToDate
					} else {
						FileUpdateStatus::NotInstalled
					}
				}
			};

			files_with_status.push(FileWithUpdateStatus {
				file: file.clone(),
				status: update_status,
			});
		}

		results.push(BundleWithUpdateStatus {
			bundle,
			files: files_with_status,
			has_updates,
		});
	}

	Ok(results)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FileUpdateStatus {
	NotInstalled,
	UpToDate,
	UpdateAvailable {
		installed_version_id: String,
		new_version_id: String,
	},
}

#[taurpc::ipc_type]
pub struct FileWithUpdateStatus {
	pub file: ModpackFile,
	pub status: FileUpdateStatus,
}

#[taurpc::ipc_type]
pub struct BundleWithUpdateStatus {
	pub bundle: ModpackArchive,
	pub files: Vec<FileWithUpdateStatus>,
	pub has_updates: bool,
}

async fn apply_single_update(
	update: &BundlePackageUpdate,
) -> LauncherResult<onelauncher_core::entity::packages::Model> {
	tracing::info!(
		cluster_id = %update.cluster_id,
		package_hash = %update.installed_package_hash,
		bundle_name = %update.bundle_name,
		old_version = %update.installed_version_id,
		new_version = %update.new_version_id,
		"Applying bundle package update"
	);

	let cluster = api::cluster::dao::get_cluster_by_id(update.cluster_id)
		.await?
		.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", update.cluster_id))?;

	// Download the new version FIRST, before removing the old one.
	// This way, if the download fails, the old package remains untouched.
	let model = match &update.new_file.kind {
		ModpackFileKind::Managed((pkg, version)) => {
			tracing::debug!(
				package_id = %pkg.id,
				version_id = %version.version_id,
				"Downloading new managed package version (before removing old)"
			);
			api::packages::download_package(pkg, version, None, None).await?
		}
		ModpackFileKind::External(ext_package) => {
			tracing::debug!(
				url = %ext_package.url,
				sha1 = %ext_package.sha1,
				"Downloading new external package version (before removing old)"
			);
			api::packages::download_external_package(ext_package, &cluster, None, Some(true), None)
				.await?
				.ok_or_else(|| anyhow::anyhow!("Failed to download external package"))?
		}
	};

	// Now remove the old package (safe — we already have the new one downloaded)
	tracing::debug!(
		package_hash = %update.installed_package_hash,
		"Removing old package"
	);
	api::packages::remove_package(update.cluster_id, update.installed_package_hash.clone()).await?;

	// Link and track the new package
	tracing::debug!(
		package_hash = %model.hash,
		"Linking new package to cluster"
	);
	api::packages::link_package(&model, &cluster, Some(true)).await?;

	let version_id = match &update.new_file.kind {
		ModpackFileKind::Managed((_, version)) => version.version_id.clone(),
		ModpackFileKind::External(ext) => ext.sha1.clone(),
	};

	tracing::debug!(
		package_hash = %model.hash,
		bundle_name = %update.bundle_name,
		"Tracking new package as bundle package"
	);
	api::packages::bundle_dao::track_bundle_package(
		&cluster,
		&model,
		&update.bundle_name,
		&version_id,
	)
	.await?;

	tracing::info!(
		new_hash = %model.hash,
		"Successfully updated package"
	);
	Ok(model)
}

async fn apply_single_removal(removal: &BundlePackageRemoval) -> LauncherResult<()> {
	tracing::info!(
		cluster_id = %removal.cluster_id,
		package_hash = %removal.package_hash,
		package_id = %removal.package_id,
		bundle_name = %removal.bundle_name,
		"Removing package that was removed from bundle"
	);

	tracing::debug!(
		package_hash = %removal.package_hash,
		"Removing package from cluster"
	);
	api::packages::remove_package(removal.cluster_id, removal.package_hash.clone()).await?;

	tracing::info!(
		package_id = %removal.package_id,
		package_hash = %removal.package_hash,
		"Successfully removed package that was removed from bundle"
	);

	Ok(())
}

async fn apply_single_addition(
	addition: &BundlePackageAddition,
) -> LauncherResult<onelauncher_core::entity::packages::Model> {
	let file_id = match &addition.new_file.kind {
		ModpackFileKind::Managed((pkg, _)) => pkg.id.clone(),
		ModpackFileKind::External(ext) => ext.sha1.clone(),
	};

	tracing::info!(
		cluster_id = %addition.cluster_id,
		bundle_name = %addition.bundle_name,
		file_id = %file_id,
		"Installing new package from bundle"
	);

	let cluster = api::cluster::dao::get_cluster_by_id(addition.cluster_id)
		.await?
		.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", addition.cluster_id))?;

	match &addition.new_file.kind {
		ModpackFileKind::Managed((pkg, version)) => {
			tracing::debug!(
				package_id = %pkg.id,
				version_id = %version.version_id,
				"Downloading new managed package"
			);
			let model = api::packages::download_package(pkg, version, None, None).await?;

			tracing::debug!(
				package_hash = %model.hash,
				"Linking new package to cluster"
			);
			api::packages::link_package(&model, &cluster, Some(true)).await?;

			tracing::debug!(
				package_hash = %model.hash,
				bundle_name = %addition.bundle_name,
				"Tracking new package as bundle package"
			);
			api::packages::bundle_dao::track_bundle_package(
				&cluster,
				&model,
				&addition.bundle_name,
				&version.version_id,
			)
			.await?;

			tracing::info!(
				package_id = %pkg.id,
				package_hash = %model.hash,
				"Successfully installed new managed package from bundle"
			);
			Ok(model)
		}
		ModpackFileKind::External(ext_package) => {
			tracing::debug!(
				url = %ext_package.url,
				sha1 = %ext_package.sha1,
				"Downloading new external package"
			);
			let model = api::packages::download_external_package(
				ext_package,
				&cluster,
				None,
				Some(true),
				None,
			)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Failed to download external package"))?;

			tracing::debug!(
				package_hash = %model.hash,
				"Linking new external package to cluster"
			);
			api::packages::link_package(&model, &cluster, Some(true)).await?;

			tracing::debug!(
				package_hash = %model.hash,
				bundle_name = %addition.bundle_name,
				"Tracking new external package as bundle package"
			);
			api::packages::bundle_dao::track_bundle_package(
				&cluster,
				&model,
				&addition.bundle_name,
				&ext_package.sha1,
			)
			.await?;

			tracing::info!(
				url = %ext_package.url,
				package_hash = %model.hash,
				"Successfully installed new external package from bundle"
			);
			Ok(model)
		}
	}
}

#[taurpc::ipc_type]
pub struct ApplyBundleUpdatesResult {
	pub updates_applied: Vec<BundlePackageUpdate>,
	pub removals_applied: Vec<BundlePackageRemoval>,
	pub additions_applied: Vec<BundlePackageAddition>,
}

pub async fn apply_bundle_updates(
	cluster_id: ClusterId,
) -> LauncherResult<ApplyBundleUpdatesResult> {
	let check_result = check_bundle_updates(cluster_id).await?;

	let mut updates_applied = Vec::new();
	let mut removals_applied = Vec::new();
	let mut additions_applied = Vec::new();

	for removal in check_result.removals_available {
		match apply_single_removal(&removal).await {
			Ok(_) => {
				removals_applied.push(removal);
			}
			Err(e) => {
				send_error!(
					"Failed to remove bundle package '{}': {}",
					removal.package_id,
					e
				);
			}
		}
	}

	for update in check_result.updates_available {
		match apply_single_update(&update).await {
			Ok(_) => {
				updates_applied.push(update);
			}
			Err(e) => {
				send_error!("Failed to update bundle package: {}", e);
			}
		}
	}

	for addition in check_result.additions_available {
		let file_id = match &addition.new_file.kind {
			ModpackFileKind::Managed((pkg, _)) => pkg.id.clone(),
			ModpackFileKind::External(ext) => ext.sha1.clone(),
		};
		match apply_single_addition(&addition).await {
			Ok(_) => {
				additions_applied.push(addition);
			}
			Err(e) => {
				send_error!("Failed to install new bundle package '{}': {}", file_id, e);
			}
		}
	}

	Ok(ApplyBundleUpdatesResult {
		updates_applied,
		removals_applied,
		additions_applied,
	})
}
