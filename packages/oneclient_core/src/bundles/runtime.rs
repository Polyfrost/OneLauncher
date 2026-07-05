use std::sync::atomic::{AtomicBool, Ordering};

use crate::bundles::manager::BundlesManager;
use crate::bundles::updates::apply_bundle_updates_for_all_clusters;
use crate::state::LauncherServices;

static BUNDLE_SYNCING: AtomicBool = AtomicBool::new(false);

pub fn is_bundle_syncing() -> bool {
    BUNDLE_SYNCING.load(Ordering::Relaxed)
}

pub async fn sync_all_cluster_bundles(bundles: &BundlesManager, services: &LauncherServices) {
    BUNDLE_SYNCING.store(true, Ordering::Relaxed);
    if let Err(err) = apply_bundle_updates_for_all_clusters(bundles, services).await {
        tracing::error!("failed to apply bundle updates for clusters: {err:#}");
    }
    BUNDLE_SYNCING.store(false, Ordering::Relaxed);
}
