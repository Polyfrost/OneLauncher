use std::collections::{HashMap, HashSet};

use oneclient_db::dao::cluster_bundle as bundle_dao;

use super::{WantedFile, lock_key, loose_lock_key, shared_lock_key};
use crate::bundles::install::remove_artifact_from_cluster;
use crate::bundles::overrides::{
    move_lock_entries, move_merge_parts, prune_merge_parts, sync_file_lock,
};
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::store::PackageStore;
use oneclient_common::paths;

#[tracing::instrument(skip(ctx))]
pub async fn remove_modpack_files(
    cluster_id: i64,
    bundle_name: &str,
    hand_files_to: Option<i64>,
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
    if !failed.is_empty() {
        return Ok(failed);
    }

    for row in bundle_dao::list_overrides(&ctx.db, cluster_id).await? {
        if row.bundle_name == bundle_name {
            bundle_dao::remove_override(&ctx.db, cluster_id, &row.bundle_name, &row.package_id)
                .await?;
        }
    }

    let key = lock_key(&cluster, bundle_name);
    match hand_files_to {
        Some(other) => {
            let to = shared_lock_key(bundle_name, other);
            move_lock_entries(&root, &key, &to).await;
            move_lock_entries(&root, &loose_lock_key(&key), &loose_lock_key(&to)).await;
            move_merge_parts(&root, &key, &to).await;
        }
        None => {
            let listed = HashSet::new();
            sync_file_lock(&root, &key, HashMap::new(), &listed).await;
            sync_file_lock(&root, &loose_lock_key(&key), HashMap::new(), &listed).await;
            prune_merge_parts(&root, &key, &listed).await;
        }
    }

    tracing::info!(
        cluster_id,
        bundle_name,
        handed_to = hand_files_to,
        failed = failed.len(),
        "modpack removed"
    );
    Ok(failed)
}

#[tracing::instrument(skip(ctx))]
pub async fn retrack_file(
    cluster_id: i64,
    hash: &str,
    bundle_name: &str,
    wanted: &WantedFile,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    bundle_dao::track_bundle_artifact(
        &ctx.db,
        cluster_id,
        hash,
        bundle_name,
        &wanted.version_id,
        &wanted.package_id,
    )
    .await?;
    Ok(())
}

#[tracing::instrument(skip(hashes, ctx))]
pub async fn restore_disabled(cluster_id: i64, hashes: &[String], ctx: &ContentCtx) {
    for hash in hashes {
        let linked = oneclient_db::dao::artifact::get_cluster_artifact(&ctx.db, cluster_id, hash)
            .await
            .ok()
            .flatten();
        if linked.is_none_or(|link| link.enabled != 0) {
            continue;
        }
        if let Err(err) = PackageStore::set_artifact_enabled_to(cluster_id, hash, true, ctx).await {
            tracing::warn!(hash, error = %err, "could not switch a mod back on");
        }
    }
}

#[tracing::instrument(skip(package_ids, ctx))]
pub async fn remove_tracked_packages(
    cluster_id: i64,
    bundle_name: &str,
    package_ids: &HashSet<String>,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    if package_ids.is_empty() {
        return Ok(());
    }

    let tracked: Vec<String> = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .filter(|row| row.bundle_name.as_deref() == Some(bundle_name))
        .filter(|row| {
            row.package_id
                .as_ref()
                .is_some_and(|id| package_ids.contains(id))
        })
        .map(|row| row.hash)
        .collect();

    for hash in tracked {
        remove_artifact_from_cluster(cluster_id, &hash, false, ctx).await?;
    }
    Ok(())
}
