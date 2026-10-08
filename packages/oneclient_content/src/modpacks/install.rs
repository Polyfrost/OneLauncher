use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::{
    ArtifactRow, ClusterBundleOverrideRow, ClusterRow, OverrideType, SeenStatus,
};
use oneclient_events::{GroupedProgressSession, TaskCategory, TaskPhase};

use futures_util::StreamExt;

use super::{
    BlockedFile, LooseFile, ModpackContents, ModpackManifest, WantedFile, file_sha1,
    loose_lock_key, tracked_content_type,
};
use crate::bundles::install::{
    PresentContent, effective_enabled, find_override, install_bundle_files, install_external,
    remove_artifact_from_cluster,
};
use crate::bundles::overrides::{
    OverrideLayers, lock_entries, sync_file_lock, sync_layered_overrides_tracked,
};
use crate::bundles::{BundleFile, BundleFileKind};
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};
use crate::packages::PackageError;
use crate::packages::store::{self, PackageStore};
use crate::packages::types::ExternalFile;
use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_common::paths;

const LOOSE_CONCURRENCY: usize = 6;

#[derive(Debug, Clone, Default)]
pub struct ModpackInstallReport {
    pub installed: usize,
    pub failed: Vec<String>,
    pub blocked: Vec<BlockedFile>,
    pub kept: Vec<String>,
    pub disabled_duplicates: Vec<String>,
    pub disabled_hashes: Vec<String>,
    pub wanted_present: Vec<WantedFile>,
}

impl ModpackInstallReport {
    pub fn absorb(&mut self, later: Self) {
        self.installed += later.installed;
        for name in later.failed {
            if !self.failed.contains(&name) {
                self.failed.push(name);
            }
        }
        self.blocked = later.blocked;
        self.kept = later.kept;
        for name in later.disabled_duplicates {
            if !self.disabled_duplicates.contains(&name) {
                self.disabled_duplicates.push(name);
            }
        }
        for hash in later.disabled_hashes {
            if !self.disabled_hashes.contains(&hash) {
                self.disabled_hashes.push(hash);
            }
        }
        self.wanted_present = later.wanted_present;
    }
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn store_modpack_archive(path: &Path, ctx: &ContentCtx) -> ContentResult<ArtifactRow> {
    store::cache_local_file(path, ContentType::Modpack, ctx).await
}

struct InstallContext<'a> {
    manifest: &'a ModpackManifest,
    bundle_name: &'a str,
    cluster: &'a ClusterRow,
    present: &'a PresentContent,
    ctx: &'a ContentCtx,
}

