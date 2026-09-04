use freya::query::{Mutation, MutationCapability, QueriesStorage, UseMutation, use_mutation};
use oneclient_content::packages::LiveSync;
use oneclient_db::models::ClusterId;

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
    timed("cluster_content", QueriesStorage::<ClusterContentQuery>::invalidate_all()).await;
    timed("bundle_overrides", QueriesStorage::<BundleOverridesQuery>::invalidate_all()).await;
    timed("bundles_with_status", QueriesStorage::<BundlesWithStatusQuery>::invalidate_all()).await;
    timed("clusters", QueriesStorage::<ListClustersQuery>::invalidate_all()).await;
    timed("bundle_updates", QueriesStorage::<BundleUpdatesQuery>::invalidate_all()).await;
    timed("package_updates", QueriesStorage::<PackageUpdatesQuery>::invalidate_all()).await;
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

pub async fn invalidate_profile_queries() {
    QueriesStorage::<ListNamedProfilesQuery>::invalidate_all().await;
    QueriesStorage::<GameProfileQuery>::invalidate_all().await;
    QueriesStorage::<ClusterProfileQuery>::invalidate_all().await;
    QueriesStorage::<ClusterSettingsQuery>::invalidate_all().await;
    QueriesStorage::<ListClustersQuery>::invalidate_all().await;
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ClusterMutation;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ClusterAction {
    ToggleArtifact {
        cluster_id: ClusterId,
        hash: String,
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
            ClusterAction::ToggleArtifact { cluster_id, hash } => {
                oneclient_core::toggle_artifact_enabled(*cluster_id, hash, content)
                    .await
                    .map(|(_, live)| {
                        if live == LiveSync::Deferred && state.games.is_active(*cluster_id) {
                            services
                                .events
                                .notify("Saved for the next launch")
                                .body("Minecraft is running, but this could not be added to the open game.")
                                .send();
                        }
                    })
            }
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
        };
        tracing::debug!(
            target: "oneclient_app::perf",
            ms = started.elapsed().as_millis() as u64,
            ok = result.is_ok(),
            "cluster action ran"
        );
        result.map_err(|e| e.to_string())
    }

    async fn on_settled(&self, _keys: &ClusterAction, result: &Result<(), String>) {
        if let Err(err) = result
            && let Ok(state) = crate::launcher::state()
        {
            state.services.events.notify("Action failed").body(err).error().send();
        }
        invalidate_cluster_queries().await;
    }
}

pub fn use_cluster_mutation() -> UseMutation<ClusterMutation> {
    use_mutation(Mutation::new(ClusterMutation))
}
