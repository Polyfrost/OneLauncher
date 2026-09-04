use std::sync::Arc;

use oneclient_core::LauncherState;

use oneclient_content::packages::PackageStore;
use oneclient_events::Level;

use crate::components::IconType;
use crate::notifications::{
    ClusterUpdateItem, ClusterUpdateSummary, NotificationAction, NotificationActionKind,
    NotificationSpec, PackageUpdateGroup,
};

/// Falls back to a placeholder a missing row must not lose the whole message
pub async fn cluster_display_name(
    cluster_id: i64,
    services: &oneclient_core::LauncherServices,
) -> String {
    PackageStore::get_cluster(cluster_id, &services.content())
        .await
        .map(|cluster| cluster.name)
        .unwrap_or_else(|_| "Cluster".to_string())
}

pub async fn package_update_group(
    cluster_id: i64,
    updates: &[oneclient_core::BrowserPackageUpdate],
    services: &oneclient_core::LauncherServices,
) -> Option<PackageUpdateGroup> {
    if updates.is_empty() {
        return None;
    }

    Some(PackageUpdateGroup {
        cluster_id,
        cluster_name: cluster_display_name(cluster_id, services).await,
        packages: updates.to_vec(),
    })
}

pub fn item_from_bundle_file(file: &oneclient_core::BundleFile) -> ClusterUpdateItem {
    match &file.kind {
        oneclient_core::BundleFileKind::Managed {
            provider,
            project_id,
            ..
        } => ClusterUpdateItem {
            provider: *provider,
            project_id: Some(project_id.clone()),
            fallback: file.display_name(),
        },
        oneclient_core::BundleFileKind::External(_) => ClusterUpdateItem {
            provider: oneclient_content::packages::ProviderId::Local,
            project_id: None,
            fallback: file.display_name(),
        },
    }
}

async fn cluster_update_summary(
    cluster_id: i64,
    result: &oneclient_core::ApplyBundleUpdatesResult,
    services: &oneclient_core::LauncherServices,
) -> Option<ClusterUpdateSummary> {
    let updated: Vec<ClusterUpdateItem> = result
        .updates_applied
        .iter()
        .map(|u| item_from_bundle_file(&u.new_file))
        .collect();
    let added: Vec<ClusterUpdateItem> = result
        .additions_applied
        .iter()
        .map(|a| item_from_bundle_file(&a.new_file))
        .collect();
    let removed: Vec<ClusterUpdateItem> = result
        .removals_applied
        .iter()
        .map(|r| ClusterUpdateItem {
            provider: r
                .provider
                .unwrap_or(oneclient_content::packages::ProviderId::Local),
            project_id: r.project_id.clone(),
            fallback: r
                .display_name
                .clone()
                .unwrap_or_else(|| r.package_id.clone()),
        })
        .collect();

    if updated.is_empty() && added.is_empty() && removed.is_empty() {
        return None;
    }

    let cluster_name =
        oneclient_content::packages::PackageStore::get_cluster(cluster_id, &services.content())
            .await
            .map(|c| c.name)
            .unwrap_or_else(|_| "Cluster".to_string());

    Some(ClusterUpdateSummary {
        cluster_id,
        cluster_name,
        updated,
        added,
        removed,
    })
}

pub async fn cluster_update_notification(
    cluster_id: i64,
    result: &oneclient_core::ApplyBundleUpdatesResult,
    services: &oneclient_core::LauncherServices,
) -> Option<NotificationSpec> {
    let summary = cluster_update_summary(cluster_id, result, services).await?;
    let total = summary.total();
    let body = format!(
        "{total} package{} changed in {}",
        if total == 1 { "" } else { "s" },
        summary.cluster_name
    );

    Some(NotificationSpec {
        title: "Cluster updated".to_string(),
        body,
        level: Level::Info,
        icon: Some(IconType::DownloadCloud02),
        progress: None,
        actions: vec![NotificationAction {
            label: "View changes".to_string(),
            kind: NotificationActionKind::OpenClusterUpdate(vec![summary]),
        }],
    })
}

/// All clusters share one "View changes" action so the notification keeps exactly two buttons
pub async fn combined_cluster_update_spec(
    changed: &[(i64, oneclient_core::ApplyBundleUpdatesResult)],
    services: &oneclient_core::LauncherServices,
) -> Option<NotificationSpec> {
    let mut summaries = Vec::new();
    let mut total_changes = 0usize;

    for (cluster_id, result) in changed {
        if let Some(summary) = cluster_update_summary(*cluster_id, result, services).await {
            total_changes += summary.total();
            summaries.push(summary);
        }
    }

    if summaries.is_empty() {
        return None;
    }

    let cluster_count = summaries.len();
    let body = format!(
        "{total_changes} package{} updated across {cluster_count} cluster{}",
        if total_changes == 1 { "" } else { "s" },
        if cluster_count == 1 { "" } else { "s" }
    );

    Some(NotificationSpec {
        title: "Mods updated".to_string(),
        body,
        level: Level::Info,
        icon: Some(IconType::DownloadCloud02),
        progress: None,
        actions: vec![NotificationAction {
            label: "View changes".to_string(),
            kind: NotificationActionKind::OpenClusterUpdate(summaries),
        }],
    })
}

