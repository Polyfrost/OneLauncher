
use std::path::PathBuf;

use freya::query::{Mutation, MutationCapability, QueriesStorage, UseMutation, use_mutation};
use oneclient_core::LauncherState;
use oneclient_core::packages::{ContentType, PackageStore};
use oneclient_db::models::ClusterId;

use super::bundles::{BundleOverridesQuery, BundleUpdatesQuery, BundlesWithStatusQuery};
use super::cluster_content::ClusterContentQuery;
use super::clusters::ListClustersQuery;
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
    QueriesStorage::<VersionsMetadataQuery>::try_invalidate_all().await;
    QueriesStorage::<LoaderVersionsQuery>::try_invalidate_all().await;
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
    ToggleArtifact { cluster_id: ClusterId, hash: String },
    RemoveArtifact { cluster_id: ClusterId, hash: String },
    RemoveBundlePackageFromDisk { cluster_id: ClusterId, hash: String },
    SetBundlePackageEnabled {
        cluster_id: ClusterId,
        bundle_name: String,
        package_id: String,
        enabled: bool,
    },
    ImportLocalFile {
        cluster_id: ClusterId,
        content_type: ContentType,
        path: PathBuf,
    },
    SetDedicatedDir {
        cluster_id: ClusterId,
        dedicated: bool,
    },
}

impl MutationCapability for ClusterMutation {
    type Ok = ();
    type Err = String;
    type Keys = ClusterAction;

    async fn run(&self, keys: &ClusterAction) -> Result<(), String> {
        let state = LauncherState::get().map_err(|e| e.to_string())?;
        let services = &state.services;
        let result = match keys {
            ClusterAction::ToggleArtifact { cluster_id, hash } => {
                PackageStore::toggle_artifact_enabled(*cluster_id, hash, services)
                    .await
                    .map(|_| ())
            }
            ClusterAction::RemoveArtifact { cluster_id, hash } => {
                PackageStore::unlink_from_cluster(hash, *cluster_id, services).await
            }
            ClusterAction::RemoveBundlePackageFromDisk { cluster_id, hash } => {
                PackageStore::unlink_from_cluster_system(hash, *cluster_id, services).await
            }
            ClusterAction::SetBundlePackageEnabled {
                cluster_id,
                bundle_name,
                package_id,
                enabled,
            } => {
                oneclient_core::set_bundle_package_enabled(
                    *cluster_id,
                    bundle_name,
                    package_id,
                    *enabled,
                    services,
                )
                .await
            }
            ClusterAction::ImportLocalFile {
                cluster_id,
                content_type,
                path,
            } => PackageStore::import_local_file(path, *content_type, *cluster_id, services)
                .await
                .map(|row| {
                    services
                        .notifier
                        .send_info("Imported", &format!("Added {}", row.file_name));
                }),
            ClusterAction::SetDedicatedDir {
                cluster_id,
                dedicated,
            } => oneclient_core::clusters::ClusterManager::set_dedicated_dir(
                &state, *cluster_id, *dedicated,
            )
            .await,
        };
        result.map_err(|e| e.to_string())
    }

    async fn on_settled(&self, _keys: &ClusterAction, result: &Result<(), String>) {
        if let Err(err) = result
            && let Ok(state) = LauncherState::get()
        {
            state.services.notifier.send_error("Action failed", err);
        }
        invalidate_cluster_queries().await;
    }
}

pub fn use_cluster_mutation() -> UseMutation<ClusterMutation> {
    use_mutation(Mutation::new(ClusterMutation))
}
