mod download;
mod gc;
mod link;
pub mod manifest;
mod paths;

pub use download::{download_external, download_version_file, ensure_artifact_file};
pub use gc::{
    GcReport, collect_unused_artifacts, evict_if_unused, find_unreferenced_files,
    remove_unreferenced_files,
};
pub use link::{
    LiveSync, link_or_copy, remove_entry, sweep_staging_files, try_link_materialized,
    try_unlink_materialized,
};
pub use paths::{artifact_absolute_path, cache_file_path, relative_cache_path};

use oneclient_db::dao::{
    artifact as artifact_dao, cluster as cluster_dao, package_metadata as meta_dao,
};
use oneclient_db::models::{ArtifactRow, ClusterRow, SeenStatus};

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
// `paths` alone is this module's own cache-path helpers
use oneclient_common::paths as common_paths;
use super::error::PackageError;
use super::file_identity::FileIdentity;
use super::{local_manifest, metadata_cache};
use super::types::{CachedArtifact, ProjectDetail, ProviderReleaseInfo, VersionDetail, LinkedArtifactInfo};
use polyio::{normalize_hash, sha1_file};
use oneclient_events::GroupedProgressChild;
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};
use std::path::{Path, PathBuf};

pub struct PackageStore;

/// One bad file in a selection must not sink the rest so failures ride along
/// with the successes instead of replacing them
#[derive(Debug, Default)]
pub struct LocalImportReport {
    pub imported: Vec<ArtifactRow>,
    pub failed: Vec<(PathBuf, ContentError)>,
}

impl PackageStore {
    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn get_cluster(
        cluster_id: i64,
        ctx: &ContentCtx,
    ) -> ContentResult<ClusterRow> {
        cluster_dao::get_by_id(&ctx.db, cluster_id)
            .await?
            .ok_or(PackageError::ClusterNotFound(cluster_id).into())
    }

    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn cached_artifact(
        hash: &str,
        ctx: &ContentCtx,
    ) -> ContentResult<Option<CachedArtifact>> {
        let Some(row) = artifact_dao::get_artifact_by_hash(&ctx.db, hash).await? else {
            return Ok(None);
        };

        Ok(Some(row_to_cached(row, ctx).await?))
    }

    #[tracing::instrument(level = "debug", skip(project, version, child, ctx), fields(project_id = %project.id, version_id = %version.version_id))]
    pub async fn download_and_cache(
        provider_id: ProviderId,
        project: &ProjectDetail,
        version: &VersionDetail,
        force: bool,
        child: Option<&GroupedProgressChild>,
        ctx: &ContentCtx,
    ) -> ContentResult<ArtifactRow> {
        let file = version.primary_file().ok_or(PackageError::NoPrimaryFile)?;

        download::download_version_file(
            provider_id,
            &project.id,
            version,
            project.content_type,
            file,
            force,
            child,
            ctx,
        )
        .await
    }

	#[allow(clippy::too_many_arguments)]
    #[tracing::instrument(skip(project, version, child, ctx), fields(project_id = %project.id, version_id = %version.version_id))]
    pub async fn install_to_cluster(
        provider_id: ProviderId,
        project: &ProjectDetail,
        version: &VersionDetail,
        cluster_id: i64,
        skip_compatibility: bool,
        force_download: bool,
        child: Option<&GroupedProgressChild>,
        ctx: &ContentCtx,
    ) -> ContentResult<(ArtifactRow, LiveSync)> {
        tracing::info!("installing package to cluster");
        let cluster = Self::get_cluster(cluster_id, ctx).await?;

        if !skip_compatibility {
            ensure_compatible(project, version, &cluster)?;
        }

        let artifact = Self::download_and_cache(
            provider_id,
            project,
            version,
            force_download,
            child,
            ctx,
        )
        .await?;

        let enabled = Self::link_artifact(&artifact, &cluster, None, ctx).await?;

        let live = if enabled {
            link::try_link_materialized(&cluster, &artifact, &artifact.file_name).await
        } else {
            LiveSync::Skipped
        };

        Ok((artifact, live))
    }

    #[tracing::instrument(level = "debug", skip(artifact, cluster, ctx))]
    pub async fn link_artifact(
        artifact: &ArtifactRow,
        cluster: &ClusterRow,
        cluster_file_name: Option<&str>,
        ctx: &ContentCtx,
    ) -> ContentResult<bool> {
        let name = cluster_file_name.unwrap_or(&artifact.file_name);

        let link =
            artifact_dao::link_cluster_artifact(&ctx.db, cluster.id, &artifact.hash, name).await?;

        Ok(link.enabled != 0)
    }

