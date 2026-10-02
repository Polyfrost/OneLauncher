use std::collections::{HashMap, HashSet};

use oneclient_db::dao::cluster_bundle as bundle_dao;

use super::loose_lock_key;
use crate::bundles::install::remove_artifact_from_cluster;
use crate::bundles::overrides::sync_file_lock;
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::store::PackageStore;
use oneclient_common::paths;

#[tracing::instrument(skip(ctx))]
pub async fn remove_modpack_files(
    cluster_id: i64,
    bundle_name: &str,
    ctx: &ContentCtx,
) -> ContentResult<Vec<String>> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let root = paths::cluster_game_dir(&cluster.folder_name, cluster.is_isolated())?;

    let tracked: Vec<String> = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .filter(|row| row.bundle_name.as_deref() == Some(bundle_name))
        .map(|row| row.hash)
        .collect();

    let mut failed = Vec::new();
    for hash in tracked {
        if let Err(err) = remove_artifact_from_cluster(cluster_id, &hash, false, ctx).await {
            tracing::warn!(hash, error = %err, "could not remove a modpack file");
            failed.push(hash);
        }
    }

    for row in bundle_dao::list_overrides(&ctx.db, cluster_id).await? {
        if row.bundle_name == bundle_name {
            bundle_dao::remove_override(&ctx.db, cluster_id, &row.bundle_name, &row.package_id)
                .await?;
        }
    }

    let listed = HashSet::new();
    sync_file_lock(&root, bundle_name, HashMap::new(), &listed).await;
    sync_file_lock(&root, &loose_lock_key(bundle_name), HashMap::new(), &listed).await;

    tracing::info!(
        cluster_id,
        bundle_name,
        failed = failed.len(),
        "modpack removed"
    );
    Ok(failed)
}
