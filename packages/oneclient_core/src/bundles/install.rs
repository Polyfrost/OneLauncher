use futures_util::StreamExt;
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::ClusterRow;
use oneclient_db::models::OverrideType;

use crate::LauncherError;
use crate::LauncherResult;
use crate::bundles::error::BundleError;
use crate::bundles::manager::BundlesManager;
use crate::bundles::overrides;
use crate::bundles::types::{BundleArchive, BundleFile, BundleFileKind};
use crate::notification::{GroupedProgressChild, GroupedProgressSession, TaskPhase};
use crate::packages::domain::{ContentType, GameLoader};
use crate::packages::error::PackageError;
use crate::packages::store::{PackageStore, unlink_cluster_file};
use crate::packages::types::ExternalFile;
use crate::state::LauncherServices;

fn is_base62(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

pub async fn install_package_from_bundle(
    file: &BundleFile,
    cluster_id: i64,
    bundle_name: &str,
    skip_compatibility: bool,
    child: Option<&GroupedProgressChild>,
    services: &LauncherServices,
) -> LauncherResult<String> {
    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    let hash = match &file.kind {
        BundleFileKind::Managed {
            provider,
            project_id,
            version_id,
            sha1,
        } => {
            let project = crate::packages::cached_project_detail(
                services,
                *provider,
                project_id,
                file.content_type(),
            )
            .await;

            let version = if is_base62(version_id) {
                crate::packages::get_version_cached(services, *provider, project_id, version_id)
                    .await?
            } else if let Ok(Some((_, version))) =
                services.packages.lookup_version(sha1, services).await
            {
                version
            } else {
                crate::packages::get_version_cached(services, *provider, project_id, version_id)
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
                services,
            )
            .await?;
            artifact.hash
        }
        BundleFileKind::External(ext) => {
            install_external(ext, &cluster, skip_compatibility, child, services).await?
        }
    };

    bundle_dao::track_bundle_artifact(
        &services.db,
        cluster_id,
        &hash,
        bundle_name,
        &file.kind.bundle_version_id(),
        &file.kind.package_id(),
    )
    .await?;

    Ok(hash)
}

async fn install_external(
    ext: &ExternalFile,
    cluster: &ClusterRow,
    skip_compatibility: bool,
    child: Option<&GroupedProgressChild>,
    services: &LauncherServices,
) -> LauncherResult<String> {
    let artifact = crate::packages::store::download_external(ext, false, child, services).await?;
    PackageStore::link_artifact(&artifact, cluster, Some(&ext.name), services).await?;

    let _ = skip_compatibility;
    Ok(artifact.hash)
}

pub async fn extract_bundle_overrides_for_cluster(
    archive: &BundleArchive,
    cluster_id: i64,
    services: &LauncherServices,
) -> LauncherResult<()> {
    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    overrides::extract_bundle_overrides(&archive.bundle.path, &cluster).await
}

pub async fn install_bundle(
    cluster_id: i64,
    bundle_name: &str,
    skip_compatibility: bool,
    bundles: &BundlesManager,
    services: &LauncherServices,
) -> LauncherResult<Vec<String>> {
    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).ok_or_else(|| {
        LauncherError::InvalidSettingsProfile {
            reason: format!("unknown loader {}", cluster.mc_loader),
        }
    })?;

    let archive = bundles
        .archives_for(services, &cluster.mc_version, loader)
        .await?
        .into_iter()
        .find(|a| a.manifest.name == bundle_name)
        .ok_or(BundleError::NotFound(bundle_name.to_string()))?;

    install_enabled_bundle_files(&archive, cluster_id, skip_compatibility, None, services).await
}