    #[tracing::instrument(level = "debug", skip(artifact, ctx), fields(hash = %artifact.hash))]
    pub async fn sync_live_content(
        cluster_id: i64,
        artifact: &ArtifactRow,
        ctx: &ContentCtx,
    ) -> ContentResult<LiveSync> {
        let Some(link) =
            artifact_dao::get_cluster_artifact(&ctx.db, cluster_id, &artifact.hash).await?
        else {
            return Ok(LiveSync::Skipped);
        };

        if link.enabled == 0 {
            return Ok(LiveSync::Skipped);
        }

        let cluster = Self::get_cluster(cluster_id, ctx).await?;

        Ok(link::try_link_materialized(&cluster, artifact, &link.cluster_file_name).await)
    }

    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn link_by_hash(
        hash: &str,
        cluster_id: i64,
        cluster_file_name: Option<&str>,
        ctx: &ContentCtx,
    ) -> ContentResult<()> {
        let artifact = artifact_dao::get_artifact_by_hash(&ctx.db, hash)
            .await?
            .ok_or(PackageError::ArtifactMissing(hash.to_string()))?;

        let cluster = Self::get_cluster(cluster_id, ctx).await?;

        Self::link_artifact(&artifact, &cluster, cluster_file_name, ctx)
            .await
            .map(|_| ())
    }

    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn list_linked_artifacts(
        cluster_id: i64,
        ctx: &ContentCtx,
    ) -> ContentResult<Vec<LinkedArtifactInfo>> {
        let links = artifact_dao::list_cluster_artifacts(&ctx.db, cluster_id).await?;
        let mut items = Vec::with_capacity(links.len());

        for link in links {
            let Some(artifact) =
                artifact_dao::get_artifact_by_hash(&ctx.db, &link.hash).await?
            else {
                continue;
            };

            let content_type = ContentType::from_repr(artifact.content_type as u8)
                .unwrap_or(ContentType::Mod);
            let release = artifact_dao::get_release_by_hash(&ctx.db, &link.hash).await?;
            let seen_status = link.status();

            items.push(LinkedArtifactInfo {
                hash: link.hash,
                cluster_file_name: link.cluster_file_name,
                enabled: link.enabled != 0,
                content_type,
                file_name: artifact.file_name,
                project_id: release.as_ref().map(|r| r.project_id.clone()),
                version_id: release.as_ref().map(|r| r.version_id.clone()),
                display_name: release.as_ref().map(|r| r.display_name.clone()),
                display_version: release.as_ref().map(|r| r.display_version.clone()),
                provider: release
                    .as_ref()
                    .and_then(|r| ProviderId::from_repr(r.provider as u8)),
                published_at: release.as_ref().and_then(|r| r.published_at.clone()),
                seen_status,
            });
        }

        Ok(items)
    }

    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn mark_artifact_new(
        cluster_id: i64,
        hash: &str,
        ctx: &ContentCtx,
    ) -> ContentResult<()> {
        artifact_dao::set_seen_status(&ctx.db, cluster_id, hash, SeenStatus::New).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn retire_seen_badges(ctx: &ContentCtx) -> ContentResult<u64> {
        Ok(artifact_dao::mark_all_seen(&ctx.db).await?)
    }

    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn set_artifact_enabled(
        cluster_id: i64,
        hash: &str,
        ctx: &ContentCtx,
    ) -> ContentResult<(bool, LiveSync)> {
        Self::write_artifact_enabled(cluster_id, hash, None, ctx).await
    }

    /// Returns the state it ended in which is the requested one unless the link was already there
    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn set_artifact_enabled_to(
        cluster_id: i64,
        hash: &str,
        enabled: bool,
        ctx: &ContentCtx,
    ) -> ContentResult<bool> {
        Self::write_artifact_enabled(cluster_id, hash, Some(enabled), ctx)
            .await
            .map(|(enabled, _)| enabled)
    }

    /// `target` of `None` means "the opposite of whatever it is now"
    async fn write_artifact_enabled(
        cluster_id: i64,
        hash: &str,
        target: Option<bool>,
        ctx: &ContentCtx,
    ) -> ContentResult<(bool, LiveSync)> {
        let cluster = Self::get_cluster(cluster_id, ctx).await?;
        let artifact = artifact_dao::get_artifact_by_hash(&ctx.db, hash)
            .await?
            .ok_or(PackageError::ArtifactMissing(hash.to_string()))?;

        let link = artifact_dao::get_cluster_artifact(&ctx.db, cluster_id, hash)
            .await?
            .ok_or(PackageError::ArtifactMissing(hash.to_string()))?;

        let content_type = ContentType::from_repr(artifact.content_type as u8)
            .ok_or_else(|| ContentError::InvalidData {
                reason: format!("unknown content type {}", artifact.content_type),
            })?;

        let enabled = target.unwrap_or(link.enabled == 0);
        let file_name = link
            .cluster_file_name
            .trim_end_matches(".disabled")
            .to_string();

        artifact_dao::update_cluster_artifact(
            &ctx.db,
            cluster_id,
            hash,
            &file_name,
            i64::from(enabled),
        )
        .await?;

        // Only the enable side has an outcome to report; a pack that is not in
        // the running folder needs no removing from it
        let live = if enabled {
            link::try_link_materialized(&cluster, &artifact, &file_name).await
        } else {
            link::try_unlink_materialized(&cluster, content_type, &link.cluster_file_name).await;
            if link.cluster_file_name != file_name {
                link::try_unlink_materialized(&cluster, content_type, &file_name).await;
            }
            LiveSync::Skipped
        };

        Ok((enabled, live))
    }

    /// Prefer [`Self::import_local_files`] for anything the user selected in one
    /// go it asks the providers about the whole set in a single request
    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn import_local_file(
        path: &Path,
        content_type: ContentType,
        cluster_id: i64,
        ctx: &ContentCtx,
    ) -> ContentResult<ArtifactRow> {
        let cluster = Self::get_cluster(cluster_id, ctx).await?;
        let row = store_local_file(path, content_type, &cluster, ctx).await?;

        describe_imports(&[(row.clone(), content_type)], ctx).await;
        Ok(row)
    }

    /// A whole drop at once so identifying twenty jars costs the two requests
    /// one jar would rather than forty
    ///
    /// A file that cannot be stored is reported rather than returned one
    /// unreadable jar must not sink the rest of the selection
    #[tracing::instrument(level = "debug", skip(files, ctx), fields(files = files.len()))]
    pub async fn import_local_files(
        files: &[(PathBuf, ContentType)],
        cluster_id: i64,
        ctx: &ContentCtx,
    ) -> ContentResult<LocalImportReport> {
        let cluster = Self::get_cluster(cluster_id, ctx).await?;

        let mut report = LocalImportReport::default();
        let mut stored = Vec::with_capacity(files.len());

        for (path, content_type) in files {
            match store_local_file(path, *content_type, &cluster, ctx).await {
                Ok(row) => {
                    stored.push((row.clone(), *content_type));
                    report.imported.push(row);
                }
                Err(err) => {
                    tracing::warn!("could not import {}: {err}", path.display());
                    report.failed.push((path.clone(), err));
                }
            }
        }

        describe_imports(&stored, ctx).await;
        Ok(report)
    }

    #[tracing::instrument(level = "debug", skip(ctx))]
    pub async fn resolve_or_download(
        provider_id: ProviderId,
        project_id: &str,
        version_id: &str,
        ctx: &ContentCtx,
    ) -> ContentResult<ArtifactRow> {
        let provider = ctx.providers.get(provider_id)?;

        let version = provider
            .get_version(project_id, version_id, ctx)
            .await?;

        let project = provider.get_project(project_id, ctx).await?;

        Self::download_and_cache(provider_id, &project, &version, false, None, ctx).await
    }
}

/// Copies the file into the cache unless an identical one is already there and
/// links it to the cluster working out what it is comes after
async fn store_local_file(
    path: &Path,
    content_type: ContentType,
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> ContentResult<ArtifactRow> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| PackageError::InvalidLocalFile(path.display().to_string()))?
        .to_string();

