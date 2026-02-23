pub mod bundle_updates;
pub mod bundles;
pub mod clusters;

use std::sync::atomic::{AtomicBool, Ordering};

use onelauncher_core::api::cluster::dao::get_all_clusters;
use onelauncher_core::send_info;

/// Global flag indicating whether a bundle sync is currently in progress.
/// Used by the frontend FS watcher to skip `syncCluster` calls during updates.
static BUNDLE_SYNCING: AtomicBool = AtomicBool::new(false);

pub fn is_bundle_syncing() -> bool {
	BUNDLE_SYNCING.load(Ordering::Relaxed)
}

pub async fn initialize_oneclient() {
	if let Err(err) = clusters::init_clusters().await {
		tracing::error!("failed to initialize clusters: {err}");
	}

	if let Err(err) = onelauncher_core::api::cluster::sync_clusters().await {
		tracing::error!("failed to sync clusters: {err}");
	}

	bundles::BundlesManager::get().await;
	tokio::spawn(async {
		check_and_apply_all_bundle_updates().await;
	});
}

async fn check_and_apply_all_bundle_updates() {
	BUNDLE_SYNCING.store(true, Ordering::Relaxed);
	tracing::info!("checking for bundle updates...");

	let clusters = match get_all_clusters().await {
		Ok(clusters) => clusters,
		Err(err) => {
			tracing::error!("failed to get clusters for bundle update check: {err}");
			BUNDLE_SYNCING.store(false, Ordering::Relaxed);
			return;
		}
	};

	let mut total_updates_applied = 0;
	let mut total_removals_applied = 0;
	let mut total_additions_applied = 0;
	let mut total_clusters_failed = 0;

	for cluster in clusters {
		tracing::debug!(
			cluster_id = %cluster.id,
			cluster_name = %cluster.name,
			"Checking and applying bundle updates for cluster"
		);

		match bundle_updates::apply_bundle_updates(cluster.id).await {
			Ok(result) => {
				let update_count = result.updates_applied.len();
				let removal_count = result.removals_applied.len();
				let addition_count = result.additions_applied.len();

				if update_count > 0 || removal_count > 0 || addition_count > 0 {
					total_updates_applied += update_count;
					total_removals_applied += removal_count;
					total_additions_applied += addition_count;

					if update_count > 0 {
						tracing::info!(
							"applied {} bundle update(s) for cluster '{}' (id: {})",
							update_count,
							cluster.name,
							cluster.id
						);

						for update in &result.updates_applied {
							tracing::info!(
								"  - updated package from bundle '{}': {} -> {}",
								update.bundle_name,
								update.installed_version_id,
								update.new_version_id
							);
						}
					}

					if removal_count > 0 {
						tracing::info!(
							"removed {} package(s) no longer in bundles for cluster '{}' (id: {})",
							removal_count,
							cluster.name,
							cluster.id
						);

						for removal in &result.removals_applied {
							tracing::info!(
								"  - removed package '{}' (was from bundle '{}')",
								removal.package_id,
								removal.bundle_name
							);
						}
					}

					if addition_count > 0 {
						tracing::info!(
							"installed {} new package(s) from bundles for cluster '{}' (id: {})",
							addition_count,
							cluster.name,
							cluster.id
						);

						for addition in &result.additions_applied {
							let file_id = match &addition.new_file.kind {
								onelauncher_core::api::packages::modpack::data::ModpackFileKind::Managed((pkg, _)) => pkg.id.clone(),
								onelauncher_core::api::packages::modpack::data::ModpackFileKind::External(ext) => ext.sha1.clone(),
							};
							tracing::info!(
								"  - installed new package '{}' from bundle '{}'",
								file_id,
								addition.bundle_name
							);
						}
					}
				} else {
					tracing::debug!("no bundle updates needed for cluster '{}'", cluster.name);
				}
			}
			Err(err) => {
				total_clusters_failed += 1;
				tracing::warn!(
					"failed to apply bundle updates for cluster '{}': {err}",
					cluster.name
				);
			}
		}
	}

	if total_updates_applied > 0 || total_removals_applied > 0 || total_additions_applied > 0 {
		let mut message_parts = Vec::new();
		if total_updates_applied > 0 {
			message_parts.push(format!("{} mod(s) updated", total_updates_applied));
		}
		if total_removals_applied > 0 {
			message_parts.push(format!("{} mod(s) removed", total_removals_applied));
		}
		if total_additions_applied > 0 {
			message_parts.push(format!("{} mod(s) added", total_additions_applied));
		}
		send_info!("Bundle sync: {}", message_parts.join(", "));
		tracing::info!(
			"bundle sync complete: {} updates applied, {} removals applied, {} additions applied",
			total_updates_applied,
			total_removals_applied,
			total_additions_applied
		);
	} else if total_clusters_failed == 0 {
		tracing::info!("all bundle packages are up to date");
	}

	if total_clusters_failed > 0 {
		tracing::warn!("failed to apply updates for {total_clusters_failed} cluster(s)");
	}

	BUNDLE_SYNCING.store(false, Ordering::Relaxed);
}
