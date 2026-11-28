use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::models::ArtifactRow;

use super::paths::{artifact_absolute_path, cache_file_path, relative_cache_path};
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::types::{ExternalFile, VersionDetail, VersionFile};
use oneclient_common::constants::MODRINTH_CDN_PREFIX;
use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_events::GroupedProgressChild;
use polyio::{normalize_hash, sha1_file};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadReason {
    Standalone,
    Dependency,
    Modpack,
    Update,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadMeta {
    pub reason: DownloadReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependent_on: Option<String>,
}

fn download_headers(url: &str, meta: Option<&DownloadMeta>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(meta) = meta
        && url.starts_with(MODRINTH_CDN_PREFIX)
        && let Ok(json) = serde_json::to_string(meta)
        && let Ok(value) = HeaderValue::from_str(&json)
    {
        headers.insert("modrinth-download-meta", value);
    }
    headers
}

#[tracing::instrument(level = "debug", skip(child, ctx))]
pub async fn ensure_artifact_file(
    hash: &str,
    url: &str,
    dest: &std::path::Path,
    child: Option<&GroupedProgressChild>,
    meta: Option<&DownloadMeta>,
    ctx: &ContentCtx,
) -> ContentResult<u64> {
    if oneclient_net::matches_on_disk(dest, hash).await {
        return Ok(polyio::stat(dest).await?.len());
    }

    tracing::debug!("downloading artifact file");
    // Modrinth and CurseForge both index by SHA-1
    let expected = polyio::Checksum::sha1(hash);
    oneclient_net::download_verified_with_headers(
        &ctx.net,
        &ctx.events,
        url,
        dest,
        Some(&expected),
        0,
        child
            .cloned()
            .map(oneclient_net::ResponseNotifyOptions::grouped),
        &download_headers(url, meta),
    )
    .await?;

    Ok(polyio::stat(dest).await?.len())
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "debug", skip(version, file, child, meta, ctx))]
pub async fn download_version_file(
    provider: ProviderId,
    project_id: &str,
    version: &VersionDetail,
    content_type: ContentType,
    file: &VersionFile,
    force: bool,
    child: Option<&GroupedProgressChild>,
    meta: Option<&DownloadMeta>,
    ctx: &ContentCtx,
) -> ContentResult<ArtifactRow> {
    let hash = normalize_hash(&file.sha1);
    if !force && let Some(row) = artifact_dao::get_artifact_by_hash(&ctx.db, &hash).await? {
        let path = artifact_absolute_path(&row.path)?;
        if path.exists() {
            let disk = sha1_file(&path).await?;
            if disk == hash {
                // The cached copy may have arrived as a local import, which
                // knows nothing of its provider; this download does
                record_release(provider, project_id, version, &hash, ctx).await?;
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

    let size = ensure_artifact_file(&hash, &file.url, &dest, child, meta, ctx).await?;
    let stored_path = relative_cache_path(&dest)?;

    let row = artifact_dao::insert_artifact(
        &ctx.db,
        &hash,
        content_type as i64,
        &stored_path,
        &file.file_name,
        Some(size as i64),
    )
    .await?;

    record_release(provider, project_id, version, &hash, ctx).await?;

    Ok(row)
}

async fn record_release(
    provider: ProviderId,
    project_id: &str,
    version: &VersionDetail,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let published_at = version.published.to_rfc3339();

    artifact_dao::upsert_provider_release(
        &ctx.db,
        provider as i64,
        project_id,
        &version.version_id,
        hash,
        &version.name,
        &version.version_number,
        Some(published_at.as_str()),
        &serde_json::to_string(&version.game_versions)?,
        &serde_json::to_string(&version.loaders)?,
    )
    .await?;

    // The provider handed us the dependency list along with the version, and
    // this is the only moment it is guaranteed to be in hand. Keeping it is what
    // lets the launcher answer "what needs this?" later without a round trip
    // per installed package — see `crate::packages::dependents`.
    store_release_dependencies(provider, project_id, version, ctx).await?;

    Ok(())
}

pub(crate) async fn store_release_dependencies(
    provider: ProviderId,
    project_id: &str,
    version: &VersionDetail,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let dependencies: Vec<(String, String, String)> = version
        .dependencies
        .iter()
        .map(|dep| {
            (
                dep.project_id.clone().unwrap_or_default(),
                dep.version_id.clone().unwrap_or_default(),
                dep.kind.as_str().to_string(),
            )
        })
        .collect();

    artifact_dao::replace_release_dependencies(
        &ctx.db,
        provider as i64,
        project_id,
        &version.version_id,
        &dependencies,
    )
    .await?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip(file, child, ctx), fields(name = %file.name))]
pub async fn download_external(
    file: &ExternalFile,
    force: bool,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<ArtifactRow> {
    if !polyio::Checksum::sha1(&file.sha1).is_well_formed() {
        return Err(crate::error::ContentError::InvalidData {
            reason: format!("'{}' has an invalid SHA-1 hash", file.name),
        });
    }
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
    let size = ensure_artifact_file(&hash, &file.url, &dest, child, None, ctx).await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_header_only_goes_to_modrinth() {
        let modrinth = "https://cdn.modrinth.com/data/a/versions/b/c.jar";
        let meta = DownloadMeta {
            reason: DownloadReason::Modpack,
            game_version: None,
            loader: None,
            dependent_on: Some("zSLSxmnP".to_string()),
        };
        assert_eq!(
            download_headers(modrinth, Some(&meta))["modrinth-download-meta"],
            r#"{"reason":"modpack","dependent_on":"zSLSxmnP"}"#
        );
        assert!(download_headers("https://github.com/a/b.jar", Some(&meta)).is_empty());
        assert!(download_headers(modrinth, None).is_empty());
    }
}
