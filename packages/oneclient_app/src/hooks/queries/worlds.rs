use std::future::Future;
use std::path::{Path, PathBuf};

use freya::prelude::spawn_forever;
use freya::query::{QueriesStorage, Query, QueryCapability, UseQuery, use_query};
use notify::{Event, EventKind, RecursiveMode};
use oneclient_core::{ClusterError, DataPackInfo, LauncherError, WorldInfo};

const LEVEL_DAT: &str = "level.dat";
const WORLD_ICON: &str = "icon.png";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterWorldsKeys {
    pub cluster_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterWorldsQuery;

impl QueryCapability for ClusterWorldsQuery {
    type Ok = Vec<WorldInfo>;
    type Err = LauncherError;
    type Keys = ClusterWorldsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let cluster = state.clusters.get(keys.cluster_id).await?;
        run_blocking(move || oneclient_core::list_cluster_worlds(&cluster)).await
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WorldDataPacksKeys {
    pub cluster_id: i64,
    pub world: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldDataPacksQuery;

impl QueryCapability for WorldDataPacksQuery {
    type Ok = Vec<DataPackInfo>;
    type Err = LauncherError;
    type Keys = WorldDataPacksKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        if keys.world.is_empty() {
            return Ok(Vec::new());
        }
        let state = crate::launcher::state()?;
        let cluster = state.clusters.get(keys.cluster_id).await?;
        Ok(oneclient_core::list_world_datapacks(&cluster, &keys.world).await?)
    }
}

pub fn use_cluster_worlds(cluster_id: i64) -> UseQuery<ClusterWorldsQuery> {
    use_query(Query::new(
        ClusterWorldsKeys { cluster_id },
        ClusterWorldsQuery,
    ))
}

pub fn use_world_datapacks(cluster_id: i64, world: String) -> UseQuery<WorldDataPacksQuery> {
    use_query(Query::new(
        WorldDataPacksKeys { cluster_id, world },
        WorldDataPacksQuery,
    ))
}

pub fn use_saves_folder_watch(folder: Option<PathBuf>, query: UseQuery<ClusterWorldsQuery>) {
    super::folder_watch::use_folder_watch(
        folder,
        RecursiveMode::Recursive,
        touches_world_list,
        move || query.invalidate(),
    );
}

fn touches_world_list(saves: &Path, event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }

    event.paths.iter().any(|path| {
        let parent = path.parent();
        parent == Some(saves)
            || (parent.and_then(Path::parent) == Some(saves)
                && path
                    .file_name()
                    .is_some_and(|name| name == LEVEL_DAT || name == WORLD_ICON))
    })
}

pub fn try_cluster_worlds(query: &UseQuery<ClusterWorldsQuery>) -> Option<Vec<WorldInfo>> {
    super::state::settled_or_loading(query)
}

pub fn try_world_datapacks(query: &UseQuery<WorldDataPacksQuery>) -> Option<Vec<DataPackInfo>> {
    super::state::settled_or_loading(query)
}

pub fn spawn_world_task(
    dispatch: crate::Actions,
    failure_title: &'static str,
    task: impl Future<Output = Result<(), LauncherError>> + 'static,
) {
    spawn_forever(async move {
        if let Err(err) = task.await {
            dispatch
                .notify(failure_title)
                .body(err.to_string())
                .error()
                .send();
        }
    });
}

async fn invalidate_worlds_queries() {
    QueriesStorage::<ClusterWorldsQuery>::invalidate_all().await;
    QueriesStorage::<WorldDataPacksQuery>::invalidate_all().await;
}

async fn run_blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, ClusterError> + Send + 'static,
) -> Result<T, LauncherError> {
    Ok(tokio::task::spawn_blocking(task)
        .await
        .map_err(|e| LauncherError::Minecraft(e.to_string()))??)
}

pub async fn add_world_datapacks(
    cluster_id: i64,
    world: String,
    files: Vec<PathBuf>,
) -> Result<(), LauncherError> {
    let state = crate::launcher::state()?;
    let cluster = state.clusters.get(cluster_id).await?;
    let result = oneclient_core::add_world_datapacks(&cluster, &world, &files).await;
    invalidate_worlds_queries().await;
    Ok(result?)
}

pub async fn delete_world(cluster_id: i64, world: String) -> Result<(), LauncherError> {
    let state = crate::launcher::state()?;
    let cluster = state.clusters.get(cluster_id).await?;
    let result = run_blocking(move || oneclient_core::delete_world(&cluster, &world)).await;
    invalidate_worlds_queries().await;
    result
}

pub async fn delete_world_datapack(
    cluster_id: i64,
    world: String,
    file_name: String,
) -> Result<(), LauncherError> {
    let state = crate::launcher::state()?;
    let cluster = state.clusters.get(cluster_id).await?;
    let result =
        run_blocking(move || oneclient_core::delete_world_datapack(&cluster, &world, &file_name))
            .await;
    invalidate_worlds_queries().await;
    result
}
