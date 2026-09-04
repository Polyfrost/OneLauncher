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

/// Searches all bundles
/// a package's `bundle_name` is rewritten on every install so a choice filed
/// under its old bundle still counts
/// `Removed` outranks `Disabled`
/// `Enabled` never suppresses
pub(crate) fn find_user_suppression(
    overrides: &[oneclient_db::models::ClusterBundleOverrideRow],
    package_id: &str,
) -> Option<OverrideType> {
    let mut found = None;

    for row in overrides.iter().filter(|o| o.package_id == package_id) {
        match OverrideType::parse(&row.override_type) {
            Some(OverrideType::Removed) => return Some(OverrideType::Removed),
            Some(OverrideType::Disabled) => found = Some(OverrideType::Disabled),
            Some(OverrideType::Enabled) | None => {}
        }
    }

    found
}

/// Clears across all bundles
/// a row left behind under another bundle would keep answering "off" forever
async fn clear_suppressing_overrides(
    cluster_id: i64,
    package_id: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    bundle_dao::clear_suppressing_overrides(&ctx.db, cluster_id, package_id).await?;
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

/// `suppression` comes from [`find_user_suppression`] so a choice filed under a
/// bundle the file has since left still counts
pub(crate) fn disable_was_deliberate(suppression: Option<OverrideType>) -> bool {
    match suppression {
        Some(OverrideType::Removed | OverrideType::Disabled) => true,
        Some(OverrideType::Enabled) | None => false,
    }
}

#[tracing::instrument(level = "debug", skip(archives, ctx))]
pub async fn heal_bundle_activity(
    cluster_id: i64,
    archives: &[BundleArchive],
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
    if tracked.iter().all(|row| row.enabled != 0) {
        return Ok(());
    }

    let mut hidden: std::collections::HashMap<(&str, String), bool> =
        std::collections::HashMap::new();
    for archive in archives {
        for file in &archive.manifest.files {
            hidden.insert(
                (archive.manifest.name.as_str(), file.kind.package_id()),
                file.hidden,
            );
        }
    }

    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;

    for row in tracked.iter().filter(|row| row.enabled == 0) {
        let (Some(bundle_name), Some(package_id)) = (&row.bundle_name, &row.package_id) else {
            continue;
        };
        let Some(is_hidden) = hidden
            .get(&(bundle_name.as_str(), package_id.clone()))
            .copied()
        else {
            continue;
        };

        // Across bundles
        // a package that moved keeps its old override row and reading only its
        // current bundle would switch it back on
        let suppression = find_user_suppression(&overrides, package_id);
        if disable_was_deliberate(suppression) {
            continue;
        }

        tracing::info!(
            cluster_id,
            bundle = %bundle_name,
            package_id,
            hidden = is_hidden,
            ?suppression,
            "re-enabling bundle content that was switched off with nothing recording the choice"
        );

        // One unrepairable row must not fail the whole install
        if let Err(err) =
            PackageStore::set_artifact_enabled_to(cluster_id, &row.hash, true, ctx).await
        {
            tracing::warn!(hash = %row.hash, error = %err, "failed to re-enable bundle content");
            continue;
        }

        // Drop the stale override everywhere or a copy under another bundle
        // keeps answering "off" and the two records never settle
        if suppression == Some(OverrideType::Disabled) {
            clear_suppressing_overrides(cluster_id, package_id, ctx).await?;
        }
    }

    Ok(())
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
    heal_bundle_activity(cluster_id, std::slice::from_ref(archive), ctx).await?;

    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    let bundle_name = archive.manifest.name.clone();
    let mut installed = Vec::new();

    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;
    let mut linked_projects: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut linked_hashes: std::collections::HashSet<&str> = std::collections::HashSet::new();
    // "Already installed" means the database not disk
    // content lives in the cache between sessions so probing the folder would
    // reinstall everything
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

/// Kept modest each fetch also costs a provider API call rate limited per-minute
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
        // The UI shows one row per package across bundles so an objection left
        // under another bundle would let the next pass undo this switch
        Some(ty @ OverrideType::Enabled) => {
            clear_suppressing_overrides(cluster_id, package_id, ctx).await?;
            bundle_dao::save_override(&ctx.db, cluster_id, bundle_name, package_id, ty).await?;
        }
        Some(ty) => {
            bundle_dao::save_override(&ctx.db, cluster_id, bundle_name, package_id, ty).await?;
        }
        None => {
            bundle_dao::remove_override(&ctx.db, cluster_id, bundle_name, package_id).await?;
        }
    }

    Ok(())
}

