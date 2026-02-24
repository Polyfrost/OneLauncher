use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

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

/// Per-cluster mutex map to prevent concurrent bundle updates on the same cluster.
static CLUSTER_UPDATE_LOCKS: OnceLock<Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>> =
	OnceLock::new();

fn get_cluster_lock(cluster_id: i64) -> Arc<tokio::sync::Mutex<()>> {
	let locks = CLUSTER_UPDATE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
	let mut map = locks.lock().unwrap();
	map.entry(cluster_id)
		.or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
		.clone()
}

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
	let overrides = bundle_dao::get_bundle_overrides(cluster_id).await?;
	check_bundle_updates_inner(cluster_id, &overrides).await
}

async fn check_bundle_updates_inner(
	cluster_id: ClusterId,
	overrides: &[onelauncher_core::entity::cluster_bundle_overrides::Model],
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

	// Fetch ALL packages in the cluster (not just bundle-tracked ones) so we can
	// infer bundle subscriptions from packages installed via the regular flow.
	let all_linked_packages = api::packages::dao::get_linked_packages(&cluster).await?;
	let all_installed_package_ids: std::collections::HashSet<String> = all_linked_packages
		.iter()
		.map(|p| p.package_id.clone())
		.collect();

	tracing::debug!(
		cluster_id = %cluster_id,
		total_installed = %all_linked_packages.len(),
		"Retrieved all linked packages for subscription inference"
	);

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

	let mut bundle_versions: std::collections::HashMap<
		String,
		std::collections::HashMap<String, (String, ModpackFile)>,
	> = std::collections::HashMap::new();

	for bundle in &bundles {
		let mut enabled_count = 0;
		let mut disabled_count = 0;
		let mut files_map = std::collections::HashMap::new();
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
					files_map.insert(pkg.id.clone(), (version.version_id.clone(), file.clone()));
				}
				ModpackFileKind::External(ext) => {
					tracing::trace!(
						bundle_name = %bundle.manifest.name,
						sha1 = %ext.sha1,
						"Indexed external bundle package"
					);
					files_map.insert(ext.sha1.clone(), (ext.sha1.clone(), file.clone()));
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
		bundle_versions.insert(bundle.manifest.name.clone(), files_map);
	}

	tracing::debug!(
		total_indexed_bundles = %bundle_versions.len(),
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

		if let Some(ref bundle_name) = bundle_pkg.bundle_name {
			if let Some(files_map) = bundle_versions.get(bundle_name) {
				if let Some((new_version_id, new_file)) = files_map.get(pkg_id) {
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
				}
			} else {
				not_in_bundle += 1;
				tracing::info!(
					package_id = %pkg_id,
					package_hash = %bundle_pkg.package_hash,
					bundle_name = %bundle_name,
					"Bundle no longer exists, marking package for removal"
				);
				removals_available.push(BundlePackageRemoval {
					cluster_id,
					package_hash: bundle_pkg.package_hash.clone(),
					package_id: pkg_id.clone(),
					bundle_name: bundle_name.clone(),
					installed_at: bundle_pkg.installed_at.unwrap_or_else(Utc::now),
				});
			}
		} else {
			tracing::debug!(
				package_id = %pkg_id,
				package_hash = %bundle_pkg.package_hash,
				"Package not found in any bundle (no bundle name tracked)"
			);
		}
	}

	// Start with explicitly tracked bundles (packages installed via the bundle-aware path).
	let mut subscribed_bundles: std::collections::HashSet<String> = bundle_packages
		.iter()
		.filter_map(|bp| bp.bundle_name.clone())
		.collect();

	// Infer subscriptions: if ANY installed package's package_id matches an enabled file in a
	// bundle, treat this cluster as subscribed to that bundle. This handles the common case where
	// packages were installed via the regular flow (bundle_name never set) but are in a bundle.
	for bundle in &bundles {
		if subscribed_bundles.contains(&bundle.manifest.name) {
			continue;
		}
		let has_matching_package = bundle.manifest.files.iter().any(|f| {
			if !f.enabled {
				return false;
			}
			matches!(&f.kind, ModpackFileKind::Managed((pkg, _)) if all_installed_package_ids.contains(&pkg.id))
		});
		if has_matching_package {
			tracing::debug!(
				bundle_name = %bundle.manifest.name,
				"Inferring bundle subscription from installed package match"
			);
			subscribed_bundles.insert(bundle.manifest.name.clone());
		}
	}

	tracing::debug!(
		cluster_id = %cluster_id,
		subscribed_bundles = ?subscribed_bundles,
		"Bundles this cluster is subscribed to"
	);

	// Use ALL installed package IDs so that packages installed via the regular flow are not
	// re-proposed as additions by the bundle update check.
	let installed_package_ids = all_installed_package_ids;

	let installed_external_hashes: std::collections::HashSet<String> = bundle_packages
		.iter()
		.filter(|bp| bp.bundle_name.is_some())
		.filter(|bp| bp.package_id.as_deref() == Some(bp.package_hash.as_str()))
		.map(|bp| bp.package_hash.clone())
		.collect();

	let overrides_map: std::collections::HashMap<
		(String, String),
		onelauncher_core::entity::cluster_bundle_overrides::OverrideType,
	> = overrides
		.iter()
		.map(|o| {
			(
				(o.bundle_name.clone(), o.package_id.clone()),
				o.override_type.clone(),
			)
		})
		.collect();

	// Filter out updates for packages the user has explicitly removed.
	// Without this, apply_single_update would try to replace a package that no longer exists.
	updates_available.retain(|u| {
		let file_id = match &u.new_file.kind {
			ModpackFileKind::Managed((pkg, _)) => pkg.id.clone(),
			ModpackFileKind::External(ext) => ext.sha1.clone(),
		};
		!matches!(
			overrides_map.get(&(u.bundle_name.clone(), file_id)),
			Some(onelauncher_core::entity::cluster_bundle_overrides::OverrideType::Removed)
		)
	});

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

				// Check user overrides
				if let Some(override_type) =
					overrides_map.get(&(bundle.manifest.name.clone(), file_id.clone()))
				{
					if *override_type
						== onelauncher_core::entity::cluster_bundle_overrides::OverrideType::Removed
					{
						tracing::info!(
							bundle_name = %bundle.manifest.name,
							file_id = %file_id,
							"Skipping package addition due to user override 'Removed'"
						);
						continue;
					}
				}

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

	let overrides = bundle_dao::get_bundle_overrides(cluster_id).await?;
	let overrides_map: std::collections::HashMap<
		(String, String),
		onelauncher_core::entity::cluster_bundle_overrides::OverrideType,
	> = overrides
		.iter()
		.map(|o| {
			(
				(o.bundle_name.clone(), o.package_id.clone()),
				o.override_type.clone(),
			)
		})
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
						let key = (bundle.manifest.name.clone(), pkg.id.clone());
						if matches!(
							overrides_map.get(&key),
							Some(onelauncher_core::entity::cluster_bundle_overrides::OverrideType::Removed)
						) {
							FileUpdateStatus::RemovedByUser
						} else {
							FileUpdateStatus::NotInstalled
						}
					}
				}
				ModpackFileKind::External(ext) => {
					if bundle_packages.iter().any(|bp| bp.package_hash == ext.sha1) {
						FileUpdateStatus::UpToDate
					} else {
						let key = (bundle.manifest.name.clone(), ext.sha1.clone());
						if matches!(
							overrides_map.get(&key),
							Some(onelauncher_core::entity::cluster_bundle_overrides::OverrideType::Removed)
						) {
							FileUpdateStatus::RemovedByUser
						} else {
							FileUpdateStatus::NotInstalled
						}
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
	RemovedByUser,
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
	overrides: &[onelauncher_core::entity::cluster_bundle_overrides::Model],
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

	// Download the new version first. If it fails the old package remains untouched.
	let model = match &update.new_file.kind {
		ModpackFileKind::Managed((pkg, version)) => {
			tracing::debug!(
				package_id = %pkg.id,
				version_id = %version.version_id,
				"Downloading new managed package version"
			);
			api::packages::download_package(pkg, version, None, None).await?
		}
		ModpackFileKind::External(ext_package) => {
			tracing::debug!(
				url = %ext_package.url,
				sha1 = %ext_package.sha1,
				"Downloading new external package version"
			);
			api::packages::download_external_package(ext_package, &cluster, None, Some(true), None)
				.await?
				.ok_or_else(|| anyhow::anyhow!("Failed to download external package"))?
		}
	};

	// Link and track the new package BEFORE removing the old one.
	// If linking or tracking fails, the old package remains untouched.
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

	// Check if this package should be disabled based on user overrides (passed in from caller)
	let file_id = match &update.new_file.kind {
		ModpackFileKind::Managed((pkg, _)) => pkg.id.clone(),
		ModpackFileKind::External(ext) => ext.sha1.clone(),
	};

	let should_be_disabled = overrides.iter().any(|o| {
		o.bundle_name == update.bundle_name
			&& o.package_id == file_id
			&& o.override_type
				== onelauncher_core::entity::cluster_bundle_overrides::OverrideType::Disabled
	});

	if should_be_disabled {
		if model.file_name.ends_with(".disabled") {
			tracing::debug!(
				package_hash = %model.hash,
				bundle_name = %update.bundle_name,
				"Updated package is already disabled; skipping toggle"
			);
		} else {
			tracing::info!(
				package_hash = %model.hash,
				bundle_name = %update.bundle_name,
				"Re-applying disabled state to updated package"
			);
			// toggle_package internally disables the mod if it's currently enabled
			api::packages::toggle_package(update.cluster_id, model.hash.clone()).await?;
		}
	}

	// If the replacement resolves to the same package hash, there is nothing to remove.
	// Removing here would unlink the package we just kept/retagged.
	if model.hash == update.installed_package_hash {
		tracing::debug!(
			package_hash = %model.hash,
			"Update resolved to the same package hash; skipping old-package removal"
		);
		return Ok(model);
	}

	// Remove the old package only after the new one is fully installed and tracked.
	// Pass record_override=false: this is a system-initiated replacement, not a user removal.
	tracing::debug!(
		package_hash = %update.installed_package_hash,
		"Removing old package"
	);
	api::packages::remove_package(
		update.cluster_id,
		update.installed_package_hash.clone(),
		false,
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
	// Pass record_override=false: removal is bundle-driven (publisher dropped the package),
	// not a user choice. If the publisher re-adds it later, it should be reinstalled.
	api::packages::remove_package(removal.cluster_id, removal.package_hash.clone(), false).await?;

	tracing::info!(
		package_id = %removal.package_id,
		package_hash = %removal.package_hash,
		"Successfully removed package that was removed from bundle"
	);

	Ok(())
}

async fn apply_single_addition(
	addition: &BundlePackageAddition,
	overrides: &[onelauncher_core::entity::cluster_bundle_overrides::Model],
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

			let should_be_disabled = overrides.iter().any(|o| {
				o.bundle_name == addition.bundle_name
					&& o.package_id == file_id
					&& o.override_type == onelauncher_core::entity::cluster_bundle_overrides::OverrideType::Disabled
			});

			if should_be_disabled {
				if model.file_name.ends_with(".disabled") {
					tracing::debug!(
						package_hash = %model.hash,
						bundle_name = %addition.bundle_name,
						"Added package is already disabled; skipping toggle"
					);
				} else {
					tracing::info!(
						package_hash = %model.hash,
						bundle_name = %addition.bundle_name,
						"Applying disabled state to newly added package due to overrides"
					);
					api::packages::toggle_package(addition.cluster_id, model.hash.clone()).await?;
				}
			}

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

			let should_be_disabled = overrides.iter().any(|o| {
				o.bundle_name == addition.bundle_name
					&& o.package_id == file_id
					&& o.override_type == onelauncher_core::entity::cluster_bundle_overrides::OverrideType::Disabled
			});

			if should_be_disabled {
				if model.file_name.ends_with(".disabled") {
					tracing::debug!(
						package_hash = %model.hash,
						bundle_name = %addition.bundle_name,
						"Added external package is already disabled; skipping toggle"
					);
				} else {
					tracing::info!(
						package_hash = %model.hash,
						bundle_name = %addition.bundle_name,
						"Applying disabled state to newly added package due to overrides"
					);
					api::packages::toggle_package(addition.cluster_id, model.hash.clone()).await?;
				}
			}

			Ok(model)
		}
	}
}