#[tracing::instrument(skip(manifest, progress, ctx), fields(pack = %manifest.name))]
pub async fn install_modpack(
    archive_path: &Path,
    manifest: &ModpackManifest,
    cluster_id: i64,
    bundle_name: &str,
    progress: Option<&GroupedProgressSession>,
    ctx: &ContentCtx,
) -> ContentResult<ModpackInstallReport> {
    let contents = &manifest.contents;
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let root = paths::cluster_game_dir(&cluster.folder_name, cluster.is_isolated())?;

    let overrides = bundle_dao::list_overrides(&ctx.db, cluster_id).await?;
    let present = PresentContent::load(cluster_id, ctx).await?;
    let tracked: HashMap<String, (String, String)> =
        bundle_dao::list_bundle_tracked(&ctx.db, cluster_id)
            .await?
            .into_iter()
            .filter(|row| row.bundle_name.as_deref() == Some(bundle_name))
            .filter_map(|row| Some((row.package_id?, (row.hash, row.bundle_version_id?))))
            .collect();

    let mut replaced: HashMap<String, String> = HashMap::new();
    let to_install = contents
        .files
        .iter()
        .filter(|file| {
            let package_id = file.kind.package_id();
            let user_choice = find_override(&overrides, bundle_name, &package_id);
            if user_choice == Some(OverrideType::Removed) {
                return false;
            }
            match tracked.get(&package_id) {
                Some((_, version)) if *version == file.kind.bundle_version_id() => false,
                Some((hash, _)) => {
                    replaced.insert(package_id, hash.clone());
                    true
                }
                None => {
                    effective_enabled(file, user_choice)
                        && !present.contains(file)
                        && !present.has_hash(file_sha1(file))
                }
            }
        })
        .cloned()
        .collect();

    let mut report = ModpackInstallReport::default();
    for file in &contents.files {
        let package_id = file.kind.package_id();
        if tracked.contains_key(&package_id) {
            continue;
        }
        let user_choice = find_override(&overrides, bundle_name, &package_id);
        if user_choice == Some(OverrideType::Removed) || !effective_enabled(file, user_choice) {
            continue;
        }
        if present.contains(file) || present.has_hash(file_sha1(file)) {
            report.wanted_present.push(WantedFile {
                sha1: file_sha1(file).to_string(),
                package_id,
                version_id: file.kind.bundle_version_id(),
            });
        }
    }
    let install = InstallContext {
        manifest,
        bundle_name,
        cluster: &cluster,
        present: &present,
        ctx,
    };

    let results =
        install_bundle_files(to_install, cluster_id, bundle_name, true, progress, ctx).await;
    for (file, installed) in results {
        let replaced = replaced.remove(&file.kind.package_id());
        let status = if replaced.is_some() {
            SeenStatus::Updated
        } else {
            SeenStatus::New
        };
        match settle_file(&install, &file, installed, replaced).await {
            Ok(hash) => {
                report.installed += 1;
                mark_seen(bundle_name, cluster_id, &hash, status, ctx).await;
            }
            Err(err) => {
                let name = file.display_name();
                tracing::warn!(file = %name, error = %err, "failed to install modpack file");
                report.failed.push(name);
            }
        }
    }

    let imported = super::is_imported_bundle(bundle_name);
    let lock_key = super::lock_key(&cluster, bundle_name);
    let loose_key = loose_lock_key(&lock_key);
    let owned = if imported {
        lock_entries(&root, &loose_key).await
    } else {
        HashMap::new()
    };
    let mut to_write = Vec::new();
    for file in &contents.loose {
        if imported && !owned.contains_key(&file.path) {
            let dest = root.join(polyio::sanitize_path(&file.path));
            if polyio::try_exists(&dest).await.unwrap_or(false) {
                if !oneclient_net::matches_on_disk(&dest, &file.sha1).await {
                    report.kept.push(file.path.clone());
                }
                continue;
            }
        }
        to_write.push(file.clone());
    }

    let game_dir = root.as_path();
    let loose = futures_util::stream::iter(to_write.into_iter().map(|file| async move {
        let result = install_loose_file(&file, game_dir, progress, ctx).await;
        (file, result)
    }))
    .buffer_unordered(LOOSE_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;
    let mut written = HashMap::new();
    for (file, result) in loose {
        match result {
            Ok(()) => {
                written.insert(file.path.clone(), file.sha1.clone());
            }
            Err(err) => {
                tracing::warn!(path = %file.path, error = %err, "failed to install modpack file");
                report.failed.push(file.path.clone());
            }
        }
    }
    let listed: HashSet<String> = contents
        .loose
        .iter()
        .map(|file| file.path.clone())
        .collect();
    sync_file_lock(&root, &loose_key, written, &listed).await;

    let prefixes: Vec<&str> = manifest
        .override_prefixes
        .iter()
        .map(String::as_str)
        .collect();
    let synced = sync_layered_overrides_tracked(
        archive_path,
        &lock_key,
        &root,
        &prefixes,
        &|rel| tracked_content_type(rel).is_none(),
        !imported,
        (!imported).then_some(&ctx.events),
        progress,
    )
    .await?;
    if imported {
        report.kept.extend(synced.conflicts);
    }

    let bundled =
        import_override_content(archive_path, &prefixes, &install, &overrides, &mut report).await?;

    remove_dropped_files(contents, &tracked, bundled, cluster_id, ctx).await;

    for file in &contents.blocked {
        let suppressed = matches!(
            find_override(&overrides, bundle_name, &file.project_id),
            Some(OverrideType::Removed | OverrideType::Disabled)
        );
        if suppressed {
            continue;
        }
        if present.has_hash(&file.sha1) {
            report.wanted_present.push(WantedFile {
                sha1: file.sha1.clone(),
                package_id: file.project_id.clone(),
                version_id: file.version_id.clone(),
            });
            continue;
        }

        match cached_blocked_file(file, ctx).await {
            Some(row) => match link_blocked_file(&cluster, bundle_name, file, &row, ctx).await {
                Ok(()) => {
                    report.installed += 1;
                    mark_seen(bundle_name, cluster_id, &row.hash, SeenStatus::New, ctx).await;
                }
                Err(err) => {
                    tracing::warn!(file = %file.file_name, error = %err, "could not reuse a cached manual download");
                    report.blocked.push(file.clone());
                }
            },
            None => report.blocked.push(file.clone()),
        }
    }

    report.failed.extend(contents.unresolved.iter().cloned());

    match super::duplicates::disable_older_duplicates(cluster_id, bundle_name, ctx).await {
        Ok(disabled) => {
            for copy in disabled {
                if !copy.from_pack {
                    report.disabled_hashes.push(copy.hash);
                }
                report.disabled_duplicates.push(copy.name);
            }
        }
        Err(err) => tracing::warn!(cluster_id, error = %err, "could not check for duplicate mods"),
    }

    tracing::info!(
        cluster_id,
        installed = report.installed,
        failed = report.failed.len(),
        blocked = report.blocked.len(),
        "modpack install finished"
    );

    Ok(report)
}

async fn mark_seen(
    bundle_name: &str,
    cluster_id: i64,
    hash: &str,
    status: SeenStatus,
    ctx: &ContentCtx,
) {
    if !super::is_imported_bundle(bundle_name) {
        return;
    }
    if let Err(err) = artifact_dao::set_seen_status(&ctx.db, cluster_id, hash, status).await {
        tracing::debug!(hash, error = %err, "could not mark a modpack file as new");
    }
}

async fn settle_file(
    install: &InstallContext<'_>,
    file: &BundleFile,
    installed: ContentResult<String>,
    replaced: Option<String>,
) -> ContentResult<String> {
    let hash = match installed.and_then(|hash| verify_installed(file, hash)) {
        Ok(hash) => hash,
        Err(err) => {
            unlink_mismatch(install, &err).await;
            let Some(direct) = direct_download(file, install.manifest) else {
                return Err(err);
            };
            tracing::info!(
                file = %file.display_name(),
                error = %err,
                "provider lookup failed, downloading the modpack's own link instead"
            );
            install_direct(install, file, direct).await?
        }
    };

    let cluster_id = install.cluster.id;
    let ctx = install.ctx;
    match replaced {
        Some(old) if old != hash => {
            let was_enabled = artifact_dao::get_cluster_artifact(&ctx.db, cluster_id, &old)
                .await?
                .is_none_or(|link| link.enabled != 0);
            remove_artifact_from_cluster(cluster_id, &old, false, ctx).await?;
            if !was_enabled {
                PackageStore::set_artifact_enabled_to(cluster_id, &hash, false, ctx).await?;
            }
        }
        Some(_) => {}
        None if install
            .manifest
            .contents
            .optional
            .contains(&file.kind.package_id()) =>
        {
            PackageStore::set_artifact_enabled_to(cluster_id, &hash, false, ctx).await?;
        }
        None => {}
    }
    Ok(hash)
}

fn verify_installed(file: &BundleFile, hash: String) -> ContentResult<String> {
    let expected = file_sha1(file);
    if expected.is_empty() || expected == hash {
        return Ok(hash);
    }

    tracing::warn!(
        file = %file.display_name(),
        expected,
        actual = %hash,
        "the provider served a different file than the modpack lists"
    );
    Err(PackageError::HashMismatch {
        expected: expected.to_string(),
        actual: hash,
    }
    .into())
}

async fn unlink_mismatch(install: &InstallContext<'_>, err: &ContentError) {
    let ContentError::Package(PackageError::HashMismatch { actual, .. }) = err else {
        return;
    };
    if install.present.has_hash(actual) {
        return;
    }
    if let Err(err) =
        remove_artifact_from_cluster(install.cluster.id, actual, false, install.ctx).await
    {
        tracing::warn!(error = %err, "could not remove the mismatched file");
    }
}

async fn remove_dropped_files(
    contents: &ModpackContents,
    tracked: &HashMap<String, (String, String)>,
    bundled: Vec<String>,
    cluster_id: i64,
    ctx: &ContentCtx,
) {
    if !contents.unresolved.is_empty() {
        tracing::warn!(
            cluster_id,
            unresolved = contents.unresolved.len(),
            "not removing dropped files because part of the modpack could not be resolved"
        );
        return;
    }

    let mut listed: HashSet<String> = contents
        .files
        .iter()
        .map(|file| file.kind.package_id())
        .collect();
    listed.extend(contents.blocked.iter().map(|file| file.project_id.clone()));
    listed.extend(bundled);

    for (package_id, (hash, _)) in tracked {
        if listed.contains(package_id) {
            continue;
        }
        if let Err(err) = remove_artifact_from_cluster(cluster_id, hash, false, ctx).await {
            tracing::warn!(package_id, error = %err, "could not remove a file the modpack dropped");
        }
    }
}

fn direct_download<'a>(
    file: &BundleFile,
    manifest: &'a ModpackManifest,
) -> Option<&'a ExternalFile> {
    match &file.kind {
        BundleFileKind::Managed { sha1, .. } => manifest.contents.direct.get(sha1),
        BundleFileKind::External { .. } => None,
    }
}

