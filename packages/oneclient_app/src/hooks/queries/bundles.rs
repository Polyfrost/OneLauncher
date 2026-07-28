use std::collections::HashMap;

use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::clusters::Cluster;
use oneclient_core::{
    BundleArchive, BundleUpdateCheckResult, BundleWithUpdateStatus, LauncherError,
    get_bundles_with_update_status, list_cluster_bundle_overrides,
};
use oneclient_db::models::ClusterId;

#[derive(Clone, Debug)]
pub struct ClusterBundles {
    pub cluster: Cluster,
    pub archives: Vec<BundleArchive>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OnboardingBundlesQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OnboardingBundlesKeys;

impl QueryCapability for OnboardingBundlesQuery {
    type Ok = Vec<ClusterBundles>;
    type Err = LauncherError;
    type Keys = OnboardingBundlesKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let clusters = state.clusters.list().await?;

        let mut out = Vec::with_capacity(clusters.len());
        for cluster in clusters {
            let archives = state
                .bundles
                .archives_for(&state.services.content(), &cluster.mc_version, cluster.mc_loader)
                .await
                .unwrap_or_default();
            out.push(ClusterBundles { cluster, archives });
        }
        Ok(out)
    }
}

pub fn use_onboarding_bundles() -> UseQuery<OnboardingBundlesQuery> {
    use_query(Query::new(OnboardingBundlesKeys, OnboardingBundlesQuery))
}

pub fn onboarding_bundles_items(
    query: &UseQuery<OnboardingBundlesQuery>,
) -> Option<Vec<ClusterBundles>> {
    super::state::settled_or_loading(query)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundlesWithStatusQuery {
    pub cluster_id: ClusterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundlesWithStatusKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for BundlesWithStatusQuery {
    type Ok = Vec<BundleWithUpdateStatus>;
    type Err = LauncherError;
    type Keys = BundlesWithStatusKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let _ = keys;
        let state = crate::launcher::state()?;
        Ok(
            get_bundles_with_update_status(
                self.cluster_id,
                state.bundles.as_ref(),
                &state.services.content(),
            )
            .await?,
        )
    }
}

pub fn bundles_with_status_items(
    query: &UseQuery<BundlesWithStatusQuery>,
) -> Vec<BundleWithUpdateStatus> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

pub fn use_bundles_with_status(cluster_id: ClusterId) -> UseQuery<BundlesWithStatusQuery> {
    use_query(Query::new(
        BundlesWithStatusKeys { cluster_id },
        BundlesWithStatusQuery { cluster_id },
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundleOverridesQuery {
    pub cluster_id: ClusterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundleOverridesKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for BundleOverridesQuery {
    type Ok = HashMap<(String, String), String>;
    type Err = LauncherError;
    type Keys = BundleOverridesKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let _ = keys;
        let state = crate::launcher::state()?;
        let rows = list_cluster_bundle_overrides(self.cluster_id, &state.services.content()).await?;
        Ok(rows
            .into_iter()
            .map(|(bundle, pkg, ty)| ((bundle, pkg), ty))
            .collect())
    }
}

pub fn use_bundle_overrides(cluster_id: ClusterId) -> UseQuery<BundleOverridesQuery> {
    use_query(Query::new(
        BundleOverridesKeys { cluster_id },
        BundleOverridesQuery { cluster_id },
    ))
}

pub fn bundle_overrides_map(
    query: &UseQuery<BundleOverridesQuery>,
) -> HashMap<(String, String), String> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundleUpdatesQuery {
    pub cluster_id: ClusterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundleUpdatesKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for BundleUpdatesQuery {
    type Ok = BundleUpdateCheckResult;
    type Err = LauncherError;
    type Keys = BundleUpdatesKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let _ = keys;
        let state = crate::launcher::state()?;
        Ok(oneclient_core::check_bundle_updates(
            self.cluster_id,
            state.bundles.as_ref(),
            &state.services.content(),
        )
        .await?)
    }
}

pub fn use_bundle_updates(cluster_id: ClusterId) -> UseQuery<BundleUpdatesQuery> {
    use_query(Query::new(
        BundleUpdatesKeys { cluster_id },
        BundleUpdatesQuery { cluster_id },
    ))
}