/// For bundle files the cluster has not installed
/// installed ones go through [`toggle_artifact_enabled`]
/// Matching the manifest default clears the override
/// switching *on* also drops objections filed under other bundles
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn set_bundle_package_enabled(
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
    enabled: bool,
    manifest_default: bool,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let override_type = match (enabled, manifest_default) {
        (true, true) | (false, false) => None,
        (true, false) => Some(OverrideType::Enabled),
        (false, true) => Some(OverrideType::Disabled),
    };

    set_bundle_package_override(cluster_id, bundle_name, package_id, override_type, ctx).await?;

    if enabled {
        clear_suppressing_overrides(cluster_id, package_id, ctx).await?;
    }

    Ok(())
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

/// Prefer this over [`toggle_artifact_enabled`] unless the current value is
/// known to be the opposite
/// a relinked artifact keeps its old `enabled` so a flip on an already-correct
/// row puts it wrong
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn set_artifact_enabled_to(
    cluster_id: i64,
    hash: &str,
    enabled: bool,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    PackageStore::set_artifact_enabled_to(cluster_id, hash, enabled, ctx).await?;

    if enabled {
        on_user_enable_artifact(cluster_id, hash, ctx).await
    } else {
        on_user_disable_artifact(cluster_id, hash, ctx).await
    }
}

/// Writes the override alongside the flag
/// without it the losing bundle copy looks disabled-by-nobody and
/// heal_bundle_activity re-enables it every launch
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn reconcile_duplicate_activity(
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    for hash in crate::packages::reconcile_duplicate_activity(cluster_id, ctx).await? {
        on_user_disable_artifact(cluster_id, &hash, ctx).await?;
    }

    Ok(())
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
        && let Some(package_id) = tracked.package_id {
            clear_suppressing_overrides(cluster_id, &package_id, ctx).await?;
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

    // Looked up first but never allowed to block removal
    // a package whose artifact row has gone missing must still be removable
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let target = artifact_dao::get_artifact_by_hash(&ctx.db, hash)
        .await?
        .and_then(|artifact| ContentType::from_repr(artifact.content_type as u8));

    let link = artifact_dao::list_cluster_artifacts(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .find(|l| l.hash == hash);

    // Database first unconditionally
    // on Windows a jar held open by a running game blocks deleting every hard
    // link which used to fail the whole removal
    // The folder is rebuilt from the database at the next launch
    artifact_dao::unlink_cluster_artifact(&ctx.db, cluster_id, hash).await?;

    // Best-effort folder cleanup failure here is not an error
    if let (Some(content_type), Some(link)) = (target, link) {
        try_unlink_materialized(&cluster, content_type, &link.cluster_file_name).await;
    }

    // The package actually lives in the cache
    // `evict_if_unused` drops it only once no other cluster still needs it
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
                let replacement_exists = bundle_dao::has_bundle_mapping_for_package(
                    &ctx.db,
                    cluster_id,
                    &package_id,
                )
                .await?;
                if !replacement_exists {
                    clear_suppressing_overrides(cluster_id, &package_id, ctx).await?;
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
    fn a_disable_with_no_override_behind_it_is_an_accident() {
        assert!(!disable_was_deliberate(None));
    }

    #[test]
    fn a_users_own_disable_is_respected() {
        assert!(disable_was_deliberate(Some(OverrideType::Disabled)));
        assert!(disable_was_deliberate(Some(OverrideType::Removed)));
    }

    #[test]
    fn a_hidden_dependency_can_be_disabled_on_its_own() {
        let file = BundleFile {
            hidden: true,
            ..file(true)
        };

        assert!(
            disable_was_deliberate(Some(OverrideType::Disabled)),
            "the hidden filter offers the toggle so the choice behind it has to outlive a launch"
        );
        assert!(!effective_enabled(&file, Some(OverrideType::Disabled)));
    }

    #[test]
    fn an_opt_in_override_never_reads_as_a_disable() {
        assert!(!disable_was_deliberate(Some(OverrideType::Enabled)));
    }

    #[test]
    fn suppression_is_found_under_a_bundle_the_package_has_since_left() {
        let rows = vec![row("Bundle B", "fabric-api", OverrideType::Disabled)];

        assert_eq!(
            find_override(&rows, "Bundle C", "fabric-api"),
            None,
            "the per-bundle question is still answered per bundle"
        );
        assert_eq!(
            find_user_suppression(&rows, "fabric-api"),
            Some(OverrideType::Disabled),
            "the user's choice follows the package, not the bundle it was filed under"
        );
    }

    #[test]
    fn removal_outranks_a_disable_filed_elsewhere() {
        let rows = vec![
            row("Bundle A", "yacl", OverrideType::Disabled),
            row("Bundle B", "yacl", OverrideType::Removed),
        ];

        assert_eq!(
            find_user_suppression(&rows, "yacl"),
            Some(OverrideType::Removed)
        );
    }

    #[test]
    fn an_opt_in_is_not_an_objection() {
        let rows = vec![
            row("Bundle A", "sodium", OverrideType::Enabled),
            row("Bundle B", "lithium", OverrideType::Disabled),
        ];

        assert_eq!(find_user_suppression(&rows, "sodium"), None);
        assert_eq!(find_user_suppression(&rows, "unheard-of"), None);
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
        assert_eq!(find_override(&rows, "Bundle B", "pkg-1"), None);
        assert_eq!(find_override(&rows, "Bundle A", "pkg-3"), None);
    }
}
