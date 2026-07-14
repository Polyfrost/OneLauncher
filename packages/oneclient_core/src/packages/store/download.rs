use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::models::ArtifactRow;

use super::paths::{artifact_absolute_path, cache_file_path, relative_cache_path};
use crate::LauncherResult;
use crate::crypto::{normalize_hash, sha1_file};
use crate::packages::domain::{ContentType, ProviderId};
use crate::packages::error::PackageError;
use crate::notification::{GroupedProgressChild, TaskPhase};
use crate::packages::provider::http::download_url;
use crate::packages::types::{ExternalFile, VersionDetail, VersionFile};
use crate::state::LauncherServices;

#[tracing::instrument(level = "debug", skip(child, services))]
pub async fn ensure_artifact_file(
    hash: &str,
    url: &str,
    dest: &std::path::Path,
    child: Option<&GroupedProgressChild>,
    services: &LauncherServices,
) -> LauncherResult<u64> {
    polyio::create_dir_all(dest.parent().unwrap_or(dest)).await?;

    if dest.exists() {
        let existing = sha1_file(dest).await?;
        if normalize_hash(&existing) == normalize_hash(hash) {
            return Ok(tokio::fs::metadata(dest).await?.len());
        }
        tokio::fs::remove_file(dest).await?;
    }

    tracing::debug!("downloading artifact file");
    download_url(&services.requester, url, dest, child, services).await?;

    if let Some(child) = child {
        child.set_phase(TaskPhase::Verifying);
    }
    let actual = sha1_file(dest).await?;
    if normalize_hash(&actual) != normalize_hash(hash) {
        tracing::warn!(expected = %hash, %actual, "downloaded artifact hash mismatch");
        return Err(PackageError::HashMismatch {
            expected: hash.to_string(),
            actual,
        }
        .into());
    }

    Ok(tokio::fs::metadata(dest).await?.len())
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "debug", skip(version, file, child, services))]
pub async fn download_version_file(
    provider: ProviderId,
    project_id: &str,
    version: &VersionDetail,
    content_type: ContentType,
    file: &VersionFile,
    force: bool,
    child: Option<&GroupedProgressChild>,
    services: &LauncherServices,
) -> LauncherResult<ArtifactRow> {
    let hash = normalize_hash(&file.sha1);
    if !force && let Some(row) = artifact_dao::get_artifact_by_hash(&services.db, &hash).await? {
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

    let size = ensure_artifact_file(&hash, &file.url, &dest, child, services).await?;
    let stored_path = relative_cache_path(&dest)?;

    let published_at = version.published.to_rfc3339();

    let row = artifact_dao::insert_artifact(
        &services.db,
        &hash,
        content_type as i64,
        &stored_path,
        &file.file_name,
        Some(size as i64),
    )
    .await?;

    artifact_dao::upsert_provider_release(
        &services.db,
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

#[tracing::instrument(level = "debug", skip(file, child, services), fields(name = %file.name))]
pub async fn download_external(
    file: &ExternalFile,
    force: bool,
    child: Option<&GroupedProgressChild>,
    services: &LauncherServices,
) -> LauncherResult<ArtifactRow> {
    let hash = normalize_hash(&file.sha1);
    if !force && let Some(row) = artifact_dao::get_artifact_by_hash(&services.db, &hash).await? {
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
    let size = ensure_artifact_file(&hash, &file.url, &dest, child, services).await?;
    let stored_path = relative_cache_path(&dest)?;

    artifact_dao::insert_artifact(
        &services.db,
        &hash,
        file.content_type as i64,
        &stored_path,
        &file.name,
        Some(size as i64),
    )
    .await
    .map_err(Into::into)
}
