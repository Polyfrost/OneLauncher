use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use bytes::Bytes;
use freya::query::{
    Mutation, MutationCapability, QueriesStorage, Query, QueryCapability, UseMutation, UseQuery,
    use_mutation, use_query,
};
use notify::{Event, EventKind};
use oneclient_core::{LauncherError, ScreenshotInfo};
use tokio::sync::Semaphore;

use crate::launcher::off_ui_blocking;

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
        let state = crate::launcher::state()?;
        let cluster = state.clusters.get(keys.cluster_id).await?;
        Ok(off_ui_blocking(move || oneclient_core::list_cluster_screenshots(&cluster)).await?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalImageKeys {
    pub path: PathBuf,
    pub max_edge: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalImageQuery {
    picked: bool,
}

impl QueryCapability for LocalImageQuery {
    type Ok = Bytes;
    type Err = LauncherError;
    type Keys = LocalImageKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        if keys.path.as_os_str().is_empty() {
            return Ok(Bytes::new());
        }

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

        let picked = self.picked;
        Ok(tokio::task::spawn_blocking(move || {
            if picked {
                oneclient_core::load_picked_image(&path, max_edge)
            } else {
                oneclient_core::load_screenshot(&path, max_edge)
            }
        })
        .await
        .map_err(|e| LauncherError::Minecraft(e.to_string()))??)
    }

    fn matches(&self, _keys: &Self::Keys) -> bool {
        !self.picked
    }
}

pub fn use_cluster_screenshots(cluster_id: i64) -> UseQuery<ClusterScreenshotsQuery> {
    use_query(Query::new(
        ClusterScreenshotsKeys { cluster_id },
        ClusterScreenshotsQuery,
    ))
}

pub fn use_local_image(path: PathBuf, max_edge: u32, picked: bool) -> UseQuery<LocalImageQuery> {
    use_query(Query::new(
        LocalImageKeys { path, max_edge },
        LocalImageQuery { picked },
    ))
}

pub fn use_screenshot_folder_watch(
    folder: Option<PathBuf>,
    query: UseQuery<ClusterScreenshotsQuery>,
) {
    super::folder_watch::use_folder_watch(folder, false, touches_image, move || query.invalidate());
}

fn touches_image(_root: &Path, event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }

    event.paths.iter().any(|path| {
        path.extension().and_then(OsStr::to_str).is_some_and(|ext| {
            ["png", "jpg", "jpeg"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
    })
}

pub fn try_cluster_screenshots(
    query: &UseQuery<ClusterScreenshotsQuery>,
) -> Option<Vec<ScreenshotInfo>> {
    super::state::settled_or_loading(query)
}

pub async fn invalidate_screenshots_queries() {
    QueriesStorage::<ClusterScreenshotsQuery>::invalidate_all().await;
    QueriesStorage::<LocalImageQuery>::invalidate_matching(LocalImageKeys {
        path: PathBuf::new(),
        max_edge: 0,
    })
    .await;
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
            ScreenshotAction::Delete { path } => {
                let path = path.clone();
                Ok(off_ui_blocking(move || oneclient_core::delete_screenshot(&path)).await?)
            }
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
