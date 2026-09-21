use freya::query::{Mutation, MutationCapability, QueriesStorage, UseMutation, use_mutation};
use oneclient_common::domain::GameLoader;
use oneclient_content::packages::LiveSync;
use oneclient_core::BundleArchive;
use oneclient_db::models::{ClusterId, ClusterKind, OverrideType};
use std::collections::HashSet;
use std::path::PathBuf;

use super::bundles::{BundleOverridesQuery, BundleUpdatesQuery, BundlesWithStatusQuery};
use super::cluster_content::ClusterContentQuery;
use super::clusters::ListClustersQuery;
use super::package_updates::PackageUpdatesQuery;
use super::settings_profiles::{
    ClusterProfileQuery, ClusterSettingsQuery, GameProfileQuery, ListNamedProfilesQuery,
};

async fn timed(step: &'static str, fut: impl std::future::Future<Output = ()>) {
    let started = std::time::Instant::now();
    fut.await;
    tracing::debug!(
        target: "oneclient_app::perf",
        step,
        ms = started.elapsed().as_millis() as u64,
        "invalidate step"
    );
}

pub async fn invalidate_cluster_queries() {
    let started = std::time::Instant::now();
    timed(
        "cluster_content",
        QueriesStorage::<ClusterContentQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundle_overrides",
        QueriesStorage::<BundleOverridesQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundles_with_status",
        QueriesStorage::<BundlesWithStatusQuery>::invalidate_all(),
    )
    .await;
    timed(
        "clusters",
        QueriesStorage::<ListClustersQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundle_updates",
        QueriesStorage::<BundleUpdatesQuery>::invalidate_all(),
    )
    .await;
    timed(
        "package_updates",
        QueriesStorage::<PackageUpdatesQuery>::invalidate_all(),
    )
    .await;
    tracing::debug!(
        target: "oneclient_app::perf",
        ms = started.elapsed().as_millis() as u64,
        "cluster queries invalidated"
    );
}

/// Split out of [`invalidate_cluster_queries`] so an install can wait for just
/// this before dropping its busy flag
pub async fn invalidate_cluster_content_queries() {
    QueriesStorage::<ClusterContentQuery>::invalidate_all().await;
}

/// Everything [`invalidate_cluster_queries`] does bar the cluster list and the
/// cached package updates neither of which an enabled flag can move
/// The bundle queries do move a toggle writes a bundle override and both read
/// those back
async fn invalidate_enabled_flag_queries() {
    timed(
        "cluster_content",
        QueriesStorage::<ClusterContentQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundle_overrides",
        QueriesStorage::<BundleOverridesQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundles_with_status",
        QueriesStorage::<BundlesWithStatusQuery>::invalidate_all(),
    )
    .await;
    timed(
        "bundle_updates",
        QueriesStorage::<BundleUpdatesQuery>::invalidate_all(),
    )
    .await;
}

