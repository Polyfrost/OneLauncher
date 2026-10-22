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

/// A lookup not a fetch there is no per-cluster query the list is always
/// loaded and small A hook because it subscribes to the list
pub fn use_cluster(cluster_id: i64) -> Option<Cluster> {
    let clusters = super::state::settled_or_loading(&use_clusters()).unwrap_or_default();
    clusters.into_iter().find(|c| c.id == cluster_id)
}
