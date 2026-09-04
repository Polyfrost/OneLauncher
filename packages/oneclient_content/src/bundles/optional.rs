use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::dao::cluster_optional_mod as optional_dao;
use oneclient_db::models::{OptionalModStatus, OverrideType};
use crate::bundles::manager::BundlesManager;
use crate::bundles::types::{BundleArchive, BundleFile};
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::store::PackageStore;
use oneclient_common::domain::GameLoader;

#[derive(Debug, Clone)]
pub struct PendingOptionalMod {
    pub cluster_id: i64,
    pub bundle_name: String,
    pub package_id: String,
    pub status: OptionalModStatus,
    pub file: BundleFile,
}

#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn pending_optional_mods(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<Vec<PendingOptionalMod>> {
    let rows: Vec<_> = optional_dao::list_pending(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .filter(|row| row.status() == OptionalModStatus::New)
        .collect();
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Fabric);
    let archives = bundles
        .archives_for(ctx, &cluster.mc_version, loader)
        .await?;
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;

    let mut pending = Vec::new();
    for row in rows {
        let Some((bundle_name, file)) = find_shipped(&archives, &row.bundle_name, &row.package_id)
        else {
            tracing::debug!(
                cluster_id,
                package_id = %row.package_id,
                "queued offer is no longer shipped by any bundle"
            );
            continue;
        };

        // The queue row can outlive the offer the user may have opted in from
        // the package manager since it was written
        let user_override = overrides
            .iter()
            .find(|o| o.bundle_name == bundle_name && o.package_id == row.package_id)
            .and_then(|o| OverrideType::parse(&o.override_type));
        if crate::bundles::effective_enabled(&file, user_override) {
            tracing::debug!(
                cluster_id,
                package_id = %row.package_id,
                "queued offer is already enabled for this cluster"
            );
            continue;
        }

        let status = row.status();
        pending.push(PendingOptionalMod {
            cluster_id,
            bundle_name,
            package_id: row.package_id,
            status,
            file,
        });
    }

    Ok(pending)
}

fn find_shipped(
    archives: &[BundleArchive],
    preferred_bundle: &str,
    package_id: &str,
) -> Option<(String, BundleFile)> {
    let in_archive = |archive: &BundleArchive| {
        archive
            .manifest
            .files
            .iter()
            .find(|file| file.kind.package_id() == package_id)
            .map(|file| (archive.manifest.name.clone(), file.clone()))
    };

    archives
        .iter()
        .find(|archive| archive.manifest.name == preferred_bundle)
        .and_then(in_archive)
        .or_else(|| archives.iter().find_map(in_archive))
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn skip_optional_mods(
    cluster_id: i64,
    package_ids: &[String],
    ctx: &ContentCtx,
) -> ContentResult<()> {
    optional_dao::mark_skipped(&ctx.db, cluster_id, package_ids).await?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn resolve_optional_mods(
    cluster_id: i64,
    package_ids: &[String],
    ctx: &ContentCtx,
) -> ContentResult<()> {
    optional_dao::resolve(&ctx.db, cluster_id, package_ids).await?;
    Ok(())
}