pub async fn invalidate_profile_queries() {
    QueriesStorage::<ListNamedProfilesQuery>::invalidate_all().await;
    QueriesStorage::<GameProfileQuery>::invalidate_all().await;
    QueriesStorage::<ClusterProfileQuery>::invalidate_all().await;
    QueriesStorage::<ClusterSettingsQuery>::invalidate_all().await;
    QueriesStorage::<ListClustersQuery>::invalidate_all().await;
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

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ClusterMutation;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ClusterAction {
    SetArtifactEnabled {
        cluster_id: ClusterId,
        hash: String,
        enabled: bool,
    },
    RemoveArtifact {
        cluster_id: ClusterId,
        hash: String,
    },
    RemoveBundlePackageFromDisk {
        cluster_id: ClusterId,
        hash: String,
    },
    SetBundlePackageEnabled {
        cluster_id: ClusterId,
        bundle_name: String,
        package_id: String,
        enabled: bool,
        /// Manifest default matching it clears the override contradicting it
        /// writes `Enabled` / `Disabled`
        manifest_default: bool,
    },
    SetDedicatedDir {
        cluster_id: ClusterId,
        dedicated: bool,
    },
    VerifyFiles {
        cluster_id: ClusterId,
    },
    CreateInstance {
        kind: ClusterKind,
        name: String,
        mc_version: String,
        mc_loader: GameLoader,
        mc_loader_version: Option<String>,
        description: Option<String>,
        tags: Vec<String>,
        cover_source: Option<PathBuf>,
        bundles: Option<Vec<String>>,
    },
    UpdateInstance {
        cluster_id: ClusterId,
        name: String,
        description: Option<String>,
        tags: Vec<String>,
        cover_source: Option<PathBuf>,
        clear_cover: bool,
    },
    DeleteInstance {
        cluster_id: ClusterId,
    },
}

impl MutationCapability for ClusterMutation {
    type Ok = ();
    type Err = String;
    type Keys = ClusterAction;

    async fn run(&self, keys: &ClusterAction) -> Result<(), String> {
        let started = std::time::Instant::now();
        let state = crate::launcher::state().map_err(|e| e.to_string())?;
        let services = &state.services;
        let content = &state.services.content();
        let result = match keys {
            ClusterAction::SetArtifactEnabled {
                cluster_id,
                hash,
                enabled,
            } => oneclient_core::set_artifact_enabled_to(*cluster_id, hash, *enabled, content)
                .await
                .map(|live| {
                    if live == LiveSync::Deferred && state.games.is_active(*cluster_id) {
                        services
                            .events
                            .notify("Saved for the next launch")
                            .body("Minecraft is running, but this could not be added to the open game.")
                            .send();
                    }
                }),
            ClusterAction::RemoveArtifact { cluster_id, hash } => {
                oneclient_core::remove_artifact_from_cluster(*cluster_id, hash, true, content).await
            }
            ClusterAction::RemoveBundlePackageFromDisk { cluster_id, hash } => {
                oneclient_core::remove_artifact_from_cluster(*cluster_id, hash, false, content).await
            }
            ClusterAction::SetBundlePackageEnabled {
                cluster_id,
                bundle_name,
                package_id,
                enabled,
                manifest_default,
            } => {
                oneclient_core::set_bundle_package_enabled(
                    *cluster_id,
                    bundle_name,
                    package_id,
                    *enabled,
                    *manifest_default,
                    content,
                )
                .await
            }
            ClusterAction::SetDedicatedDir {
                cluster_id,
                dedicated,
            } => {
                state
                    .clusters
                    .set_dedicated_dir(
                        *cluster_id,
                        *dedicated,
                        state.games.is_active(*cluster_id),
                    )
                    .await
                .map_err(|err| oneclient_content::ContentError::InvalidData {
                    reason: err.to_string(),
                })
            }
            ClusterAction::VerifyFiles { cluster_id } => {
                // Reports its own outcome not the generic failure toast a
                // verify that finds nothing wrong is still a useful result
                match oneclient_core::verify_cluster_files(&state, *cluster_id).await {
                    Ok(report) => {
                        let notify = services.events.notify("Verification complete");
                        let notify = notify.body(report.summary());
                        if report.unrepairable.is_empty() {
                            notify.send();
                        } else {
                            notify.error().send();
                        }
                        Ok(())
                    }
                    Err(err) => Err(oneclient_content::ContentError::InvalidData {
                        reason: err.to_string(),
                    }),
                }
            }
            ClusterAction::CreateInstance {
                kind,
                name,
                mc_version,
                mc_loader,
                mc_loader_version,
                description,
                tags,
                cover_source,
                bundles,
            } => {
                let global = state.settings.read().global_game_settings.clone();
                let mut options = oneclient_core::clusters::CreateClusterOptions::new(
                    name.clone(),
                    mc_version.clone(),
                    *mc_loader,
                )
                .kind(*kind)
                .user_created(true)
                .tags(tags.clone());
                options.mc_loader_version = mc_loader_version.clone();
                options.description = description.clone();

                match state.clusters.create(&global, options).await {
                    Ok(cluster) => {
                        services
                            .events
                            .signal(oneclient_events::Signal::ClustersChanged);

                        if let Some(source) = cover_source {
                            match state.clusters.set_cover_from_file(cluster.id, source).await {
                                Ok(file_name) => {
                                    let update = oneclient_core::clusters::ClusterUpdate {
                                        cover_path: oneclient_common::Patch::Set(file_name),
                                        ..Default::default()
                                    };
                                    if let Err(err) =
                                        state.clusters.update(cluster.id, update).await
                                    {
                                        tracing::warn!(cluster_id = cluster.id, error = %err, "failed to record the instance cover");
                                    }
                                }
                                Err(err) => {
                                    tracing::warn!(cluster_id = cluster.id, error = %err, "failed to store the instance cover");
                                }
                            }
                        }

                        if cluster.uses_bundles() {
                            if let Some(selected) = bundles {
                                let archives = state
                                    .bundles
                                    .archives_for(content, &cluster.mc_version, cluster.mc_loader)
                                    .await
                                    .unwrap_or_default();
                                let overrides = bundle_selection_overrides(&archives, selected);
                                if let Err(err) = oneclient_core::set_bundle_package_overrides(
                                    cluster.id, &overrides, content,
                                )
                                .await
                                {
                                    tracing::warn!(cluster_id = cluster.id, error = %err, "failed to record the bundle choices for the new instance");
                                }
                            }

                            let session = oneclient_events::GroupedProgressSession::start(
                                &services.events,
                                format!("Setting up {}", cluster.name),
                            );
                            if let Err(err) = oneclient_content::bundles::install_cluster_bundles(
                                cluster.id,
                                state.bundles.as_ref(),
                                Some(&session),
                                content,
                            )
                            .await
                            {
                                tracing::warn!(cluster_id = cluster.id, error = %err, "failed to install bundle content for the new instance");
                            }
                            session.finish();
                        }
                        Ok(())
                    }
                    Err(err) => Err(oneclient_content::ContentError::InvalidData {
                        reason: err.to_string(),
                    }),
                }
            }
            ClusterAction::UpdateInstance {
                cluster_id,
                name,
                description,
                tags,
                cover_source,
                clear_cover,
            } => {
                let cover = if *clear_cover {
                    state.clusters.clear_cover(*cluster_id).await.ok();
                    oneclient_common::Patch::Clear
                } else if let Some(source) = cover_source {
                    match state.clusters.set_cover_from_file(*cluster_id, source).await {
                        Ok(file_name) => oneclient_common::Patch::Set(file_name),
                        Err(err) => {
                            tracing::warn!(cluster_id, error = %err, "failed to store the instance cover");
                            oneclient_common::Patch::Unchanged
                        }
                    }
                } else {
                    oneclient_common::Patch::Unchanged
                };

                let update = oneclient_core::clusters::ClusterUpdate {
                    name: Some(name.clone()),
                    description: match description {
                        Some(text) => oneclient_common::Patch::Set(text.clone()),
                        None => oneclient_common::Patch::Clear,
                    },
                    tags: Some(tags.clone()),
                    cover_path: cover,
                    ..Default::default()
                };

                state
                    .clusters
                    .update(*cluster_id, update)
                    .await
                    .map(|_| ())
                    .map_err(|err| oneclient_content::ContentError::InvalidData {
                        reason: err.to_string(),
                    })
            }
            ClusterAction::DeleteInstance { cluster_id } => {
                if state.games.is_active(*cluster_id) {
                    Err(oneclient_content::ContentError::InvalidData {
                        reason: "Close the game before deleting this instance.".to_string(),
                    })
                } else {
                    let outcome = state
                        .clusters
                        .delete(*cluster_id, true)
                        .await
                        .map_err(|err| oneclient_content::ContentError::InvalidData {
                            reason: err.to_string(),
                        });
                    if outcome.is_ok() {
                        services
                            .events
                            .signal(oneclient_events::Signal::ClustersChanged);
                    }
                    outcome
                }
            }
        };
        tracing::debug!(
            target: "oneclient_app::perf",
            ms = started.elapsed().as_millis() as u64,
            ok = result.is_ok(),
            "cluster action ran"
        );
        result.map_err(|e| e.to_string())
    }

    async fn on_settled(&self, keys: &ClusterAction, result: &Result<(), String>) {
        if let Err(err) = result
            && let Ok(state) = crate::launcher::state()
        {
            state
                .services
                .events
                .notify("Action failed")
                .body(err)
                .error()
                .send();
        }
        if matches!(keys, ClusterAction::SetArtifactEnabled { .. }) {
            invalidate_enabled_flag_queries().await;
        } else {
            invalidate_cluster_queries().await;
        }
    }
}

pub fn use_cluster_mutation() -> UseMutation<ClusterMutation> {
    use_mutation(Mutation::new(ClusterMutation))
}
