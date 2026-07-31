use futures_util::StreamExt;
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::ClusterRow;
use oneclient_db::models::OverrideType;

use crate::error::ContentError;
use crate::error::ContentResult;
use crate::bundles::error::BundleError;
use crate::bundles::manager::BundlesManager;
use crate::bundles::overrides;
use crate::bundles::types::{BundleArchive, BundleFile, BundleFileKind};
use oneclient_events::{GroupedProgressChild, GroupedProgressSession, TaskCategory, TaskPhase};
use oneclient_common::domain::{ContentType, GameLoader};
use crate::packages::store::{PackageStore, evict_if_unused, try_unlink_materialized};
use crate::packages::types::ExternalFile;
use crate::ctx::ContentCtx;

fn is_base62(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn effective_enabled(file: &BundleFile, user_override: Option<OverrideType>) -> bool {
    match user_override {
        Some(OverrideType::Removed | OverrideType::Disabled) => false,
        Some(OverrideType::Enabled) => true,
        None => file.enabled,
    }
}

fn find_override(
    overrides: &[oneclient_db::models::ClusterBundleOverrideRow],
    bundle_name: &str,
    package_id: &str,
) -> Option<OverrideType> {
    overrides
        .iter()
        .find(|o| o.bundle_name == bundle_name && o.package_id == package_id)
        .and_then(|o| OverrideType::parse(&o.override_type))
}

async fn clear_suppressing_override(
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    if let Some(OverrideType::Enabled) = find_override(&overrides, bundle_name, package_id) {
        return Ok(());
    }
    bundle_dao::remove_override(&ctx.db, cluster_id, bundle_name, package_id).await?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip(file, child, ctx), fields(package = %file.display_name()))]
pub async fn install_package_from_bundle(
    file: &BundleFile,
    cluster_id: i64,
    bundle_name: &str,
    skip_compatibility: bool,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<String> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let hash = match &file.kind {
        BundleFileKind::Managed {
            provider,
            project_id,
            version_id,
            sha1,
        } => {
            let project = crate::packages::cached_project_detail(
                ctx,
                *provider,
                project_id,
                file.content_type(),
            )
            .await;

            let version = if is_base62(version_id) {
                crate::packages::get_version_cached(ctx, *provider, project_id, version_id)
                    .await?
            } else if let Ok(Some((_, version))) =
                ctx.providers.lookup_version(sha1, ctx).await
            {
                version
            } else {
                crate::packages::get_version_cached(ctx, *provider, project_id, version_id)
                    .await?
            };

            let artifact = PackageStore::install_to_cluster(
                *provider,
                &project,
                &version,
                cluster_id,
                skip_compatibility,
                false,
                child,
                ctx,
            )
            .await?;
            artifact.hash
        }
        BundleFileKind::External(ext) => {
            install_external(ext, &cluster, skip_compatibility, child, ctx).await?
        }
    };

    bundle_dao::track_bundle_artifact(
        &ctx.db,
        cluster_id,
        &hash,
        bundle_name,
        &file.kind.bundle_version_id(),
        &file.kind.package_id(),
    )
    .await?;

    Ok(hash)
}

#[tracing::instrument(level = "debug", skip(ext, cluster, child, ctx), fields(file = %ext.name))]
async fn install_external(
    ext: &ExternalFile,
    cluster: &ClusterRow,
    skip_compatibility: bool,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<String> {
    let artifact = crate::packages::store::download_external(ext, false, child, ctx).await?;
    PackageStore::link_artifact(&artifact, cluster, Some(&ext.name), ctx).await?;

    let _ = skip_compatibility;
    Ok(artifact.hash)
}

#[tracing::instrument(level = "debug", skip(archive, ctx), fields(bundle = %archive.manifest.name))]
pub async fn extract_bundle_overrides_for_cluster(
    archive: &BundleArchive,
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    overrides::sync_bundle_overrides(&archive.bundle.path, &archive.manifest.name, &cluster, None)
        .await?;
    Ok(())
}

#[tracing::instrument(skip(bundles, ctx))]
pub async fn install_bundle(
    cluster_id: i64,
    bundle_name: &str,
    skip_compatibility: bool,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<Vec<String>> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).ok_or_else(|| {
        ContentError::InvalidData {
            reason: format!("unknown loader {}", cluster.mc_loader),
        }
    })?;

    let archive = bundles
        .archives_for(ctx, &cluster.mc_version, loader)
        .await?
        .into_iter()
        .find(|a| a.manifest.name == bundle_name)
        .ok_or(BundleError::NotFound(bundle_name.to_string()))?;

    install_enabled_bundle_files(&archive, cluster_id, skip_compatibility, None, ctx).await
}