    let hash = normalize_hash(&sha1_file(path).await?);

    if let Some(row) = artifact_dao::get_artifact_by_hash(&ctx.db, &hash).await? {
        PackageStore::link_artifact(&row, cluster, None, ctx).await?;
        return Ok(row);
    }

    let dest = cache_file_path(
        content_type,
        ProviderId::Local,
        "imported",
        &hash[..hash.len().min(16)],
        &file_name,
    )?;
    if let Some(parent) = dest.parent() {
        polyio::create_dir_all(parent).await?;
    }
    polyio::copy(path, &dest).await?;

    let size = polyio::stat(&dest).await?.len();
    let stored_path = relative_cache_path(&dest)?;

    let row = artifact_dao::insert_artifact(
        &ctx.db,
        &hash,
        content_type as i64,
        &stored_path,
        &file_name,
        Some(size as i64),
    )
    .await?;

    PackageStore::link_artifact(&row, cluster, None, ctx).await?;
    Ok(row)
}

/// A dropped file arrives as a file name and nothing else so it is worth
/// finding out what it actually is before it lands in the list
///
/// The providers get asked first and for the whole batch at once because a jar
/// downloaded by hand from Modrinth or CurseForge is the same file the browser
/// would have installed and deserves the same card an update check included
/// the jar's own manifest only answers for what neither of them recognises
///
/// Never fails an unidentified mod is still a perfectly good import
#[tracing::instrument(level = "debug", skip(imports, ctx), fields(files = imports.len()))]
async fn describe_imports(imports: &[(ArtifactRow, ContentType)], ctx: &ContentCtx) {
    let mut unknown = Vec::new();
    for (row, content_type) in imports {
        if !already_described(row, ctx).await {
            unknown.push((row, *content_type));
        }
    }

    if unknown.is_empty() {
        return;
    }

    let identities: Vec<FileIdentity> = unknown
        .iter()
        .map(|(row, _)| FileIdentity::from_sha1(&row.hash))
        .collect();

    // Offline or rate limited every jar then falls through to its own manifest
    let found = ctx
        .providers
        .lookup_versions(&identities, ctx)
        .await
        .inspect_err(|err| tracing::debug!("provider lookup failed: {err}"))
        .unwrap_or_default();

    for (row, content_type) in unknown {
        if let Some((provider, version)) = found.get(&row.hash) {
            match record_release(*provider, version, &row.hash, ctx).await {
                Ok(()) => continue,
                Err(err) => {
                    tracing::debug!("could not record a release for {}: {err}", row.file_name);
                }
            }
        }

        if let Err(err) = record_manifest(row, content_type, ctx).await {
            tracing::debug!("could not read {} for metadata: {err}", row.file_name);
        }
    }
}

