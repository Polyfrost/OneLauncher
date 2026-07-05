mod download;
mod link;
mod paths;

pub use download::{download_external, download_version_file, ensure_artifact_file};
pub use link::{link_artifact_to_cluster, unlink_cluster_file};
pub use paths::{artifact_absolute_path, cache_file_path, relative_cache_path};

use oneclient_db::dao::{artifact as artifact_dao, cluster as cluster_dao};
use oneclient_db::models::{ArtifactRow, ClusterRow};

use super::domain::{ContentType, GameLoader, ProviderId};
use super::error::PackageError;
use super::types::{CachedArtifact, ProjectDetail, ProviderReleaseInfo, VersionDetail, LinkedArtifactInfo};
use crate::crypto::{normalize_hash, sha1_file};
use crate::state::LauncherServices;
use crate::{LauncherError, LauncherResult};
use std::path::Path;

pub struct PackageStore;

impl PackageStore {
    pub async fn get_cluster(
        cluster_id: i64,
        services: &LauncherServices,
    ) -> LauncherResult<ClusterRow> {
        cluster_dao::get_by_id(&services.db, cluster_id)
            .await?
            .ok_or(PackageError::ClusterNotFound(cluster_id).into())
    }

    pub async fn cached_artifact(
        hash: &str,
        services: &LauncherServices,
    ) -> LauncherResult<Option<CachedArtifact>> {
        let Some(row) = artifact_dao::get_artifact_by_hash(&services.db, hash).await? else {
            return Ok(None);
        };

        Ok(Some(row_to_cached(row, services).await?))
    }

    pub async fn download_and_cache(
        provider_id: ProviderId,
        project: &ProjectDetail,
        version: &VersionDetail,
        force: bool,
        services: &LauncherServices,
    ) -> LauncherResult<ArtifactRow> {
        let file = version.primary_file().ok_or(PackageError::NoPrimaryFile)?;

        download::download_version_file(
            provider_id,
            &project.id,
            version,
            project.content_type,
            file,
            force,
            services,
        )
        .await
    }

    pub async fn install_to_cluster(
        provider_id: ProviderId,
        project: &ProjectDetail,
        version: &VersionDetail,
        cluster_id: i64,
        skip_compatibility: bool,
        force_download: bool,
        services: &LauncherServices,
    ) -> LauncherResult<ArtifactRow> {
        let cluster = Self::get_cluster(cluster_id, services).await?;

        if !skip_compatibility {
            ensure_compatible(project, version, &cluster)?;
        }

        let artifact = Self::download_and_cache(
            provider_id,
            project,
            version,
            force_download,
            services,
        )
        .await?;

        Self::link_artifact(&artifact, &cluster, None, services).await?;
        Ok(artifact)
    }

    pub async fn link_artifact(
        artifact: &ArtifactRow,
        cluster: &ClusterRow,
        cluster_file_name: Option<&str>,
        services: &LauncherServices,
    ) -> LauncherResult<()> {
        let name = cluster_file_name.unwrap_or(&artifact.file_name);

        link::link_artifact_to_cluster(artifact, cluster, Some(name)).await?;
        artifact_dao::link_cluster_artifact(&services.db, cluster.id, &artifact.hash, name).await?;

        Ok(())
    }

    pub async fn link_by_hash(
        hash: &str,
        cluster_id: i64,
        cluster_file_name: Option<&str>,
        services: &LauncherServices,
    ) -> LauncherResult<()> {
        let artifact = artifact_dao::get_artifact_by_hash(&services.db, hash)
            .await?
            .ok_or(PackageError::ArtifactMissing(hash.to_string()))?;

        let cluster = Self::get_cluster(cluster_id, services).await?;

        Self::link_artifact(&artifact, &cluster, cluster_file_name, services).await
    }

    pub async fn unlink_from_cluster(
        hash: &str,
        cluster_id: i64,
        services: &LauncherServices,
    ) -> LauncherResult<()> {
        crate::bundles::remove_artifact_from_cluster(cluster_id, hash, true, services).await
    }

    pub async fn unlink_from_cluster_system(
        hash: &str,
        cluster_id: i64,
        services: &LauncherServices,
    ) -> LauncherResult<()> {
        crate::bundles::remove_artifact_from_cluster(cluster_id, hash, false, services).await
    }