#[taurpc::ipc_type]
pub struct ApplyBundleUpdatesResult {
	pub updates_applied: Vec<BundlePackageUpdate>,
	pub removals_applied: Vec<BundlePackageRemoval>,
	pub additions_applied: Vec<BundlePackageAddition>,
	pub updates_failed: Vec<String>,
	pub removals_failed: Vec<String>,
	pub additions_failed: Vec<String>,
}

pub async fn apply_bundle_updates(
	cluster_id: ClusterId,
) -> LauncherResult<ApplyBundleUpdatesResult> {
	// Hold a per-cluster lock for the duration of the update to prevent concurrent
	// apply calls on the same cluster from racing each other.
	let cluster_lock = get_cluster_lock(cluster_id);
	let _guard = cluster_lock.lock().await;

	// Fetch overrides once: shared by both the check and the apply pass.
	let cluster_overrides = api::packages::bundle_dao::get_bundle_overrides(cluster_id).await?;
	let check_result = check_bundle_updates_inner(cluster_id, &cluster_overrides).await?;

	let mut updates_applied = Vec::new();
	let mut removals_applied = Vec::new();
	let mut additions_applied = Vec::new();
	let mut updates_failed = Vec::new();
	let mut removals_failed = Vec::new();
	let mut additions_failed = Vec::new();

	for removal in check_result.removals_available {
		match apply_single_removal(&removal).await {
			Ok(_) => {
				removals_applied.push(removal);
			}
			Err(e) => {
				let msg = format!(
					"Failed to remove bundle package '{}': {}",
					removal.package_id, e
				);
				send_error!("{}", msg);
				removals_failed.push(msg);
			}
		}
	}

	for update in check_result.updates_available {
		match apply_single_update(&update, &cluster_overrides).await {
			Ok(_) => {
				updates_applied.push(update);
			}
			Err(e) => {
				let msg = format!("Failed to update bundle package: {}", e);
				send_error!("{}", msg);
				updates_failed.push(msg);
			}
		}
	}

	for addition in check_result.additions_available {
		let file_id = match &addition.new_file.kind {
			ModpackFileKind::Managed((pkg, _)) => pkg.id.clone(),
			ModpackFileKind::External(ext) => ext.sha1.clone(),
		};
		match apply_single_addition(&addition, &cluster_overrides).await {
			Ok(_) => {
				additions_applied.push(addition);
			}
			Err(e) => {
				let msg = format!("Failed to install new bundle package '{}': {}", file_id, e);
				send_error!("{}", msg);
				additions_failed.push(msg);
			}
		}
	}

	// Re-extract overrides from any bundle that had changes applied.
	// This keeps config files, resource packs, and other overridden assets in sync.
	let has_changes = !updates_applied.is_empty()
		|| !removals_applied.is_empty()
		|| !additions_applied.is_empty();

	if has_changes {
		let mut affected_bundles: std::collections::HashSet<String> =
			std::collections::HashSet::new();

		for u in &updates_applied {
			affected_bundles.insert(u.bundle_name.clone());
		}
		for r in &removals_applied {
			affected_bundles.insert(r.bundle_name.clone());
		}
		for a in &additions_applied {
			affected_bundles.insert(a.bundle_name.clone());
		}

		if !affected_bundles.is_empty() {
			match api::cluster::dao::get_cluster_by_id(cluster_id).await {
				Ok(Some(cluster)) => {
					match BundlesManager::get()
						.await
						.get_bundles_for(&cluster.mc_version, cluster.mc_loader)
						.await
					{
						Ok(bundles) => {
							for bundle in &bundles {
								if affected_bundles.contains(&bundle.manifest.name) {
									tracing::info!(
										bundle_name = %bundle.manifest.name,
										cluster_id = %cluster_id,
										"Re-extracting overrides from updated bundle"
									);
									if let Err(e) =
										onelauncher_core::api::packages::modpack::mrpack::copy_overrides_folder_no_overwrite(
											&cluster,
											&bundle.path,
										)
										.await
									{
										send_error!(
											"Failed to extract overrides from bundle '{}': {}",
											bundle.manifest.name,
											e
										);
									}
								}
							}
						}
						Err(e) => {
							tracing::error!(
								cluster_id = %cluster_id,
								"Failed to retrieve bundles for override extraction after update: {}",
								e
							);
						}
					}
				}
				Ok(None) => {
					tracing::error!(
						cluster_id = %cluster_id,
						"Cluster not found during post-update override extraction"
					);
				}
				Err(e) => {
					tracing::error!(
						cluster_id = %cluster_id,
						"Failed to retrieve cluster for override extraction after update: {}",
						e
					);
				}
			}
		}
	}

	Ok(ApplyBundleUpdatesResult {
		updates_applied,
		removals_applied,
		additions_applied,
		updates_failed,
		removals_failed,
		additions_failed,
	})
}

#[cfg(test)]
#[path = "bundle_updates_test.rs"]
mod bundle_updates_test;
