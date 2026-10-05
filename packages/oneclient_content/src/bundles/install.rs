use futures_util::StreamExt;
use oneclient_db::dao::applied_migration as migration_dao;
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster as cluster_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::ClusterRow;
use oneclient_db::models::OverrideType;

use crate::bundles::error::BundleError;
use crate::bundles::manager::BundlesManager;
use crate::bundles::overrides;
use crate::bundles::types::{BundleArchive, BundleFile, BundleFileKind};
use crate::ctx::ContentCtx;
use crate::error::ContentError;
use crate::error::ContentResult;
use crate::packages::store::{LiveSync, PackageStore, evict_if_unused, try_unlink_materialized};
use crate::packages::types::ExternalFile;
use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_events::{GroupedProgressChild, GroupedProgressSession, TaskCategory, TaskPhase};

fn is_base62(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

pub const BUNDLE_CONSENT: &str = "*";

fn takes_bundle(archive: &BundleArchive, live: &std::collections::HashSet<String>) -> bool {
    archive.manifest.enabled || live.contains(&archive.manifest.name)
}

fn whole_bundle_names(
    tracked: &[oneclient_db::models::BundleTrackedArtifactRow],
    overrides: &[oneclient_db::models::ClusterBundleOverrideRow],
) -> std::collections::HashSet<String> {
    tracked
        .iter()
        .filter(|row| row.enabled != 0)
        .filter_map(|row| row.bundle_name.clone())
        .chain(
            overrides
                .iter()
                .filter(|o| {
                    o.package_id == BUNDLE_CONSENT
                        && o.bundle_name != BUNDLE_CONSENT
                        && OverrideType::parse(&o.override_type) == Some(OverrideType::Enabled)
                })
                .map(|o| o.bundle_name.clone()),
        )
        .collect()
}

fn taken_bundles(
    tracked: &[oneclient_db::models::BundleTrackedArtifactRow],
    overrides: &[oneclient_db::models::ClusterBundleOverrideRow],
) -> (
    std::collections::HashSet<String>,
    std::collections::HashSet<String>,
) {
    (
        super::updates::live_bundle_names(tracked, overrides),
        whole_bundle_names(tracked, overrides),
    )
}

fn awaits_bundle_prompt(overrides: &[oneclient_db::models::ClusterBundleOverrideRow]) -> bool {
    find_override(overrides, BUNDLE_CONSENT, BUNDLE_CONSENT).is_some()
}

pub async fn taken_bundle_names(
    cluster_id: i64,
    archives: &[BundleArchive],
    ctx: &ContentCtx,
) -> ContentResult<std::collections::HashSet<String>> {
    let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    let (live, _) = taken_bundles(&tracked, &overrides);
    Ok(archives
        .iter()
        .filter(|archive| takes_bundle(archive, &live))
        .map(|archive| archive.manifest.name.clone())
        .collect())
}

fn unpicked_defaults(
    archive: &BundleArchive,
    archives: &[BundleArchive],
    live: &std::collections::HashSet<String>,
    whole: &std::collections::HashSet<String>,
    overrides: &[oneclient_db::models::ClusterBundleOverrideRow],
) -> std::collections::HashSet<String> {
    let bundle_name = &archive.manifest.name;
    if archive.manifest.enabled || !live.contains(bundle_name) || whole.contains(bundle_name) {
        return Default::default();
    }

    archive
        .manifest
        .files
        .iter()
        .filter(|file| file.enabled && !file.hidden)
        .map(|file| file.kind.package_id())
        .filter(|package_id| find_override(overrides, bundle_name, package_id).is_none())
        .filter(|package_id| {
            !archives.iter().any(|other| {
                other.manifest.name != *bundle_name
                    && takes_bundle(other, live)
                    && other
                        .manifest
                        .files
                        .iter()
                        .any(|f| f.enabled && f.kind.package_id() == *package_id)
            })
        })
        .collect()
}

pub(crate) async fn decline_unpicked_defaults(
    cluster_id: i64,
    archives: &[BundleArchive],
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    if awaits_bundle_prompt(&overrides) {
        return Ok(());
    }
    let (live, whole) = taken_bundles(&tracked, &overrides);
    let declined: Vec<_> = archives
        .iter()
        .flat_map(|archive| {
            unpicked_defaults(archive, archives, &live, &whole, &overrides)
                .into_iter()
                .map(|package_id| {
                    (
                        archive.manifest.name.clone(),
                        package_id,
                        OverrideType::Removed,
                    )
                })
        })
        .collect();
    bundle_dao::save_overrides(&ctx.db, cluster_id, &declined).await?;
    Ok(())
}

fn inherited_consents(
    source_tracked: &[oneclient_db::models::BundleTrackedArtifactRow],
    source_overrides: &[oneclient_db::models::ClusterBundleOverrideRow],
    target_tracked: &[oneclient_db::models::BundleTrackedArtifactRow],
    target_overrides: &[oneclient_db::models::ClusterBundleOverrideRow],
    declined: &std::collections::HashSet<String>,
) -> Vec<(String, String, OverrideType)> {
    let mut names: Vec<String> = whole_bundle_names(source_tracked, source_overrides)
        .into_iter()
        .filter(|name| {
            !target_overrides.iter().any(|o| o.bundle_name == *name)
                && !target_tracked
                    .iter()
                    .any(|row| row.bundle_name.as_deref() == Some(name.as_str()))
        })
        .collect();
    names.sort();

    let mut rows = Vec::new();
    for name in names {
        for o in source_overrides
            .iter()
            .filter(|o| o.bundle_name == name && o.package_id != BUNDLE_CONSENT)
        {
            if let Some(ty @ (OverrideType::Removed | OverrideType::Disabled)) =
                OverrideType::parse(&o.override_type)
            {
                rows.push((name.clone(), o.package_id.clone(), ty));
            }
        }
        for row in source_tracked.iter().filter(|row| {
            row.bundle_name.as_deref() == Some(name.as_str()) && declined.contains(&row.hash)
        }) {
            if let Some(package_id) = &row.package_id {
                rows.push((name.clone(), package_id.clone(), OverrideType::Removed));
            }
        }
        rows.push((name, BUNDLE_CONSENT.to_string(), OverrideType::Enabled));
    }
    if awaits_bundle_prompt(source_overrides) {
        rows.push((
            BUNDLE_CONSENT.to_string(),
            BUNDLE_CONSENT.to_string(),
            OverrideType::Enabled,
        ));
    }
    rows
}

#[tracing::instrument(level = "debug", skip(declined, ctx))]
pub async fn inherit_bundle_consent(
    source_cluster_id: i64,
    target_cluster_id: i64,
    declined: &std::collections::HashSet<String>,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    if bundle_cluster(source_cluster_id, ctx).await?.is_none()
        || bundle_cluster(target_cluster_id, ctx).await?.is_none()
    {
        return Ok(());
    }

    let consents = inherited_consents(
        &bundle_dao::list_bundle_tracked(&ctx.db, source_cluster_id).await?,
        &bundle_dao::list_overrides(&ctx.db, source_cluster_id).await?,
        &bundle_dao::list_bundle_tracked(&ctx.db, target_cluster_id).await?,
        &bundle_dao::list_overrides(&ctx.db, target_cluster_id).await?,
        declined,
    );
    bundle_dao::save_overrides(&ctx.db, target_cluster_id, &consents).await?;
    Ok(())
}

async fn cluster_archives(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<Vec<BundleArchive>> {
    let Some(cluster) = bundle_cluster(cluster_id, ctx).await? else {
        return Ok(Vec::new());
    };
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Fabric);
    bundles.archives_for(ctx, &cluster.mc_version, loader).await
}

#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn pending_bundle_choices(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<Vec<(String, bool)>> {
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    if !awaits_bundle_prompt(&overrides) {
        return Ok(Vec::new());
    }
    let archives = cluster_archives(cluster_id, bundles, ctx).await?;
    if archives.is_empty() {
        return Ok(Vec::new());
    }

    let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
    let (live, _) = taken_bundles(&tracked, &overrides);
    let choices: Vec<_> = archives
        .iter()
        .filter(|archive| !archive.manifest.enabled && !archive.bundle.hidden)
        .map(|archive| {
            let name = archive.manifest.name.clone();
            let held = live.contains(&name);
            (name, held)
        })
        .collect();
    if choices.is_empty() {
        bundle_dao::remove_override(&ctx.db, cluster_id, BUNDLE_CONSENT, BUNDLE_CONSENT).await?;
    }
    Ok(choices)
}