async fn install_direct(
    install: &InstallContext<'_>,
    file: &BundleFile,
    direct: &ExternalFile,
) -> ContentResult<String> {
    let ctx = install.ctx;
    let hash = install_external(direct, install.cluster, true, None, ctx).await?;
    bundle_dao::track_bundle_artifact(
        &ctx.db,
        install.cluster.id,
        &hash,
        install.bundle_name,
        &file.kind.bundle_version_id(),
        &file.kind.package_id(),
    )
    .await?;
    Ok(hash)
}

async fn import_override_content(
    archive_path: &Path,
    prefixes: &[&str],
    install: &InstallContext<'_>,
    overrides: &[ClusterBundleOverrideRow],
    report: &mut ModpackInstallReport,
) -> ContentResult<Vec<String>> {
    let mut layers = OverrideLayers::open(archive_path, prefixes).await?;
    let bundled: Vec<(String, String, ContentType)> = layers
        .entries()
        .iter()
        .filter_map(|(rel, entry)| {
            let content_type = tracked_content_type(rel)?;
            Some((rel.clone(), entry.clone(), content_type))
        })
        .collect();
    let mut hashes = Vec::new();
    if bundled.is_empty() {
        return Ok(hashes);
    }

    let staging = polyio::tempdir().await?;
    let mut files = Vec::new();

    for (rel, entry, content_type) in bundled {
        let Some(bytes) = layers.read(&entry).await else {
            report.failed.push(rel);
            continue;
        };
        let hash = polyio::normalize_hash(&polyio::sha1_bytes(&bytes));
        hashes.push(hash.clone());
        let removed = matches!(
            find_override(overrides, install.bundle_name, &hash),
            Some(OverrideType::Removed | OverrideType::Disabled)
        );
        if removed {
            continue;
        }
        if install.present.has_hash(&hash) {
            report.wanted_present.push(WantedFile {
                sha1: hash.clone(),
                package_id: hash.clone(),
                version_id: hash.clone(),
            });
            continue;
        }

        let Some(file_name) = rel.rsplit('/').next() else {
            continue;
        };
        let path = staging
            .dir_path()
            .join(content_type.folder_name())
            .join(file_name);
        if let Some(parent) = path.parent() {
            polyio::create_dir_all(parent).await?;
        }
        polyio::write(&path, bytes).await?;
        files.push((path, content_type));
    }

    let ctx = install.ctx;
    let imported = PackageStore::import_local_files(&files, install.cluster.id, ctx).await?;

    for row in &imported.imported {
        bundle_dao::track_bundle_artifact(
            &ctx.db,
            install.cluster.id,
            &row.hash,
            install.bundle_name,
            &row.hash,
            &row.hash,
        )
        .await?;
        report.installed += 1;
        mark_seen(
            install.bundle_name,
            install.cluster.id,
            &row.hash,
            SeenStatus::New,
            ctx,
        )
        .await;
    }

    for (path, err) in imported.failed {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        tracing::warn!(file = %name, error = %err, "failed to import a file bundled in the modpack");
        report.failed.push(name);
    }

    Ok(hashes)
}