/// The session is detached not finished so the caller can reuse its notification
/// as the "Installed" / "Install failed" result
/// `session_id` is `None` if no download ran
pub struct PackageInstall {
    pub session_id: Option<uuid::Uuid>,
    pub result: anyhow::Result<String>,
    pub dependencies: Vec<String>,
    /// Unresolved or failed dependencies the package still installs
    pub missing_dependencies: Vec<String>,
}

pub fn install_body(name: &str, dependencies: &[String], missing: &[String]) -> String {
    let mut body = format!("Added {name}");

    if !dependencies.is_empty() {
        body.push_str(&format!(
            " with {} dependenc{}",
            dependencies.len(),
            if dependencies.len() == 1 { "y" } else { "ies" }
        ));
    }
    body.push('.');

    if !missing.is_empty() {
        body.push_str(&format!(" Could not add: {}.", missing.join(", ")));
    }

    body
}

impl PackageInstall {
    fn failed(err: anyhow::Error) -> Self {
        Self {
            session_id: None,
            result: Err(err),
            dependencies: Vec::new(),
            missing_dependencies: Vec::new(),
        }
    }
}

pub async fn install_package(
    state: &Arc<LauncherState>,
    provider: oneclient_content::packages::ProviderId,
    project_id: &str,
    version_id: &str,
    cluster_id: i64,
) -> PackageInstall {
    let lookup = async {
        let provider_impl = state.services.packages.get(provider)?;
        let project = provider_impl
            .get_project(project_id, &state.services.content())
            .await?;
        let version = provider_impl
            .get_version(project_id, version_id, &state.services.content())
            .await?;
        anyhow::Ok((project, version))
    }
    .await;

    let (project, version) = match lookup {
        Ok(found) => found,
        Err(err) => return PackageInstall::failed(err),
    };

    // Resolved before the session starts so its children can be announced up front
    let mut resolution = oneclient_content::packages::DependencyResolution::default();
    if oneclient_content::packages::resolves_dependencies(project.content_type) {
        match oneclient_content::packages::resolve_required(
            provider,
            &version,
            cluster_id,
            &state.services.content(),
        )
        .await
        {
            Ok(resolved) => resolution = resolved,
            Err(err) => {
                tracing::warn!(%err, "dependency resolution failed, installing package alone");
            }
        }
    }

    let session = oneclient_events::GroupedProgressSession::start(
        &state.services.events,
        format!("Installing {}", project.name),
    );

    let size = version.primary_file().map(|f| f.size).unwrap_or(0);
    let dependency_bytes: u64 = resolution
        .install
        .iter()
        .map(|dep| dep.version.primary_file().map(|f| f.size).unwrap_or(0))
        .sum();
    session.expect(
        oneclient_events::TaskCategory::Packages,
        1 + resolution.install.len() as u64,
        size + dependency_bytes,
    );

    let mut installed_dependencies = Vec::new();
    let mut missing_dependencies = resolution.unresolved;

    // Dependencies first so the package is never in a cluster without them however the run ends
    for dependency in &resolution.install {
        let child = session.child(
            dependency.project.name.clone(),
            dependency
                .version
                .primary_file()
                .map(|f| f.size)
                .unwrap_or(0),
            oneclient_events::TaskCategory::Packages,
        );

        let result = PackageStore::install_to_cluster(
            provider,
            &dependency.project,
            &dependency.version,
            cluster_id,
            false,
            false,
            Some(&child),
            &state.services.content(),
        )
        .await;

        child.finish();

        match result {
            Ok(artifact) => {
                mark_new(cluster_id, &artifact.hash, state).await;
                installed_dependencies.push(dependency.project.name.clone());
            }
            Err(err) => {
                tracing::warn!(
                    dependency = %dependency.project.name,
                    %err,
                    "failed to install dependency"
                );
                missing_dependencies.push(dependency.project.name.clone());
            }
        }
    }

    let child = session.child(
        project.name.clone(),
        size,
        oneclient_events::TaskCategory::Packages,
    );

    let result = PackageStore::install_to_cluster(
        provider,
        &project,
        &version,
        cluster_id,
        false,
        false,
        Some(&child),
        &state.services.content(),
    )
    .await;

    child.finish();

    if let Ok(artifact) = &result {
        mark_new(cluster_id, &artifact.hash, state).await;
    }

    PackageInstall {
        session_id: Some(session.detach()),
        result: result.map(|_| project.name).map_err(anyhow::Error::from),
        dependencies: installed_dependencies,
        missing_dependencies,
    }
}

async fn mark_new(cluster_id: i64, hash: &str, state: &LauncherState) {
    if let Err(err) =
        PackageStore::mark_artifact_new(cluster_id, hash, &state.services.content()).await
    {
        tracing::warn!(cluster_id, hash, %err, "failed to badge package as new");
    }
}