#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn choose_bundles(
    cluster_id: i64,
    chosen: &std::collections::HashSet<String>,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let archives = cluster_archives(cluster_id, bundles, ctx).await?;
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
    let (live, _) = taken_bundles(&tracked, &overrides);
    let kept = |archive: &BundleArchive| {
        archive.manifest.enabled || chosen.contains(&archive.manifest.name)
    };

    for archive in archives
        .iter()
        .filter(|archive| !archive.manifest.enabled && !archive.bundle.hidden)
    {
        let name = &archive.manifest.name;
        let held = live.contains(name);
        let stale = if kept(archive) {
            OverrideType::Removed
        } else {
            OverrideType::Enabled
        };
        if kept(archive) != held {
            for o in overrides.iter().filter(|o| {
                o.bundle_name == *name && OverrideType::parse(&o.override_type) == Some(stale)
            }) {
                bundle_dao::remove_override(&ctx.db, cluster_id, name, &o.package_id).await?;
            }
        }
        if kept(archive) {
            bundle_dao::save_override(
                &ctx.db,
                cluster_id,
                name,
                BUNDLE_CONSENT,
                OverrideType::Enabled,
            )
            .await?;
            continue;
        }

        for row in tracked
            .iter()
            .filter(|row| row.bundle_name.as_deref() == Some(name.as_str()))
        {
            let rehome = row.package_id.as_deref().and_then(|package_id| {
                archives
                    .iter()
                    .filter(|other| other.manifest.name != *name && kept(other))
                    .find_map(|other| {
                        let file = other
                            .manifest
                            .files
                            .iter()
                            .find(|f| f.enabled && f.kind.package_id() == package_id)?;
                        Some((
                            &other.manifest.name,
                            file.kind.bundle_version_id(),
                            package_id,
                        ))
                    })
            });
            match rehome {
                Some((bundle_name, version_id, package_id)) => {
                    bundle_dao::track_bundle_artifact(
                        &ctx.db,
                        cluster_id,
                        &row.hash,
                        bundle_name,
                        &version_id,
                        package_id,
                    )
                    .await?;
                }
                None => remove_artifact_from_cluster(cluster_id, &row.hash, false, ctx).await?,
            }
        }
    }

    bundle_dao::remove_override(&ctx.db, cluster_id, BUNDLE_CONSENT, BUNDLE_CONSENT).await?;
    Ok(())
}

