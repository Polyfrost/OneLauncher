use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::LauncherError;
use oneclient_core::clusters::Cluster;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListClustersQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListClustersKeys;

impl QueryCapability for ListClustersQuery {
    type Ok = Vec<Cluster>;
    type Err = LauncherError;
    type Keys = ListClustersKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        Ok(state.clusters.list().await?)
    }
}

pub fn use_clusters() -> UseQuery<ListClustersQuery> {
    use_query(Query::new(ListClustersKeys, ListClustersQuery))
}

/// The one cluster a route is about, pulled out of the list query.
///
/// There is no per-cluster query, since the list is always loaded and small, so
/// this is a lookup, not a fetch. It is a hook because it subscribes to the
/// list on the caller's behalf.
pub fn use_cluster(cluster_id: i64) -> Option<Cluster> {
    let clusters = super::state::settled_or_loading(&use_clusters()).unwrap_or_default();
    clusters.into_iter().find(|c| c.id == cluster_id)
}