/// The same file may already have arrived through the browser or an earlier
/// import and describing it again buys nothing
async fn already_described(row: &ArtifactRow, ctx: &ContentCtx) -> bool {
    if artifact_dao::get_release_by_hash(&ctx.db, &row.hash)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        return true;
    }

    !metadata_cache::read_cached_package_meta(
        ctx,
        ProviderId::Local,
        std::slice::from_ref(&row.hash),
    )
    .await
    .is_empty()
}

/// Written exactly as a download would have so the row joins the update flow
/// rather than sitting outside it
async fn record_release(
    provider: ProviderId,
    version: &VersionDetail,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let published_at = version.published.to_rfc3339();

    artifact_dao::upsert_provider_release(
        &ctx.db,
        provider as i64,
        &version.project_id,
        &version.version_id,
        hash,
        &version.name,
        &version.version_number,
        Some(published_at.as_str()),
        &serde_json::to_string(&version.game_versions)?,
        &serde_json::to_string(&version.loaders)?,
    )
    .await?;

    Ok(())
}

/// Cached under the artifact hash because a mod nobody recognises has no
/// project id to be keyed by
async fn record_manifest(
    row: &ArtifactRow,
    content_type: ContentType,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    // Only mods carry a loader manifest resource packs and shaders describe
    // themselves too but in formats that name neither an author nor a mod
    if content_type != ContentType::Mod {
        return Ok(());
    }

    let jar = artifact_absolute_path(&row.path)?;
    let manifest = local_manifest::read_jar_manifest(&jar).await;

    let icon = match &manifest.icon_entry {
        Some(entry) => store_local_icon(&row.hash, &jar, entry).await,
        None => None,
    };

    // A jar that parsed to nothing still gets a row so `already_described` stops
    // both providers being asked about it on every later import
    meta_dao::upsert_package_metadata(
        &ctx.db,
        ProviderId::Local as i64,
        &row.hash,
        manifest.name.as_deref().unwrap_or(&row.file_name),
        manifest.description.as_deref().unwrap_or_default(),
        &manifest.author_line(),
        icon.as_deref(),
    )
    .await?;

    Ok(())
}

/// Kept out of the package cache which the collector sweeps by artifact path
/// and would take an icon sitting next to its jar for an orphan
async fn store_local_icon(hash: &str, jar: &std::path::Path, entry: &str) -> Option<String> {
    let bytes = local_manifest::read_jar_icon(jar, entry).await?;
    if bytes.is_empty() {
        return None;
    }

    let extension = std::path::Path::new(entry)
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| ext.len() <= 4 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("png")
        .to_ascii_lowercase();
    let name = format!("{hash}.{extension}");

    let dir = common_paths::local_icons_dir().ok()?;
    polyio::create_dir_all(&dir).await.ok()?;
    polyio::write(dir.join(&name), &bytes).await.ok()?;

    Some(format!("{}{name}", common_paths::LOCAL_IMAGE_SCHEME))
}