fn every_file_switched_off(
    archive: &BundleArchive,
    overrides: &[oneclient_db::models::ClusterBundleOverrideRow],
) -> bool {
    let bundle_name = &archive.manifest.name;
    !archive.manifest.files.iter().any(|file| {
        effective_enabled(
            file,
            find_override(overrides, bundle_name, &file.kind.package_id()),
        )
    })
}

pub fn effective_enabled(file: &BundleFile, user_override: Option<OverrideType>) -> bool {
    match user_override {
        Some(OverrideType::Removed | OverrideType::Disabled) => false,
        Some(OverrideType::Enabled) => true,
        None => file.enabled,
    }
}

pub(crate) fn find_override(
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
                crate::packages::get_version_cached(ctx, *provider, project_id, version_id).await?
            } else if let Ok(Some((_, version))) = ctx.providers.lookup_version(sha1, ctx).await {
                version
            } else {
                crate::packages::get_version_cached(ctx, *provider, project_id, version_id).await?
            };

            let (artifact, _) = PackageStore::install_to_cluster(
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
        BundleFileKind::External { file: ext, .. } => {
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
pub(crate) async fn install_external(
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
    if !cluster.uses_bundles() {
        return Ok(Vec::new());
    }
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

const DUPLICATE_DISABLE_REPAIR: &str = "repair-hidden-duplicate-disables";

async fn clear_reconciler_disables(
    cluster_id: i64,
    archives: &[BundleArchive],
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let id = format!("{DUPLICATE_DISABLE_REPAIR}:{cluster_id}");
    if migration_dao::is_applied(&ctx.db, &id).await? {
        return Ok(());
    }

    let hidden: std::collections::HashSet<String> = archives
        .iter()
        .flat_map(|archive| &archive.manifest.files)
        .filter(|file| file.hidden)
        .map(|file| file.kind.package_id())
        .collect();

    if hidden.is_empty() {
        return Ok(());
    }

    let mut cleared = 0u64;
    for package_id in &hidden {
        cleared += bundle_dao::clear_disabled_overrides(&ctx.db, cluster_id, package_id).await?;
    }

    if cleared > 0 {
        tracing::info!(
            cluster_id,
            cleared,
            "cleared disabled overrides on hidden bundle files that no user was ever shown"
        );
    }

    migration_dao::mark_applied(&ctx.db, &id).await?;
    Ok(())
}

pub(crate) fn external_ids_by_sha1(
    archives: &[BundleArchive],
) -> std::collections::HashMap<String, String> {
    archives
        .iter()
        .flat_map(|archive| &archive.manifest.files)
        .filter_map(|file| match &file.kind {
            BundleFileKind::External {
                file: ext,
                id: Some(id),
                ..
            } => Some((ext.sha1.clone(), id.clone())),
            _ => None,
        })
        .collect()
}

async fn adopt_external_ids(
    cluster_id: i64,
    archives: &[BundleArchive],
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let ids = external_ids_by_sha1(archives);
    if ids.is_empty() {
        return Ok(());
    }

    for row in bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await? {
        let (Some(bundle_name), Some(version_id), Some(package_id)) =
            (&row.bundle_name, &row.bundle_version_id, &row.package_id)
        else {
            continue;
        };
        if *package_id != row.hash {
            continue;
        }
        let Some(id) = ids.get(package_id) else {
            continue;
        };
        bundle_dao::track_bundle_artifact(
            &ctx.db,
            cluster_id,
            &row.hash,
            bundle_name,
            version_id,
            id,
        )
        .await?;
    }

    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    for row in &overrides {
        let Some(id) = ids.get(&row.package_id) else {
            continue;
        };
        let superseded = overrides
            .iter()
            .any(|other| other.bundle_name == row.bundle_name && other.package_id == *id);
        if !superseded && let Some(override_type) = OverrideType::parse(&row.override_type) {
            bundle_dao::save_override(&ctx.db, cluster_id, &row.bundle_name, id, override_type)
                .await?;
        }
        bundle_dao::remove_override(&ctx.db, cluster_id, &row.bundle_name, &row.package_id).await?;
    }

    Ok(())
}

#[tracing::instrument(level = "debug", skip(archives, ctx))]
pub async fn heal_bundle_activity(
    cluster_id: i64,
    archives: &[BundleArchive],
    ctx: &ContentCtx,
) -> ContentResult<()> {
    clear_reconciler_disables(cluster_id, archives, ctx).await?;
    adopt_external_ids(cluster_id, archives, ctx).await?;

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

    let mut live_packages: std::collections::HashSet<String> = tracked
        .iter()
        .filter(|row| row.enabled != 0)
        .filter_map(|row| row.package_id.clone())
        .collect();

    for row in tracked.iter().filter(|row| row.enabled == 0) {
        let (Some(bundle_name), Some(package_id)) = (&row.bundle_name, &row.package_id) else {
            continue;
        };

        if live_packages.contains(package_id) {
            continue;
        }
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

        live_packages.insert(package_id.clone());
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

    // "Already installed" means the database not disk
    // content lives in the cache between sessions so probing the folder would
    // reinstall everything
    let present = PresentContent::load(cluster_id, ctx).await?;

    let to_install: Vec<BundleFile> = archive
        .manifest
        .files
        .iter()
        .filter(|file| {
            let package_id = file.kind.package_id();
            effective_enabled(file, find_override(&overrides, &bundle_name, &package_id))
                && !present.contains(file)
        })
        .cloned()
        .collect();

    tracing::info!(
        cluster_id,
        bundle = %bundle_name,
        to_install = to_install.len(),
        "installing enabled bundle files"
    );

    let results = install_bundle_files(
        to_install,
        cluster_id,
        &bundle_name,
        skip_compatibility,
        progress,
        ctx,
    )
    .await;

    for (file, result) in results {
        match result {
            Ok(hash) => installed.push(hash),
            Err(err) => {
                tracing::warn!(file = %file.display_name(), error = %err, "failed to install bundle file");
            }
        }
    }

    Ok(installed)
}

pub(crate) async fn install_bundle_files(
    to_install: Vec<BundleFile>,
    cluster_id: i64,
    bundle_name: &str,
    skip_compatibility: bool,
    progress: Option<&GroupedProgressSession>,
    ctx: &ContentCtx,
) -> Vec<(BundleFile, ContentResult<String>)> {
    if let Some(p) = progress {
        let reserved_bytes: u64 = to_install.iter().map(|f| f.size.max(1)).sum();
        p.expect(
            TaskCategory::Packages,
            to_install.len() as u64,
            reserved_bytes,
        );
    }

    futures_util::stream::iter(to_install.into_iter().map(|file| async move {
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
        (file, result)
    }))
    .buffer_unordered(BUNDLE_INSTALL_CONCURRENCY)
    .collect::<Vec<_>>()
    .await
}

pub(crate) struct PresentContent {
    projects: std::collections::HashSet<String>,
    hashes: std::collections::HashSet<String>,
    tracked_ids: std::collections::HashSet<String>,
}

impl PresentContent {
    pub(crate) async fn load(cluster_id: i64, ctx: &ContentCtx) -> ContentResult<Self> {
        let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;
        let tracked_ids = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id)
            .await?
            .into_iter()
            .filter_map(|row| row.package_id)
            .collect();

        Ok(Self {
            projects: linked
                .iter()
                .filter_map(|info| info.project_id.clone())
                .collect(),
            hashes: linked.into_iter().map(|info| info.hash).collect(),
            tracked_ids,
        })
    }

    pub(crate) fn has_hash(&self, hash: &str) -> bool {
        self.hashes.contains(hash)
    }

    pub(crate) fn contains(&self, file: &BundleFile) -> bool {
        match &file.kind {
            BundleFileKind::Managed { project_id, .. } => self.projects.contains(project_id),
            BundleFileKind::External { file: ext, id, .. } => {
                self.hashes.contains(&ext.sha1)
                    || id.as_ref().is_some_and(|id| self.tracked_ids.contains(id))
            }
        }
    }
}

/// Kept modest each fetch also costs a provider API call rate limited per-minute
pub(crate) const BUNDLE_INSTALL_CONCURRENCY: usize = 6;

#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn enabled_bundle_bytes(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<u64> {
    let Some(cluster) = bundle_cluster(cluster_id, ctx).await? else {
        return Ok(0);
    };

    let loader = GameLoader::from_repr(cluster.mc_loader as u8).ok_or_else(|| {
        ContentError::InvalidData {
            reason: format!("unknown loader {}", cluster.mc_loader),
        }
    })?;

    let archives = bundles
        .archives_for(ctx, &cluster.mc_version, loader)
        .await?;
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
    let (live, whole) = taken_bundles(&tracked, &overrides);
    let present = PresentContent::load(cluster_id, ctx).await?;

    let mut total = 0u64;
    for archive in archives.iter().filter(|a| takes_bundle(a, &live)) {
        let bundle_name = &archive.manifest.name;
        let unpicked = unpicked_defaults(archive, &archives, &live, &whole, &overrides);
        for file in &archive.manifest.files {
            let package_id = file.kind.package_id();
            if !effective_enabled(file, find_override(&overrides, bundle_name, &package_id))
                || unpicked.contains(&package_id)
                || present.contains(file)
            {
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
    if bundle_cluster(cluster_id, ctx).await?.is_none() {
        return Ok(());
    }

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
/// installed ones go through [`set_artifact_enabled_to`]
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
        (true, _) => Some(OverrideType::Enabled),
        (false, false) => None,
        (false, true) => Some(OverrideType::Disabled),
    };

    set_bundle_package_override(cluster_id, bundle_name, package_id, override_type, ctx).await
}

#[tracing::instrument(level = "debug", skip(overrides, ctx), fields(count = overrides.len()))]
pub async fn set_bundle_package_overrides(
    cluster_id: i64,
    overrides: &[(String, String, OverrideType)],
    ctx: &ContentCtx,
) -> ContentResult<()> {
    if bundle_cluster(cluster_id, ctx).await?.is_none() {
        return Ok(());
    }

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

#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn enabled_bundle_projects(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<std::collections::HashSet<String>> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    if !cluster.uses_bundles() {
        return Ok(std::collections::HashSet::new());
    }
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).ok_or_else(|| {
        ContentError::InvalidData {
            reason: format!("unknown loader {}", cluster.mc_loader),
        }
    })?;

    let archives = bundles
        .archives_for(ctx, &cluster.mc_version, loader)
        .await?;
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
    let (live, whole) = taken_bundles(&tracked, &overrides);

    let mut projects = std::collections::HashSet::new();
    for archive in archives.iter().filter(|a| takes_bundle(a, &live)) {
        let bundle_name = &archive.manifest.name;
        let unpicked = unpicked_defaults(archive, &archives, &live, &whole, &overrides);
        for file in &archive.manifest.files {
            let BundleFileKind::Managed { project_id, .. } = &file.kind else {
                continue;
            };
            if effective_enabled(file, find_override(&overrides, bundle_name, project_id))
                && !unpicked.contains(project_id)
            {
                projects.insert(project_id.clone());
            }
        }
    }

    Ok(projects)
}

pub(crate) async fn bundle_cluster(
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<Option<ClusterRow>> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    Ok(cluster.uses_bundles().then_some(cluster))
}

#[tracing::instrument(skip(bundles, progress, ctx))]
pub async fn install_cluster_bundles(
    cluster_id: i64,
    bundles: &BundlesManager,
    progress: Option<&GroupedProgressSession>,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    if !cluster.uses_bundles() {
        tracing::debug!(cluster_id, "instance does not take bundle content");
        return Ok(());
    }

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
    heal_bundle_activity(cluster_id, &archives, ctx).await?;
    let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
    decline_unpicked_defaults(cluster_id, &archives, ctx).await?;
    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    let live = super::updates::live_bundle_names(&tracked, &overrides);
    for archive in &archives {
        if !takes_bundle(archive, &live) {
            continue;
        }
        let installed =
            install_enabled_bundle_files(archive, cluster_id, true, progress, ctx).await?;
        let bundle_name = &archive.manifest.name;
        let has_content = !installed.is_empty()
            || tracked.iter().any(|row| {
                row.enabled != 0 && row.bundle_name.as_deref() == Some(bundle_name.as_str())
            });
        if find_override(&overrides, bundle_name, BUNDLE_CONSENT).is_some()
            && (has_content || every_file_switched_off(archive, &overrides))
        {
            bundle_dao::remove_override(&ctx.db, cluster_id, bundle_name, BUNDLE_CONSENT).await?;
        }
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

/// The caller passes the value it wants
/// a relinked artifact keeps its old `enabled` so a flip on an already-correct
/// row puts it wrong
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn set_artifact_enabled_to(
    cluster_id: i64,
    hash: &str,
    enabled: bool,
    ctx: &ContentCtx,
) -> ContentResult<LiveSync> {
    let (_, live) = PackageStore::set_artifact_enabled_to(cluster_id, hash, enabled, ctx).await?;

    if enabled {
        on_user_enable_artifact(cluster_id, hash, ctx).await?;
    } else {
        on_user_disable_artifact(cluster_id, hash, ctx).await?;
    }

    Ok(live)
}

// which clusters have to record what the user just did
async fn override_scope(cluster_id: i64, hash: &str, ctx: &ContentCtx) -> ContentResult<Vec<i64>> {
    Ok(sharing_scope(cluster_id, hash, ctx)
        .await?
        .unwrap_or_else(|| vec![cluster_id]))
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn on_user_disable_artifact(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    for id in override_scope(cluster_id, hash, ctx).await? {
        handle_user_artifact_action(id, hash, ctx, OverrideType::Disabled).await?;
    }

    Ok(())
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn on_user_enable_artifact(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    for id in override_scope(cluster_id, hash, ctx).await? {
        if let Some(tracked) = bundle_dao::get_bundle_tracked(&ctx.db, id, hash).await?
            && let Some(package_id) = tracked.package_id
        {
            clear_suppressing_overrides(id, &package_id, ctx).await?;
        }
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
    let Some(tracked) = bundle_dao::get_bundle_tracked(&ctx.db, cluster_id, hash).await? else {
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

    let link = artifact_dao::get_cluster_artifact(&ctx.db, cluster_id, hash).await?;

    // Database first unconditionally
    // on Windows a jar held open by a running game blocks deleting every hard
    // link which used to fail the whole removal
    // The folder is rebuilt from the database at the next launch
    artifact_dao::unlink_cluster_artifact(&ctx.db, cluster_id, hash).await?;

    // Best-effort folder cleanup failure here is not an error
    let deferred = match (target, link) {
        (Some(content_type), Some(link)) => {
            try_unlink_materialized(&cluster, content_type, &link.cluster_file_name, &ctx.db).await
                == LiveSync::Deferred
        }
        _ => false,
    };

    // The package actually lives in the cache
    // `evict_if_unused` drops it only once no other cluster still needs it
    if !deferred && let Err(err) = evict_if_unused(hash, ctx).await {
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
            let replacement_exists =
                bundle_dao::has_bundle_mapping_for_package(&ctx.db, cluster_id, &package_id)
                    .await?;
            if !replacement_exists {
                clear_suppressing_overrides(cluster_id, &package_id, ctx).await?;
            }
        }
    }

    Ok(())
}

async fn sharing_scope(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<Option<Vec<i64>>> {
    let global = artifact_dao::get_artifact_by_hash(&ctx.db, hash)
        .await?
        .and_then(|artifact| ContentType::from_repr(artifact.content_type as u8))
        .is_some_and(|content_type| content_type.is_global());
    if !global {
        return Ok(None);
    }

    let sharing = cluster_dao::list_oneclient_ids(&ctx.db).await?;

    Ok(sharing.contains(&cluster_id).then_some(sharing))
}

async fn sharing_clusters(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<Option<Vec<i64>>> {
    let Some(sharing) = sharing_scope(cluster_id, hash, ctx).await? else {
        return Ok(None);
    };

    Ok(Some(
        artifact_dao::list_clusters_linking(&ctx.db, hash)
            .await?
            .into_iter()
            .filter(|id| sharing.contains(id))
            .collect(),
    ))
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn clusters_sharing_artifact(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<Option<usize>> {
    Ok(sharing_clusters(cluster_id, hash, ctx)
        .await?
        .map(|clusters| clusters.len()))
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn delete_artifact(cluster_id: i64, hash: &str, ctx: &ContentCtx) -> ContentResult<()> {
    let Some(linked_clusters) = sharing_clusters(cluster_id, hash, ctx).await? else {
        return remove_artifact_from_cluster(cluster_id, hash, true, ctx).await;
    };

    let mut first_error = None;
    for linked_cluster in linked_clusters {
        if let Err(err) = remove_artifact_from_cluster(linked_cluster, hash, true, ctx).await {
            tracing::warn!(cluster_id = linked_cluster, hash, error = %err, "failed to delete shared package from a cluster");
            first_error.get_or_insert(err);
        }
    }

    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundles::types::BundleFileType;
    use oneclient_common::domain::ContentType;
    use oneclient_db::models::ClusterBundleOverrideRow;

    fn file(enabled: bool) -> BundleFile {
        BundleFile {
            enabled,
            hidden: false,
            path: "mods/example.jar".to_string(),
            size: 1,
            file_type: BundleFileType::Normal,
            kind: BundleFileKind::External {
                file: ExternalFile {
                    name: "example.jar".to_string(),
                    url: "https://example.invalid/example.jar".to_string(),
                    sha1: "abc123".to_string(),
                    size: 1,
                    content_type: ContentType::Mod,
                },
                id: None,
                meta: None,
            },
        }
    }

    #[test]
    fn override_none_falls_back_to_manifest_default() {
        assert!(effective_enabled(&file(true), None));
        assert!(!effective_enabled(&file(false), None));
    }

    #[test]
    fn suppressing_overrides_win_over_enabled_manifest() {
        assert!(!effective_enabled(
            &file(true),
            Some(OverrideType::Disabled)
        ));
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

    fn archive(name: &str, enabled: bool, files: Vec<BundleFile>) -> BundleArchive {
        BundleArchive {
            bundle: crate::bundles::Bundle {
                remote_path: format!("/bundles/{name}.mrpack"),
                mc_version: "1.21.11".to_string(),
                loader: GameLoader::Fabric,
                file_name: format!("{name}.mrpack"),
                name: name.to_string(),
                version_id: "1.0.0".to_string(),
                category: name.to_string(),
                loader_version: "0.16.0".to_string(),
                path: std::path::PathBuf::from("/tmp/unused.mrpack"),
                hidden: false,
            },
            manifest: crate::bundles::BundleManifest {
                name: name.to_string(),
                version_id: "1.0.0".to_string(),
                category: name.to_string(),
                mc_version: "1.21.11".to_string(),
                loader: GameLoader::Fabric,
                loader_version: "0.16.0".to_string(),
                enabled,
                java_version_override: None,
                files,
            },
        }
    }

    fn live(rows: &[ClusterBundleOverrideRow]) -> std::collections::HashSet<String> {
        crate::bundles::updates::live_bundle_names(&[], rows)
    }

    #[test]
    fn an_opt_in_bundle_installs_only_on_a_recorded_choice() {
        let opt_in = archive("SkyBlock", false, vec![file(true)]);
        let default = archive("QoL", true, vec![file(true)]);
        let consent = [row("SkyBlock", BUNDLE_CONSENT, OverrideType::Enabled)];

        assert!(!takes_bundle(&opt_in, &live(&[])));
        assert!(takes_bundle(&opt_in, &live(&consent)));
        assert!(takes_bundle(&default, &live(&[])));
        assert!(!takes_bundle(
            &opt_in,
            &live(&[
                row("QoL", BUNDLE_CONSENT, OverrideType::Enabled),
                row("SkyBlock", "abc123", OverrideType::Removed),
            ])
        ));
    }

    #[test]
    fn the_consent_row_leaves_the_files_on_their_catalog_defaults() {
        let rows = [row("SkyBlock", BUNDLE_CONSENT, OverrideType::Enabled)];
        let shipped = file(false);

        assert!(!effective_enabled(
            &shipped,
            find_override(&rows, "SkyBlock", &shipped.kind.package_id())
        ));
    }

    #[test]
    fn consent_is_spent_once_every_file_is_switched_off() {
        let opt_in = archive("SkyBlock", false, vec![named("main")]);
        let switched_off = [row("SkyBlock", "main", OverrideType::Removed)];

        assert!(!every_file_switched_off(&opt_in, &[]));
        assert!(every_file_switched_off(&opt_in, &switched_off));
    }

    fn named(sha1: &str) -> BundleFile {
        let mut named = file(true);
        if let BundleFileKind::External { file, .. } = &mut named.kind {
            file.sha1 = sha1.to_string();
        }
        named
    }

    fn tracked(bundle: &str, enabled: bool) -> oneclient_db::models::BundleTrackedArtifactRow {
        oneclient_db::models::BundleTrackedArtifactRow {
            cluster_id: 1,
            hash: "h".to_string(),
            cluster_file_name: "f.jar".to_string(),
            enabled: i64::from(enabled),
            bundle_name: Some(bundle.to_string()),
            bundle_version_id: None,
            package_id: None,
            installed_at: None,
        }
    }

    #[test]
    fn picking_one_file_of_an_opt_in_bundle_declines_its_other_defaults() {
        let mut lib = named("lib");
        lib.hidden = true;
        let mut extra = named("extra");
        extra.enabled = false;
        let archives = [
            archive(
                "SkyBlock",
                false,
                vec![named("main"), named("shared"), extra, lib],
            ),
            archive("QoL", true, vec![named("shared")]),
        ];
        let pick = [row("SkyBlock", "extra", OverrideType::Enabled)];
        let unpicked = |tracked: &[_], rows: &[_]| {
            let (live, whole) = taken_bundles(tracked, rows);
            unpicked_defaults(&archives[0], &archives, &live, &whole, rows)
        };

        assert_eq!(unpicked(&[], &pick), ["main".to_string()].into());
        let consented = [
            pick[0].clone(),
            row("SkyBlock", BUNDLE_CONSENT, OverrideType::Enabled),
        ];
        assert!(unpicked(&[], &consented).is_empty());
        assert!(unpicked(&[tracked("SkyBlock", true)], &pick).is_empty());
    }

    #[test]
    fn a_migration_target_inherits_the_bundles_its_source_took_whole() {
        let source_tracked = [tracked("Installed", true), tracked("Emptied", false)];
        let source_rows = [
            row("Consented", BUNDLE_CONSENT, OverrideType::Enabled),
            row("Picked", "extra", OverrideType::Enabled),
            row("Declined here", BUNDLE_CONSENT, OverrideType::Enabled),
        ];
        let target_rows = [row("Declined here", "main", OverrideType::Removed)];
        let none = std::collections::HashSet::new();

        let names: Vec<String> =
            inherited_consents(&source_tracked, &source_rows, &[], &target_rows, &none)
                .into_iter()
                .map(|(name, ..)| name)
                .collect();
        assert_eq!(names, ["Consented", "Installed"]);
        assert!(
            inherited_consents(
                &source_tracked[..1],
                &[],
                &[tracked("Installed", false)],
                &[],
                &none
            )
            .is_empty()
        );
        let flag = row(BUNDLE_CONSENT, BUNDLE_CONSENT, OverrideType::Enabled);
        assert_eq!(
            inherited_consents(&[], std::slice::from_ref(&flag), &[], &[], &none),
            vec![(
                BUNDLE_CONSENT.to_string(),
                BUNDLE_CONSENT.to_string(),
                OverrideType::Enabled
            )]
        );
    }

    #[test]
    fn inherited_consent_carries_what_the_source_and_the_prompt_declined() {
        let mut left_behind = tracked("SkyBlock", true);
        left_behind.package_id = Some("unticked".to_string());
        let source_rows = [
            row("SkyBlock", "removed", OverrideType::Removed),
            row("SkyBlock", "picked", OverrideType::Enabled),
            row("Other", "x", OverrideType::Removed),
        ];
        let declined = [left_behind.hash.clone()].into();
        let removed = |id: &str| {
            (
                "SkyBlock".to_string(),
                id.to_string(),
                OverrideType::Removed,
            )
        };

        assert_eq!(
            inherited_consents(&[left_behind], &source_rows, &[], &[], &declined),
            vec![
                removed("removed"),
                removed("unticked"),
                (
                    "SkyBlock".to_string(),
                    BUNDLE_CONSENT.to_string(),
                    OverrideType::Enabled
                ),
            ]
        );
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
