
use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::packages::{ContentType, PackageStore};
use oneclient_core::{LauncherError, LauncherState, LinkedArtifactInfo};
use oneclient_db::models::ClusterId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterContentQuery {
    pub cluster_id: ClusterId,
    pub content_type: ContentType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterContentKeys {
    pub cluster_id: ClusterId,
    pub content_type: ContentType,
}

impl QueryCapability for ClusterContentQuery {
    type Ok = Vec<LinkedArtifactInfo>;
    type Err = LauncherError;
    type Keys = ClusterContentKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get()?;
        let all = PackageStore::list_linked_artifacts(self.cluster_id, &state.services).await?;
        Ok(all
            .into_iter()
            .filter(|item| item.content_type == self.content_type)
            .collect())
    }
}

pub fn use_cluster_content(
    cluster_id: ClusterId,
    content_type: ContentType,
) -> UseQuery<ClusterContentQuery> {
    use_query(Query::new(
        ClusterContentKeys {
            cluster_id,
            content_type,
        },
        ClusterContentQuery {
            cluster_id,
            content_type,
        },
    ))
}

pub fn cluster_content_items(query: &UseQuery<ClusterContentQuery>) -> Vec<LinkedArtifactInfo> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. } => list.clone(),
        QueryStateData::Loading { res: Some(Ok(list)) } => list.clone(),
        _ => Vec::new(),
    }
}
