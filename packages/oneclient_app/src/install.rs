//! Shared helpers for content installs and the notifications they raise.
//!
//! Lifted out of the bridge runtime so the actions layer can use them without
//! the command loop existing.

use std::sync::Arc;

use oneclient_core::LauncherState;

use oneclient_content::packages::PackageStore;
use oneclient_events::Level;

use crate::components::IconType;
use crate::notifications::{
    ClusterUpdateItem, ClusterUpdateSummary, NotificationAction, NotificationActionKind,
    NotificationSpec,
};

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

/// Builds a single notification summarising a batch bundle sync. Every changed
/// cluster rides along on one "View changes" action so the notification keeps
/// exactly two buttons no matter how many clusters moved; the modal does the
/// per-cluster breakdown. `None` when nothing changed.
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

pub async fn install_package(
    state: &Arc<LauncherState>,
    provider: oneclient_content::packages::ProviderId,
    project_id: &str,
    version_id: &str,
    cluster_id: i64,
) -> anyhow::Result<String> {
    let provider_impl = state.services.packages.get(provider)?;
    let project = provider_impl
        .get_project(project_id, &state.services.content())
        .await?;
    let version = provider_impl
        .get_version(project_id, version_id, &state.services.content())
        .await?;

    let session = oneclient_events::GroupedProgressSession::start(
        &state.services.events,
        format!("Installing {}", project.name),
    );
    let size = version.primary_file().map(|f| f.size).unwrap_or(0);
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
    session.finish();
    result?;
    Ok(project.name)
}
