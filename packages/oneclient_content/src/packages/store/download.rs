use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::models::ArtifactRow;

use super::paths::{artifact_absolute_path, cache_file_path, relative_cache_path};
use crate::error::ContentResult;
use polyio::{normalize_hash, sha1_file};
use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_events::GroupedProgressChild;
use crate::packages::types::{ExternalFile, VersionDetail, VersionFile};
use crate::ctx::ContentCtx;

#[tracing::instrument(level = "debug", skip(child, ctx))]
pub async fn ensure_artifact_file(
    hash: &str,
    url: &str,
    dest: &std::path::Path,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<u64> {
    if oneclient_net::matches_on_disk(dest, hash).await {
        return Ok(polyio::stat(dest).await?.len());
    }

    tracing::debug!("downloading artifact file");
    // Modrinth and CurseForge both index by SHA-1.
    let expected = polyio::Checksum::sha1(hash);
    oneclient_net::download_verified(
        &ctx.net,
        &ctx.events,
        url,
        dest,
        Some(&expected),
        0,
        child.cloned().map(oneclient_net::ResponseNotifyOptions::grouped),
    )
    .await?;

    Ok(polyio::stat(dest).await?.len())
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "debug", skip(version, file, child, ctx))]
pub async fn download_version_file(
    provider: ProviderId,
    project_id: &str,
    version: &VersionDetail,
    content_type: ContentType,
    file: &VersionFile,
    force: bool,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<ArtifactRow> {
    let hash = normalize_hash(&file.sha1);
    if !force && let Some(row) = artifact_dao::get_artifact_by_hash(&ctx.db, &hash).await? {
        let path = artifact_absolute_path(&row.path)?;
        if path.exists() {
            let disk = sha1_file(&path).await?;
            if disk == hash {
                return Ok(row);
            }
        }
    }

    let dest = cache_file_path(
        content_type,
        provider,
        project_id,
        &version.version_id,
        &file.file_name,
    )?;

    let size = ensure_artifact_file(&hash, &file.url, &dest, child, ctx).await?;
    let stored_path = relative_cache_path(&dest)?;

    let published_at = version.published.to_rfc3339();

    let row = artifact_dao::insert_artifact(
        &ctx.db,
        &hash,
        content_type as i64,
        &stored_path,
        &file.file_name,
        Some(size as i64),
    )
    .await?;

    artifact_dao::upsert_provider_release(
        &ctx.db,
        provider as i64,
        project_id,
        &version.version_id,
        &hash,
        &version.name,
        &version.version_number,
        Some(published_at.as_str()),
        &serde_json::to_string(&version.game_versions)?,
        &serde_json::to_string(&version.loaders)?,
    )
    .await?;

    Ok(row)
}

#[tracing::instrument(level = "debug", skip(file, child, ctx), fields(name = %file.name))]
pub async fn download_external(
    file: &ExternalFile,
    force: bool,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<ArtifactRow> {
    let hash = normalize_hash(&file.sha1);
    if !force && let Some(row) = artifact_dao::get_artifact_by_hash(&ctx.db, &hash).await? {
        let path = artifact_absolute_path(&row.path)?;
        if path.exists() {
            return Ok(row);
        }
    }

    let dest = cache_file_path(
        file.content_type,
        ProviderId::Local,
        "external",
        &hash[..hash.len().min(16)],
        &file.name,
    )?;
    let size = ensure_artifact_file(&hash, &file.url, &dest, child, ctx).await?;
    let stored_path = relative_cache_path(&dest)?;

    artifact_dao::insert_artifact(
        &ctx.db,
        &hash,
        file.content_type as i64,
        &stored_path,
        &file.name,
        Some(size as i64),
    )
    .await
    .map_err(Into::into)
}