#[tracing::instrument(level = "debug", skip(row, ctx), fields(hash = %row.hash))]
async fn row_to_cached(
    row: ArtifactRow,
    ctx: &ContentCtx,
) -> ContentResult<CachedArtifact> {
    let path = artifact_absolute_path(&row.path)?;
    let release = artifact_dao::get_release_by_hash(&ctx.db, &row.hash)
        .await?
        .map(|r| ProviderReleaseInfo {
            provider: ProviderId::from_repr(r.provider as u8).unwrap_or(ProviderId::Local),
            project_id: r.project_id,
            version_id: r.version_id,
            display_name: r.display_name,
            display_version: r.display_version,
            mc_versions: serde_json::from_str(&r.mc_versions).unwrap_or_default(),
            loaders: serde_json::from_str(&r.mc_loaders).unwrap_or_default(),
        });

    Ok(CachedArtifact {
        hash: row.hash,
        content_type: ContentType::from_repr(row.content_type as u8).unwrap_or(ContentType::Mod),
        path,
        file_name: row.file_name,
        size_bytes: row.size_bytes.map(|s| s as u64),
        release,
    })
}

fn ensure_compatible(
    project: &ProjectDetail,
    version: &VersionDetail,
    cluster: &ClusterRow,
) -> ContentResult<()> {
    if project.provider == ProviderId::Local {
        return Ok(());
    }

    if project.content_type == ContentType::Mod {
        let cluster_loader =
            GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Vanilla);

        if !version.loaders.is_empty()
            && !version
                .loaders
                .iter()
                .any(|l| cluster_loader.compatible_with(*l))
        {
            return Err(PackageError::IncompatibleLoader.into());
        }
    }

    if !version.game_versions.is_empty()
        && !version
            .game_versions
            .iter()
            .any(|v| cluster.mc_version.contains(v))
    {
        return Err(PackageError::IncompatibleMcVersion.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packages::types::PackageBody;
    use chrono::Utc;

    fn project(content_type: ContentType) -> ProjectDetail {
        ProjectDetail {
            id: "p".into(),
            slug: "p".into(),
            provider: ProviderId::Modrinth,
            content_type,
            name: "p".into(),
            summary: String::new(),
            author: String::new(),
            members: Vec::new(),
            gallery: Vec::new(),
            body: PackageBody::Raw(String::new()),
            license: None,
            links: Vec::new(),
            version_ids: Vec::new(),
            game_versions: Vec::new(),
            loaders: Vec::new(),
            icon_url: None,
            created: Utc::now(),
            updated: Utc::now(),
            downloads: 0,
        }
    }

    fn version(loaders: Vec<GameLoader>, game_versions: Vec<&str>) -> VersionDetail {
        VersionDetail {
            version_id: "v".into(),
            project_id: "p".into(),
            name: "v".into(),
            version_number: "1".into(),
            changelog: None,
            game_versions: game_versions.into_iter().map(Into::into).collect(),
            loaders,
            published: Utc::now(),
            downloads: 0,
            files: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    fn cluster(loader: GameLoader, mc_version: &str) -> ClusterRow {
        ClusterRow {
            id: 1,
            name: "c".into(),
            folder_name: "c".into(),
            setting_profile_name: None,
            mc_version: mc_version.into(),
            mc_loader: loader as i64,
            stage: 0,
            mc_loader_version: None,
            created_at: None,
            last_played: None,
            overall_played: None,
            linked_modpack_hash: None,
        }
    }

    #[test]
    fn resource_pack_installs_into_modded_cluster() {
        let result = ensure_compatible(
            &project(ContentType::ResourcePack),
            &version(vec![GameLoader::Vanilla], vec!["1.21.4"]),
            &cluster(GameLoader::Fabric, "1.21.4"),
        );

        assert!(result.is_ok(), "resource packs are not bound to the loader");
    }

    #[test]
    fn mod_still_rejects_incompatible_loader() {
        let result = ensure_compatible(
            &project(ContentType::Mod),
            &version(vec![GameLoader::Forge], vec!["1.21.4"]),
            &cluster(GameLoader::Fabric, "1.21.4"),
        );

        assert!(matches!(
            result,
            Err(ContentError::Package(PackageError::IncompatibleLoader))
        ));
    }

    #[test]
    fn resource_pack_still_rejects_incompatible_mc_version() {
        let result = ensure_compatible(
            &project(ContentType::ResourcePack),
            &version(vec![GameLoader::Vanilla], vec!["1.7.10"]),
            &cluster(GameLoader::Fabric, "1.21.4"),
        );

        assert!(matches!(
            result,
            Err(ContentError::Package(PackageError::IncompatibleMcVersion))
        ));
    }
}