async fn install_loose_file(
    file: &LooseFile,
    root: &Path,
    progress: Option<&GroupedProgressSession>,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let dest = root.join(polyio::sanitize_path(&file.path));
    let child = progress.map(|p| {
        let child = p.child(file.path.clone(), file.size.max(1), TaskCategory::Packages);
        child.set_phase(TaskPhase::Downloading);
        child
    });

    let result = store::ensure_artifact_file(&file.sha1, &file.url, &dest, child.as_ref(), ctx)
        .await
        .map(|_| ());

    if let Some(child) = child {
        child.finish();
    }
    result
}

#[tracing::instrument(level = "debug", skip(blocked))]
pub async fn find_blocked_downloads(
    locations: &[PathBuf],
    blocked: &[BlockedFile],
) -> Vec<(PathBuf, BlockedFile)> {
    let mut by_size: HashMap<u64, Vec<&BlockedFile>> = HashMap::new();
    for file in blocked {
        by_size.entry(file.size).or_default().push(file);
    }

    let mut found: Vec<(PathBuf, BlockedFile)> = Vec::new();
    for path in candidate_files(locations).await {
        let Ok(meta) = polyio::stat(&path).await else {
            continue;
        };
        let Some(candidates) = by_size.get(&meta.len()) else {
            continue;
        };
        let Ok(sha1) = polyio::sha1_file(&path).await else {
            continue;
        };
        let sha1 = polyio::normalize_hash(&sha1);

        if let Some(file) = candidates.iter().find(|file| file.sha1 == sha1)
            && !found.iter().any(|(_, known)| known.sha1 == sha1)
        {
            found.push((path, (*file).clone()));
        }
    }

    found
}

