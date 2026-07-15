use std::collections::HashMap;

use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::clusters::{Cluster, ClusterManager};
use oneclient_core::{
    BundleArchive, BundleUpdateCheckResult, BundleWithUpdateStatus, LauncherError, LauncherState,
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
        let state = LauncherState::get()?;
        let clusters = ClusterManager::list(&state).await?;

        let mut out = Vec::with_capacity(clusters.len());
        for cluster in clusters {
            let archives = state
                .bundles
                .archives_for(&state.services, &cluster.mc_version, cluster.mc_loader)
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
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. } => Some(list.clone()),
        QueryStateData::Loading {
            res: Some(Ok(list)),
        } => Some(list.clone()),
        _ => None,
    }
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
        let state = LauncherState::get()?;
        get_bundles_with_update_status(self.cluster_id, state.bundles.as_ref(), &state.services)
            .await
    }
}

pub fn bundles_with_status_items(
    query: &UseQuery<BundlesWithStatusQuery>,
) -> Vec<BundleWithUpdateStatus> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. } => list.clone(),
        QueryStateData::Loading {
            res: Some(Ok(list)),
        } => list.clone(),
        _ => Vec::new(),
    }
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
        let state = LauncherState::get()?;
        let rows = list_cluster_bundle_overrides(self.cluster_id, &state.services).await?;
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
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(map), .. } => map.clone(),
        QueryStateData::Loading { res: Some(Ok(map)) } => map.clone(),
        _ => HashMap::new(),
    }
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
        let state = LauncherState::get()?;
        oneclient_core::check_bundle_updates(
            self.cluster_id,
            state.bundles.as_ref(),
            &state.services,
        )
        .await
    }
}

pub fn use_bundle_updates(cluster_id: ClusterId) -> UseQuery<BundleUpdatesQuery> {
    use_query(Query::new(
        BundleUpdatesKeys { cluster_id },
        BundleUpdatesQuery { cluster_id },
    ))
}
