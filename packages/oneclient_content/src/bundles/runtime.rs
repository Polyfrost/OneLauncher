use std::sync::atomic::{AtomicBool, Ordering};


use crate::bundles::manager::BundlesManager;
use crate::bundles::ApplyBundleUpdatesResult;
use crate::bundles::updates::apply_bundle_updates_for_all_clusters;
use oneclient_events::GroupedProgressSession;
use crate::ctx::ContentCtx;

static BUNDLE_SYNCING: AtomicBool = AtomicBool::new(false);

pub fn is_bundle_syncing() -> bool {
    BUNDLE_SYNCING.load(Ordering::Relaxed)
}

/// `session` when given groups every cluster's downloads into one notification
#[tracing::instrument(skip_all)]
pub async fn sync_all_cluster_bundles(
    bundles: &BundlesManager,
    ctx: &ContentCtx,
    session: Option<&GroupedProgressSession>,
) -> Vec<(i64, ApplyBundleUpdatesResult)> {
    tracing::info!("syncing all cluster bundles");
    BUNDLE_SYNCING.store(true, Ordering::Relaxed);
    let changed = match apply_bundle_updates_for_all_clusters(bundles, ctx, session).await {
        Ok(changed) => changed,
        Err(err) => {
            tracing::error!("failed to apply bundle updates for clusters: {err:#}");
            Vec::new()
        }
    };
    BUNDLE_SYNCING.store(false, Ordering::Relaxed);
    changed
}