    pub async fn list_linked_artifacts(
        cluster_id: i64,
        services: &LauncherServices,
    ) -> LauncherResult<Vec<LinkedArtifactInfo>> {
        let links = artifact_dao::list_cluster_artifacts(&services.db, cluster_id).await?;
        let mut items = Vec::with_capacity(links.len());

        for link in links {
            let Some(artifact) =
                artifact_dao::get_artifact_by_hash(&services.db, &link.hash).await?
            else {
                continue;
            };

            let content_type = ContentType::from_repr(artifact.content_type as u8)
                .unwrap_or(ContentType::Mod);
            let release = artifact_dao::get_release_by_hash(&services.db, &link.hash).await?;

            items.push(LinkedArtifactInfo {
                hash: link.hash,
                cluster_file_name: link.cluster_file_name,
                enabled: link.enabled != 0,
                content_type,
                file_name: artifact.file_name,
                project_id: release.as_ref().map(|r| r.project_id.clone()),
                display_name: release.as_ref().map(|r| r.display_name.clone()),
                display_version: release.as_ref().map(|r| r.display_version.clone()),
                provider: release
                    .as_ref()
                    .and_then(|r| ProviderId::from_repr(r.provider as u8)),
            });
        }

        Ok(items)
    }

    pub async fn toggle_artifact_enabled(
        cluster_id: i64,
        hash: &str,
        services: &LauncherServices,
    ) -> LauncherResult<bool> {
        let cluster = Self::get_cluster(cluster_id, services).await?;
        let artifact = artifact_dao::get_artifact_by_hash(&services.db, hash)
            .await?
            .ok_or(PackageError::ArtifactMissing(hash.to_string()))?;

        let link = artifact_dao::get_cluster_artifact(&services.db, cluster_id, hash)
            .await?
            .ok_or(PackageError::ArtifactMissing(hash.to_string()))?;

        let content_type = ContentType::from_repr(artifact.content_type as u8)
            .ok_or_else(|| LauncherError::InvalidSettingsProfile {
                reason: format!("unknown content type {}", artifact.content_type),
            })?;

        let enabled = link.enabled == 0;
        let file_name = link
            .cluster_file_name
            .trim_end_matches(".disabled")
            .to_string();

        if enabled {
            link::link_artifact_to_cluster(&artifact, &cluster, Some(&file_name)).await?;
        } else {
            link::unlink_cluster_file(&cluster, content_type, &link.cluster_file_name).await?;
            if link.cluster_file_name != file_name {
                link::unlink_cluster_file(&cluster, content_type, &file_name).await?;
            }
        }

        artifact_dao::update_cluster_artifact(
            &services.db,
            cluster_id,
            hash,
            &file_name,
            i64::from(enabled),
        )
        .await?;

        if enabled {
            crate::bundles::on_user_enable_artifact(cluster_id, hash, services).await?;
        } else {
            crate::bundles::on_user_disable_artifact(cluster_id, hash, services).await?;
        }

        Ok(enabled)
    }

    pub async fn import_local_file(
        path: &Path,
        content_type: ContentType,
        cluster_id: i64,
        services: &LauncherServices,
    ) -> LauncherResult<ArtifactRow> {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| PackageError::InvalidLocalFile(path.display().to_string()))?
            .to_string();

        let hash = normalize_hash(&sha1_file(path).await?);

        if let Some(row) = artifact_dao::get_artifact_by_hash(&services.db, &hash).await? {
            let cluster = Self::get_cluster(cluster_id, services).await?;
            Self::link_artifact(&row, &cluster, None, services).await?;
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
        tokio::fs::copy(path, &dest).await?;

        let size = tokio::fs::metadata(&dest).await?.len();
        let stored_path = relative_cache_path(&dest)?;

        let row = artifact_dao::insert_artifact(
            &services.db,
            &hash,
            content_type as i64,
            &stored_path,
            &file_name,
            Some(size as i64),
        )
        .await?;

        let cluster = Self::get_cluster(cluster_id, services).await?;
        Self::link_artifact(&row, &cluster, None, services).await?;
        Ok(row)
    }

    pub async fn resolve_or_download(
        provider_id: ProviderId,
        project_id: &str,
        version_id: &str,
        services: &LauncherServices,
    ) -> LauncherResult<ArtifactRow> {
        let provider = services.packages.get(provider_id)?;

        let version = provider
            .get_version(project_id, version_id, services)
            .await?;

        let project = provider.get_project(project_id, services).await?;

        Self::download_and_cache(provider_id, &project, &version, false, services).await
    }
}

async fn row_to_cached(
    row: ArtifactRow,
    services: &LauncherServices,
) -> LauncherResult<CachedArtifact> {
    let path = artifact_absolute_path(&row.path)?;
    let release = artifact_dao::get_release_by_hash(&services.db, &row.hash)
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
) -> LauncherResult<()> {
    if project.provider == ProviderId::Local {
        return Ok(());
    }

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
