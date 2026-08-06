use freya::query::{Mutation, MutationCapability, QueriesStorage, UseMutation, use_mutation};
use oneclient_db::models::{ClusterId, OverrideType};

use super::bundles::{BundleOverridesQuery, BundleUpdatesQuery, BundlesWithStatusQuery};
use super::cluster_content::ClusterContentQuery;
use super::clusters::ListClustersQuery;
use super::package_updates::PackageUpdatesQuery;
use super::settings_profiles::{
    ClusterProfileQuery, ClusterSettingsQuery, GameProfileQuery, ListNamedProfilesQuery,
};
use super::versions::{LoaderVersionsQuery, VersionsMetadataQuery};

pub async fn invalidate_cluster_queries() {
    QueriesStorage::<ListClustersQuery>::try_invalidate_all().await;
    QueriesStorage::<ClusterContentQuery>::try_invalidate_all().await;
    QueriesStorage::<BundlesWithStatusQuery>::try_invalidate_all().await;
    QueriesStorage::<BundleOverridesQuery>::try_invalidate_all().await;
    QueriesStorage::<BundleUpdatesQuery>::try_invalidate_all().await;
    QueriesStorage::<PackageUpdatesQuery>::try_invalidate_all().await;
    QueriesStorage::<VersionsMetadataQuery>::try_invalidate_all().await;
    QueriesStorage::<LoaderVersionsQuery>::try_invalidate_all().await;
}

/// Just the query that decides whether a cluster already has a package.
///
/// Split out of [`invalidate_cluster_queries`] so an install can wait for
/// exactly this before it stops calling itself busy. The full sweep reaches the
/// network several times over, and waiting for all of it would trade one stale
/// button for a slower one.
pub async fn invalidate_cluster_content_queries() {
    QueriesStorage::<ClusterContentQuery>::try_invalidate_all().await;
}

pub async fn invalidate_profile_queries() {
    QueriesStorage::<ListNamedProfilesQuery>::try_invalidate_all().await;
    QueriesStorage::<GameProfileQuery>::try_invalidate_all().await;
    QueriesStorage::<ClusterProfileQuery>::try_invalidate_all().await;
    QueriesStorage::<ClusterSettingsQuery>::try_invalidate_all().await;
    QueriesStorage::<ListClustersQuery>::try_invalidate_all().await;
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
        /// The bundle manifest's `enabled` flag, so we know whether the user's
        /// choice matches the default (clear the override) or contradicts it
        /// (write `Enabled` / `Disabled`).
        manifest_default: bool,
    },
    SetDedicatedDir {
        cluster_id: ClusterId,
        dedicated: bool,
    },
    /// Rehash every installed file and re-download whatever no longer matches.
    VerifyFiles {
        cluster_id: ClusterId,
    },
}

impl MutationCapability for ClusterMutation {
    type Ok = ();
    type Err = String;
    type Keys = ClusterAction;

    async fn run(&self, keys: &ClusterAction) -> Result<(), String> {
        let state = crate::launcher::state().map_err(|e| e.to_string())?;
        let services = &state.services;
        let content = &state.services.content();
        let result = match keys {
            ClusterAction::ToggleArtifact { cluster_id, hash } => {
                // Recorded in the database and applied to the game folder at the
                // next launch, whether or not a session is live: Minecraft reads
                // its mods once at startup, so there is nothing a mid-session
                // write could achieve.
                oneclient_core::toggle_artifact_enabled(*cluster_id, hash, content)
                    .await
                    .map(|_| ())
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
                // Matching the manifest default means "no opinion" -> clear the
                // override. Contradicting it pins the choice in either direction.
                let override_type = match (*enabled, *manifest_default) {
                    (true, true) | (false, false) => None,
                    (true, false) => Some(OverrideType::Enabled),
                    (false, true) => Some(OverrideType::Disabled),
                };
                oneclient_core::set_bundle_package_override(
                    *cluster_id,
                    bundle_name,
                    package_id,
                    override_type,
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
                // Reports its own outcome rather than going through the generic
                // failure toast: a verify that finds nothing wrong is a useful
                // result the user asked for, not a silent no-op.
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
