use std::collections::HashSet;
use std::sync::Arc;

use oneclient_cluster::Cluster;
use oneclient_content::bundles::BundleArchive;
use oneclient_db::models::OverrideType;
use oneclient_events::GroupedProgressSession;

use crate::state::LauncherState;

pub async fn set_up_bundles(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    selected: Option<&[String]>,
    session: &GroupedProgressSession,
) {
    let content = state.services.content();

    if let Some(selected) = selected {
        let archives = state
            .bundles
            .archives_for(&content, &cluster.mc_version, cluster.mc_loader)
            .await
            .unwrap_or_default();
        let overrides = bundle_selection_overrides(&archives, selected);
        if let Err(err) = oneclient_content::bundles::set_bundle_package_overrides(
            cluster.id, &overrides, &content,
        )
        .await
        {
            tracing::warn!(cluster_id = cluster.id, error = %err, "failed to record the bundle choices for the new instance");
        }
        let choices: Vec<_> = archives
            .iter()
            .filter(|archive| !archive.manifest.enabled)
            .map(|archive| {
                let name = &archive.manifest.name;
                (name.clone(), selected.contains(name))
            })
            .collect();
        if let Err(err) =
            oneclient_content::bundles::set_bundle_choices(cluster.id, &choices, &content).await
        {
            tracing::warn!(cluster_id = cluster.id, error = %err, "failed to record the bundle choices for the new instance");
        }
    }

    if let Err(err) = oneclient_content::bundles::install_cluster_bundles(
        cluster.id,
        state.bundles.as_ref(),
        Some(session),
        &content,
    )
    .await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "failed to install bundle content for the new instance");
    }
    if let Err(err) =
        super::apply_bundle_java_override(state, cluster.id, true, true, Some(session)).await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "failed to apply the bundle java override for the new instance");
    }
}

fn bundle_selection_overrides(
    archives: &[BundleArchive],
    selected: &[String],
) -> Vec<(String, String, OverrideType)> {
    let taken = |archive: &BundleArchive| selected.contains(&archive.manifest.name);

    let mut kept: HashSet<String> = HashSet::new();
    for archive in archives.iter().filter(|archive| taken(archive)) {
        for file in archive.manifest.files.iter().filter(|file| file.enabled) {
            kept.insert(file.kind.package_id());
        }
    }

    let mut overrides = Vec::new();
    for archive in archives.iter().filter(|archive| !taken(archive)) {
        for file in archive.manifest.files.iter().filter(|file| file.enabled) {
            let package_id = file.kind.package_id();
            if kept.contains(&package_id) {
                continue;
            }
            overrides.push((
                archive.manifest.name.clone(),
                package_id,
                OverrideType::Removed,
            ));
        }
    }
    overrides
}