#[tracing::instrument(level = "debug", skip(archive, progress, ctx), fields(bundle = %archive.manifest.name))]
pub async fn install_enabled_bundle_files(
    archive: &BundleArchive,
    cluster_id: i64,
    skip_compatibility: bool,
    progress: Option<&GroupedProgressSession>,
    ctx: &ContentCtx,
) -> ContentResult<Vec<String>> {
    extract_bundle_overrides_for_cluster(archive, cluster_id, ctx).await?;

    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    let bundle_name = archive.manifest.name.clone();
    let mut installed = Vec::new();

    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;
    let mut linked_projects: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut linked_hashes: std::collections::HashSet<&str> = std::collections::HashSet::new();
    // "Already installed" is what the database says, not what is on disk right
    // now: content lives in the cache between sessions, so probing the folder
    // would report every package as missing and reinstall the whole bundle.
    for info in &linked {
        if let Some(pid) = &info.project_id {
            linked_projects.insert(pid.as_str());
        }
        linked_hashes.insert(info.hash.as_str());
    }

    let to_install: Vec<BundleFile> = archive
        .manifest
        .files
        .iter()
        .filter(|file| {
            let package_id = file.kind.package_id();
            if !effective_enabled(file, find_override(&overrides, &bundle_name, &package_id)) {
                return false;
            }
            let already_installed = match &file.kind {
                BundleFileKind::Managed { project_id, .. } => {
                    linked_projects.contains(project_id.as_str())
                }
                BundleFileKind::External(ext) => linked_hashes.contains(ext.sha1.as_str()),
            };
            !already_installed
        })
        .cloned()
        .collect();

    tracing::info!(
        cluster_id,
        bundle = %bundle_name,
        to_install = to_install.len(),
        "installing enabled bundle files"
    );

    if let Some(p) = progress {
        let reserved_bytes: u64 = to_install.iter().map(|f| f.size.max(1)).sum();
        p.expect(TaskCategory::Packages, to_install.len() as u64, reserved_bytes);
    }

    let bundle_name = &bundle_name;
    let results = futures_util::stream::iter(to_install.into_iter().map(|file| async move {
        let child = progress.map(|p| {
            let c = p.child(
                format!("Mod {}", file.display_name()),
                file.size.max(1),
                oneclient_events::TaskCategory::Packages,
            );
            c.set_phase(TaskPhase::Downloading);
            c
        });

        let result = install_package_from_bundle(
            &file,
            cluster_id,
            bundle_name,
            skip_compatibility,
            child.as_ref(),
            ctx,
        )
        .await;

        if let Some(child) = child {
            child.set_phase(TaskPhase::Installing);
            child.finish();
        }
        (file.display_name(), result)
    }))
    .buffer_unordered(BUNDLE_INSTALL_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    for (name, result) in results {
        match result {
            Ok(hash) => installed.push(hash),
            Err(err) => {
                tracing::warn!(file = %name, error = %err, "failed to install bundle file");
            }
        }
    }

    Ok(installed)
}

/// How many bundle packages are fetched at once. Kept modest because each one
/// also costs a provider API call, and those are rate limited per-minute.
pub(crate) const BUNDLE_INSTALL_CONCURRENCY: usize = 6;

#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn enabled_bundle_bytes(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<u64> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).ok_or_else(|| {
        ContentError::InvalidData {
            reason: format!("unknown loader {}", cluster.mc_loader),
        }
    })?;

    let archives = bundles
        .archives_for(ctx, &cluster.mc_version, loader)
        .await?;
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;

    let mut linked_projects: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut linked_hashes: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for info in &linked {
        if let Some(pid) = &info.project_id {
            linked_projects.insert(pid.as_str());
        }
        linked_hashes.insert(info.hash.as_str());
    }

    let mut total = 0u64;
    for archive in &archives {
        let bundle_name = &archive.manifest.name;
        for file in &archive.manifest.files {
            let package_id = file.kind.package_id();
            if !effective_enabled(file, find_override(&overrides, bundle_name, &package_id)) {
                continue;
            }
            let already_installed = match &file.kind {
                BundleFileKind::Managed { project_id, .. } => {
                    linked_projects.contains(project_id.as_str())
                }
                BundleFileKind::External(ext) => linked_hashes.contains(ext.sha1.as_str()),
            };
            if already_installed {
                continue;
            }
            total += file.size;
        }
    }

    Ok(total)
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn set_bundle_package_override(
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
    override_type: Option<OverrideType>,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    match override_type {
        Some(ty) => {
            bundle_dao::save_override(&ctx.db, cluster_id, bundle_name, package_id, ty).await?;
        }
        None => {
            bundle_dao::remove_override(&ctx.db, cluster_id, bundle_name, package_id).await?;
        }
    }

    Ok(())
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn set_bundle_package_enabled(
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
    enabled: bool,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let override_type = if enabled {
        None
    } else {
        Some(OverrideType::Disabled)
    };
    set_bundle_package_override(cluster_id, bundle_name, package_id, override_type, ctx).await
}

#[tracing::instrument(level = "debug", skip(overrides, ctx), fields(count = overrides.len()))]
pub async fn set_bundle_package_overrides(
    cluster_id: i64,
    overrides: &[(String, String, OverrideType)],
    ctx: &ContentCtx,
) -> ContentResult<()> {
    bundle_dao::save_overrides(&ctx.db, cluster_id, overrides).await?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn set_bundle_package_opt_in(
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
    opted_in: bool,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let override_type = if opted_in {
        Some(OverrideType::Enabled)
    } else {
        None
    };
    set_bundle_package_override(cluster_id, bundle_name, package_id, override_type, ctx).await
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn list_cluster_bundle_overrides(
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<Vec<(String, String, String)>> {
    let rows = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    Ok(rows
        .into_iter()
        .map(|o| (o.bundle_name, o.package_id, o.override_type))
        .collect())
}

#[tracing::instrument(skip(bundles, progress, ctx))]
pub async fn install_cluster_bundles(
    cluster_id: i64,
    bundles: &BundlesManager,
    progress: Option<&GroupedProgressSession>,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).ok_or_else(|| {
        ContentError::InvalidData {
            reason: format!("unknown loader {}", cluster.mc_loader),
        }
    })?;

    let archives = bundles
        .archives_for(ctx, &cluster.mc_version, loader)
        .await?;
    tracing::info!(
        cluster_id,
        mc_version = %cluster.mc_version,
        bundles = archives.len(),
        "installing enabled bundle content"
    );
    for archive in &archives {
        let installed =
            install_enabled_bundle_files(archive, cluster_id, true, progress, ctx).await?;
        tracing::info!(
            cluster_id,
            bundle = %archive.manifest.name,
            installed = installed.len(),
            "installed bundle files"
        );
    }

    Ok(())
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn on_user_remove_artifact(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    handle_user_artifact_action(cluster_id, hash, ctx, OverrideType::Removed).await
}

#[tracing::instrument(level = "debug", skip(ctx))]
/// Flips an artifact's enabled state and updates the bundle override records.
///
/// The composition point between the two layers: `PackageStore` does the
/// storage half and knows nothing about bundles; this adds the bookkeeping that
/// only makes sense once bundles exist.
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn toggle_artifact_enabled(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<bool> {
    let enabled = PackageStore::set_artifact_enabled(cluster_id, hash, ctx).await?;

    if enabled {
        on_user_enable_artifact(cluster_id, hash, ctx).await?;
    } else {
        on_user_disable_artifact(cluster_id, hash, ctx).await?;
    }

    Ok(enabled)
}

pub async fn on_user_disable_artifact(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    handle_user_artifact_action(cluster_id, hash, ctx, OverrideType::Disabled).await
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn on_user_enable_artifact(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    if let Some(tracked) = bundle_dao::get_bundle_tracked(&ctx.db, cluster_id, hash).await?
        && let (Some(bundle_name), Some(package_id)) = (tracked.bundle_name, tracked.package_id) {
            clear_suppressing_override(cluster_id, &bundle_name, &package_id, ctx).await?;
        }
    Ok(())
}

#[tracing::instrument(level = "debug", skip(ctx))]
async fn handle_user_artifact_action(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
    override_type: OverrideType,
) -> ContentResult<()> {
    let Some(tracked) = bundle_dao::get_bundle_tracked(&ctx.db, cluster_id, hash).await?
    else {
        return Ok(());
    };

    let (Some(bundle_name), Some(package_id)) = (tracked.bundle_name, tracked.package_id) else {
        return Ok(());
    };

    bundle_dao::save_override(
        &ctx.db,
        cluster_id,
        &bundle_name,
        &package_id,
        override_type,
    )
    .await?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn remove_artifact_from_cluster(
    cluster_id: i64,
    hash: &str,
    record_override: bool,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let bundle_data = bundle_dao::get_bundle_tracked(&ctx.db, cluster_id, hash).await?;

    // Everything needed for the eager folder cleanup is looked up first, but
    // none of it is allowed to block the removal itself. A package whose
    // artifact row has gone missing still has to be removable.
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let target = artifact_dao::get_artifact_by_hash(&ctx.db, hash)
        .await?
        .and_then(|artifact| ContentType::from_repr(artifact.content_type as u8));

    let link = artifact_dao::list_cluster_artifacts(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .find(|l| l.hash == hash);

    // The database first, and unconditionally. Removal used to delete the file
    // before dropping the row, so a jar held open by a running game — on Windows
    // that blocks deleting every hard link to it — failed the whole operation
    // and left the package installed. Dropping the row always succeeds, and the
    // game folder is rebuilt from the database at the next launch.
    artifact_dao::unlink_cluster_artifact(&ctx.db, cluster_id, hash).await?;

    // Then a best-effort pass at the folder so it matches straight away when
    // nothing is holding the file. Failure here is not an error.
    if let (Some(content_type), Some(link)) = (target, link) {
        try_unlink_materialized(&cluster, content_type, &link.cluster_file_name).await;
    }

    // The cache is where the package actually lives, so it goes too once the
    // last cluster stops asking for it. Still installed elsewhere means still
    // needed, which `evict_if_unused` checks for us.
    if let Err(err) = evict_if_unused(hash, ctx).await {
        tracing::warn!(hash, error = %err, "failed to evict unused artifact from the cache");
    }

    if let Some(tracked) = bundle_data
        && let (Some(bundle_name), Some(package_id)) =
            (tracked.bundle_name.clone(), tracked.package_id.clone())
        {
            if record_override {
                bundle_dao::save_override(
                    &ctx.db,
                    cluster_id,
                    &bundle_name,
                    &package_id,
                    OverrideType::Removed,
                )
                .await?;
            } else {
                let replacement_exists = bundle_dao::has_bundle_mapping(
                    &ctx.db,
                    cluster_id,
                    &bundle_name,
                    &package_id,
                )
                .await?;
                if !replacement_exists {
                    clear_suppressing_override(
                        cluster_id,
                        &bundle_name,
                        &package_id,
                        ctx,
                    )
                    .await?;
                }
            }
        }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oneclient_common::domain::ContentType;
    use oneclient_db::models::ClusterBundleOverrideRow;

    fn file(enabled: bool) -> BundleFile {
        BundleFile {
            enabled,
            hidden: false,
            path: "mods/example.jar".to_string(),
            size: 1,
            kind: BundleFileKind::External(ExternalFile {
                name: "example.jar".to_string(),
                url: "https://example.invalid/example.jar".to_string(),
                sha1: "abc123".to_string(),
                size: 1,
                content_type: ContentType::Mod,
            }),
        }
    }

    #[test]
    fn override_none_falls_back_to_manifest_default() {
        assert!(effective_enabled(&file(true), None));
        assert!(!effective_enabled(&file(false), None));
    }

    #[test]
    fn suppressing_overrides_win_over_enabled_manifest() {
        assert!(!effective_enabled(&file(true), Some(OverrideType::Disabled)));
        assert!(!effective_enabled(&file(true), Some(OverrideType::Removed)));
    }

    #[test]
    fn enabled_override_wins_over_disabled_manifest() {
        assert!(effective_enabled(&file(false), Some(OverrideType::Enabled)));
    }

    #[test]
    fn enabled_override_round_trips_through_the_db_string() {
        let parsed = OverrideType::parse(OverrideType::Enabled.as_str());
        assert_eq!(parsed, Some(OverrideType::Enabled));
    }

    #[test]
    fn unknown_override_string_is_ignored() {
        assert_eq!(OverrideType::parse("something-new"), None);
        assert!(!effective_enabled(&file(false), None));
    }

    fn row(bundle: &str, pid: &str, ty: OverrideType) -> ClusterBundleOverrideRow {
        ClusterBundleOverrideRow {
            id: 1,
            cluster_id: 1,
            bundle_name: bundle.to_string(),
            package_id: pid.to_string(),
            override_type: ty.as_str().to_string(),
        }
    }

    #[test]
    fn find_override_matches_on_bundle_and_package() {
        let rows = vec![
            row("Bundle A", "pkg-1", OverrideType::Enabled),
            row("Bundle B", "pkg-2", OverrideType::Disabled),
        ];
        assert_eq!(
            find_override(&rows, "Bundle A", "pkg-1"),
            Some(OverrideType::Enabled)
        );
        assert_eq!(
            find_override(&rows, "Bundle B", "pkg-2"),
            Some(OverrideType::Disabled)
        );
        // Same package id under a different bundle must not collide.
        assert_eq!(find_override(&rows, "Bundle B", "pkg-1"), None);
        assert_eq!(find_override(&rows, "Bundle A", "pkg-3"), None);
    }
}
