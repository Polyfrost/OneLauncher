use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use bytes::Bytes;
use freya::query::{
    Mutation, MutationCapability, QueriesStorage, Query, QueryCapability, QueryStateData,
    UseMutation, UseQuery, use_mutation, use_query,
};
use oneclient_core::clusters::ClusterManager;
use oneclient_core::{LauncherError, LauncherState, ScreenshotInfo};
use tokio::sync::Semaphore;

static LOCAL_IMAGE_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterScreenshotsKeys {
    pub cluster_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterScreenshotsQuery;

impl QueryCapability for ClusterScreenshotsQuery {
    type Ok = Vec<ScreenshotInfo>;
    type Err = LauncherError;
    type Keys = ClusterScreenshotsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get()?;
        let cluster = ClusterManager::get(&state, keys.cluster_id).await?;
        oneclient_core::list_cluster_screenshots(&cluster)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalImageKeys {
    pub path: PathBuf,
    pub max_edge: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalImageQuery;

impl QueryCapability for LocalImageQuery {
    type Ok = Bytes;
    type Err = LauncherError;
    type Keys = LocalImageKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let path = keys.path.clone();
        let max_edge = (keys.max_edge != 0).then_some(keys.max_edge);

        let _permit = if max_edge.is_some() {
            let sem = LOCAL_IMAGE_SEMAPHORE
                .get_or_init(|| Arc::new(Semaphore::new(1)))
                .clone();
            Some(
                sem.acquire_owned()
                    .await
                    .map_err(|_| LauncherError::Minecraft("local image semaphore closed".into()))?,
            )
        } else {
            None
        };

        tokio::task::spawn_blocking(move || oneclient_core::load_screenshot(&path, max_edge))
            .await
            .map_err(|e| LauncherError::Minecraft(e.to_string()))?
    }
}

pub fn use_cluster_screenshots(cluster_id: i64) -> UseQuery<ClusterScreenshotsQuery> {
    use_query(Query::new(
        ClusterScreenshotsKeys { cluster_id },
        ClusterScreenshotsQuery,
    ))
}

pub fn use_local_image(path: PathBuf, max_edge: u32) -> UseQuery<LocalImageQuery> {
    use_query(Query::new(
        LocalImageKeys { path, max_edge },
        LocalImageQuery,
    ))
}

pub fn try_cluster_screenshots(
    query: &UseQuery<ClusterScreenshotsQuery>,
) -> Option<Vec<ScreenshotInfo>> {
    match &*query.read().state() {
        QueryStateData::Settled { res: Ok(value), .. } => Some(value.clone()),
        QueryStateData::Loading {
            res: Some(Ok(value)),
        } => Some(value.clone()),
        _ => None,
    }
}

pub async fn invalidate_screenshots_queries() {
    QueriesStorage::<ClusterScreenshotsQuery>::try_invalidate_all().await;
    QueriesStorage::<LocalImageQuery>::try_invalidate_all().await;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ScreenshotAction {
    Delete { path: PathBuf },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScreenshotActionMutation;

impl MutationCapability for ScreenshotActionMutation {
    type Ok = ();
    type Err = LauncherError;
    type Keys = ScreenshotAction;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        match keys {
            ScreenshotAction::Delete { path } => oneclient_core::delete_screenshot(path),
        }
    }

    async fn on_settled(&self, _keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if result.is_ok() {
            invalidate_screenshots_queries().await;
        }
    }
}

pub type UseScreenshotAction = UseMutation<ScreenshotActionMutation>;

pub fn use_screenshot_action() -> UseScreenshotAction {
    use_mutation(Mutation::new(ScreenshotActionMutation))
}