async fn candidate_files(locations: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for location in locations {
        let Ok(meta) = polyio::stat(location).await else {
            continue;
        };
        if meta.is_file() {
            files.push(location.clone());
            continue;
        }

        let Ok(mut entries) = polyio::read_dir(location).await else {
            continue;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.file_type().await.is_ok_and(|kind| kind.is_file()) {
                files.push(entry.path());
            }
        }
    }
    files
}

#[tracing::instrument(level = "debug", skip(found, ctx), fields(files = found.len()))]
pub async fn import_blocked_files(
    cluster_id: i64,
    bundle_name: &str,
    found: &[(PathBuf, BlockedFile)],
    ctx: &ContentCtx,
) -> ContentResult<Vec<String>> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let mut imported = Vec::new();

    for (path, file) in found {
        let row = store::cache_local_file(path, file.content_type, ctx).await?;
        if row.hash != file.sha1 {
            return Err(PackageError::HashMismatch {
                expected: file.sha1.clone(),
                actual: row.hash,
            }
            .into());
        }

        link_blocked_file(&cluster, bundle_name, file, &row, ctx).await?;
        mark_seen(bundle_name, cluster_id, &row.hash, SeenStatus::New, ctx).await;
        imported.push(row.hash);
    }

    Ok(imported)
}

async fn link_blocked_file(
    cluster: &ClusterRow,
    bundle_name: &str,
    file: &BlockedFile,
    row: &ArtifactRow,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    PackageStore::link_artifact(row, cluster, Some(&file.file_name), ctx).await?;
    bundle_dao::track_bundle_artifact(
        &ctx.db,
        cluster.id,
        &row.hash,
        bundle_name,
        &file.version_id,
        &file.project_id,
    )
    .await?;

    if artifact_dao::get_release_by_hash(&ctx.db, &row.hash)
        .await?
        .is_some()
    {
        return Ok(());
    }

    match crate::packages::get_version_cached(
        ctx,
        ProviderId::CurseForge,
        &file.project_id,
        &file.version_id,
    )
    .await
    {
        Ok(version) => {
            if let Err(err) =
                store::record_release(ProviderId::CurseForge, &version, &row.hash, ctx).await
            {
                tracing::debug!(file = %file.file_name, error = %err, "could not record the release");
            }
        }
        Err(err) => {
            tracing::debug!(file = %file.file_name, error = %err, "could not describe the manual download");
        }
    }

    Ok(())
}

async fn cached_blocked_file(file: &BlockedFile, ctx: &ContentCtx) -> Option<ArtifactRow> {
    let row = artifact_dao::get_artifact_by_hash(&ctx.db, &file.sha1)
        .await
        .ok()
        .flatten()?;
    let path = store::artifact_absolute_path(&row.path).ok()?;
    polyio::try_exists(&path)
        .await
        .unwrap_or(false)
        .then_some(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocked(sha1: &str, size: u64) -> BlockedFile {
        BlockedFile {
            project_id: "1".into(),
            version_id: "2".into(),
            project_name: "Thing".into(),
            file_name: "thing.jar".into(),
            path: "mods/thing.jar".into(),
            sha1: sha1.into(),
            size,
            content_type: ContentType::Mod,
            page_url: None,
        }
    }

    #[tokio::test]
    async fn downloads_are_matched_by_hash_not_by_name() {
        let dir = polyio::testing::ScratchDir::new("blocked-downloads");
        let wanted = b"the real jar";
        let renamed = dir.path().join("thing (1).jar");
        tokio::fs::write(&renamed, wanted).await.unwrap();
        tokio::fs::write(dir.path().join("thing.jar"), b"another file!")
            .await
            .unwrap();

        let sha1 = polyio::sha1_bytes(wanted);
        let found = find_blocked_downloads(
            &[dir.path().to_path_buf()],
            &[blocked(&sha1, wanted.len() as u64)],
        )
        .await;

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, renamed);

        let picked =
            find_blocked_downloads(&[renamed.clone()], &[blocked(&sha1, wanted.len() as u64)])
                .await;
        assert_eq!(picked.len(), 1);
    }
}