pub async fn install_enabled_bundle_files(
    archive: &BundleArchive,
    cluster_id: i64,
    skip_compatibility: bool,
    progress: Option<&GroupedProgressSession>,
    services: &LauncherServices,
) -> LauncherResult<Vec<String>> {
    extract_bundle_overrides_for_cluster(archive, cluster_id, services).await?;

    let overrides = bundle_dao::list_overrides(&services.db, cluster_id).await?;
    let bundle_name = archive.manifest.name.clone();
    let mut installed = Vec::new();

    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    let linked = PackageStore::list_linked_artifacts(cluster_id, services).await?;
    let mut linked_projects: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut linked_hashes: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let clusters_dir = crate::paths::clusters_dir()?;
    for info in &linked {
        let path = clusters_dir
            .join(&cluster.folder_name)
            .join(info.content_type.folder_name())
            .join(&info.cluster_file_name);
        if path.exists() {
            if let Some(pid) = &info.project_id {
                linked_projects.insert(pid.as_str());
            }
            linked_hashes.insert(info.hash.as_str());
        }
    }

    let to_install: Vec<BundleFile> = archive
        .manifest
        .files
        .iter()
        .filter(|file| {
            if !file.enabled {
                return false;
            }
            let package_id = file.kind.package_id();
            let overridden = overrides.iter().any(|o| {
                o.bundle_name == bundle_name
                    && o.package_id == package_id
                    && matches!(
                        OverrideType::parse(&o.override_type),
                        Some(OverrideType::Removed | OverrideType::Disabled)
                    )
            });
            if overridden {
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

    let bundle_name = &bundle_name;
    let results = futures_util::stream::iter(to_install.into_iter().map(|file| async move {
        let child = progress.map(|p| {
            let c = p.child(format!("Mod {}", file.display_name()), file.size.max(1));
            c.set_phase(TaskPhase::Downloading);
            c
        });

        let result = install_package_from_bundle(
            &file,
            cluster_id,
            bundle_name,
            skip_compatibility,
            child.as_ref(),
            services,
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

const BUNDLE_INSTALL_CONCURRENCY: usize = 6;

pub async fn enabled_bundle_bytes(
    cluster_id: i64,
    bundles: &BundlesManager,
    services: &LauncherServices,
) -> LauncherResult<u64> {
    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).ok_or_else(|| {
        LauncherError::InvalidSettingsProfile {
            reason: format!("unknown loader {}", cluster.mc_loader),
        }
    })?;

    let archives = bundles
        .archives_for(services, &cluster.mc_version, loader)
        .await?;
    let overrides = bundle_dao::list_overrides(&services.db, cluster_id).await?;
    let linked = PackageStore::list_linked_artifacts(cluster_id, services).await?;

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
            if !file.enabled {
                continue;
            }
            let package_id = file.kind.package_id();
            let overridden = overrides.iter().any(|o| {
                &o.bundle_name == bundle_name
                    && o.package_id == package_id
                    && matches!(
                        OverrideType::parse(&o.override_type),
                        Some(OverrideType::Removed | OverrideType::Disabled)
                    )
            });
            if overridden {
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

pub async fn set_bundle_package_enabled(
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
    enabled: bool,
    services: &LauncherServices,
) -> LauncherResult<()> {
    if enabled {
        bundle_dao::remove_override(&services.db, cluster_id, bundle_name, package_id).await?;
    } else {
        bundle_dao::save_override(
            &services.db,
            cluster_id,
            bundle_name,
            package_id,
            OverrideType::Disabled,
        )
        .await?;
    }

    Ok(())
}

pub async fn list_cluster_bundle_overrides(
    cluster_id: i64,
    services: &LauncherServices,
) -> LauncherResult<Vec<(String, String, String)>> {
    let rows = bundle_dao::list_overrides(&services.db, cluster_id).await?;
    Ok(rows
        .into_iter()
        .map(|o| (o.bundle_name, o.package_id, o.override_type))
        .collect())
}

pub async fn install_cluster_bundles(
    cluster_id: i64,
    bundles: &BundlesManager,
    progress: Option<&GroupedProgressSession>,
    services: &LauncherServices,
) -> LauncherResult<()> {
    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).ok_or_else(|| {
        LauncherError::InvalidSettingsProfile {
            reason: format!("unknown loader {}", cluster.mc_loader),
        }
    })?;

    let archives = bundles
        .archives_for(services, &cluster.mc_version, loader)
        .await?;
    tracing::info!(
        cluster_id,
        mc_version = %cluster.mc_version,
        bundles = archives.len(),
        "installing enabled bundle content"
    );
    for archive in &archives {
        let installed =
            install_enabled_bundle_files(archive, cluster_id, true, progress, services).await?;
        tracing::info!(
            cluster_id,
            bundle = %archive.manifest.name,
            installed = installed.len(),
            "installed bundle files"
        );
    }

    Ok(())
}

pub async fn on_user_remove_artifact(
    cluster_id: i64,
    hash: &str,
    services: &LauncherServices,
) -> LauncherResult<()> {
    handle_user_artifact_action(cluster_id, hash, services, OverrideType::Removed).await
}

pub async fn on_user_disable_artifact(
    cluster_id: i64,
    hash: &str,
    services: &LauncherServices,
) -> LauncherResult<()> {
    handle_user_artifact_action(cluster_id, hash, services, OverrideType::Disabled).await
}

pub async fn on_user_enable_artifact(
    cluster_id: i64,
    hash: &str,
    services: &LauncherServices,
) -> LauncherResult<()> {
    if let Some(tracked) = bundle_dao::get_bundle_tracked(&services.db, cluster_id, hash).await?
        && let (Some(bundle_name), Some(package_id)) = (tracked.bundle_name, tracked.package_id) {
            bundle_dao::remove_override(&services.db, cluster_id, &bundle_name, &package_id)
                .await?;
        }
    Ok(())
}

async fn handle_user_artifact_action(
    cluster_id: i64,
    hash: &str,
    services: &LauncherServices,
    override_type: OverrideType,
) -> LauncherResult<()> {
    let Some(tracked) = bundle_dao::get_bundle_tracked(&services.db, cluster_id, hash).await?
    else {
        return Ok(());
    };

    let (Some(bundle_name), Some(package_id)) = (tracked.bundle_name, tracked.package_id) else {
        return Ok(());
    };

    bundle_dao::save_override(
        &services.db,
        cluster_id,
        &bundle_name,
        &package_id,
        override_type,
    )
    .await?;

    Ok(())
}

pub async fn remove_artifact_from_cluster(
    cluster_id: i64,
    hash: &str,
    record_override: bool,
    services: &LauncherServices,
) -> LauncherResult<()> {
    let bundle_data = bundle_dao::get_bundle_tracked(&services.db, cluster_id, hash).await?;
    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    let artifact = artifact_dao::get_artifact_by_hash(&services.db, hash)
        .await?
        .ok_or(PackageError::ArtifactMissing(hash.to_string()))?;

    let content_type = ContentType::from_repr(artifact.content_type as u8).ok_or_else(|| {
        LauncherError::InvalidSettingsProfile {
            reason: format!("unknown content type {}", artifact.content_type),
        }
    })?;

    if let Some(link) = artifact_dao::list_cluster_artifacts(&services.db, cluster_id)
        .await?
        .into_iter()
        .find(|l| l.hash == hash)
    {
        unlink_cluster_file(&cluster, content_type, &link.cluster_file_name).await?;
    }

    artifact_dao::unlink_cluster_artifact(&services.db, cluster_id, hash).await?;

    if let Some(tracked) = bundle_data
        && let (Some(bundle_name), Some(package_id)) =
            (tracked.bundle_name.clone(), tracked.package_id.clone())
        {
            if record_override {
                bundle_dao::save_override(
                    &services.db,
                    cluster_id,
                    &bundle_name,
                    &package_id,
                    OverrideType::Removed,
                )
                .await?;
            } else {
                let replacement_exists = bundle_dao::has_bundle_mapping(
                    &services.db,
                    cluster_id,
                    &bundle_name,
                    &package_id,
                )
                .await?;
                if !replacement_exists {
                    bundle_dao::remove_override(
                        &services.db,
                        cluster_id,
                        &bundle_name,
                        &package_id,
                    )
                    .await?;
                }
            }
        }

    Ok(())
}
