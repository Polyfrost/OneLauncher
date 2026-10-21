use std::collections::{HashMap, HashSet};

use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::ClusterRow;

use super::{WantedFile, lock_key, loose_lock_key, shared_lock_key};
use crate::bundles::install::detach_artifact;
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
        if let Err(err) = detach_artifact(cluster_id, &hash, ctx).await {
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
pub async fn restore_disabled(
    cluster_id: i64,
    hashes: &[String],
    ctx: &ContentCtx,
) -> (Vec<String>, Vec<String>) {
    let mut restored = Vec::new();
    let mut stuck = Vec::new();
    for hash in hashes {
        let linked = oneclient_db::dao::artifact::get_cluster_artifact(&ctx.db, cluster_id, hash)
            .await
            .ok()
            .flatten();
        if linked.is_none_or(|link| link.enabled != 0) {
            continue;
        }
        match PackageStore::set_artifact_enabled_to(cluster_id, hash, true, ctx).await {
            Ok(_) => restored.push(hash.clone()),
            Err(err) => {
                tracing::warn!(hash, error = %err, "could not switch a mod back on");
                stuck.push(hash.clone());
            }
        }
    }
    (restored, stuck)
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
        detach_artifact(cluster_id, &hash, ctx).await?;
    }
    Ok(())
}

const IMPORTED_LIST: &str = ".oneclient/modpacks.json";

#[derive(serde::Deserialize)]
struct ImportedWants {
    bundle_name: String,
    #[serde(default)]
    wanted: Vec<WantedFile>,
}

pub(crate) async fn imported_wants(cluster: &ClusterRow) -> Vec<(String, WantedFile)> {
    let Ok(dir) = paths::cluster_dir(&cluster.folder_name) else {
        return Vec::new();
    };
    let Ok(bytes) = polyio::read(dir.join(IMPORTED_LIST)).await else {
        return Vec::new();
    };
    serde_json::from_slice::<Vec<ImportedWants>>(&bytes)
        .unwrap_or_default()
        .into_iter()
        .flat_map(|pack| {
            pack.wanted
                .into_iter()
                .map(move |wanted| (pack.bundle_name.clone(), wanted))
        })
        .collect()
}

pub(crate) async fn hand_to_wanting_pack(
    cluster_id: i64,
    hash: &str,
    wants: &[(String, WantedFile)],
    ctx: &ContentCtx,
) -> ContentResult<bool> {
    let Some(row) = bundle_dao::get_bundle_tracked(&ctx.db, cluster_id, hash).await? else {
        return Ok(false);
    };
    let Some((owner, wanted)) = wants.iter().find(|(owner, wanted)| {
        row.bundle_name.as_deref() != Some(owner.as_str())
            && (wanted.sha1 == hash || row.package_id.as_deref() == Some(wanted.package_id.as_str()))
    }) else {
        return Ok(false);
    };
    let version_id = if wanted.sha1 == hash {
        wanted.version_id.clone()
    } else {
        row.bundle_version_id.clone().unwrap_or_default()
    };
    bundle_dao::track_bundle_artifact(
        &ctx.db,
        cluster_id,
        hash,
        owner,
        &version_id,
        &wanted.package_id,
    )
    .await?;
    tracing::info!(hash, to = %owner, "handed a dropped file to a pack that still uses it");
    Ok(true)
}
