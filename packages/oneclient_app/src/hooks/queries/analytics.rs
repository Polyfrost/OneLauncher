use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::LauncherError;
use oneclient_core::game::{Analytics, cluster_analytics, global_analytics};


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlobalAnalyticsKeys;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlobalAnalyticsQuery;

impl QueryCapability for GlobalAnalyticsQuery {
    type Ok = Analytics;
    type Err = LauncherError;
    type Keys = GlobalAnalyticsKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        global_analytics().await
    }
}

pub fn use_global_analytics() -> UseQuery<GlobalAnalyticsQuery> {
    use_query(Query::new(GlobalAnalyticsKeys, GlobalAnalyticsQuery))
}

pub fn try_global_analytics(query: &UseQuery<GlobalAnalyticsQuery>) -> Option<Analytics> {
    match &*query.read().state() {
        QueryStateData::Settled { res: Ok(value), .. } => Some(value.clone()),
        QueryStateData::Loading {
            res: Some(Ok(value)),
        } => Some(value.clone()),
        _ => None,
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterAnalyticsKeys {
    pub cluster_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterAnalyticsQuery;

impl QueryCapability for ClusterAnalyticsQuery {
    type Ok = Analytics;
    type Err = LauncherError;
    type Keys = ClusterAnalyticsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        cluster_analytics(keys.cluster_id).await
    }
}

pub fn use_cluster_analytics(cluster_id: i64) -> UseQuery<ClusterAnalyticsQuery> {
    use_query(Query::new(
        ClusterAnalyticsKeys { cluster_id },
        ClusterAnalyticsQuery,
    ))
}

pub fn try_cluster_analytics(query: &UseQuery<ClusterAnalyticsQuery>) -> Option<Analytics> {
    match &*query.read().state() {
        QueryStateData::Settled { res: Ok(value), .. } => Some(value.clone()),
        QueryStateData::Loading {
            res: Some(Ok(value)),
        } => Some(value.clone()),
        _ => None,
    }
}
