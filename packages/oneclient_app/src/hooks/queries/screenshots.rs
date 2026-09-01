use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use freya::prelude::{spawn, use_hook};
use freya::query::{
    Mutation, MutationCapability, QueriesStorage, Query, QueryCapability,
    UseMutation, UseQuery, use_mutation, use_query,
};
use notify::{EventKind, RecursiveMode, Watcher};
use oneclient_core::{LauncherError, ScreenshotInfo};
use tokio::sync::{Semaphore, mpsc};

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
        Ok(oneclient_core::list_cluster_screenshots(&cluster)?)
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

        Ok(
            tokio::task::spawn_blocking(move || oneclient_core::load_screenshot(&path, max_edge))
                .await
                .map_err(|e| LauncherError::Minecraft(e.to_string()))??,
        )
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

const WATCH_QUIET: Duration = Duration::from_millis(400);

pub fn use_screenshot_folder_watch(
    folder: Option<PathBuf>,
    query: UseQuery<ClusterScreenshotsQuery>,
) {
    use_hook(move || {
        let Some(folder) = folder else {
            return;
        };

        spawn(async move {
            if let Err(err) = watch_folder(&folder, query).await {
                tracing::warn!(
                    folder = %folder.display(),
                    error = %err,
                    "not watching the screenshot folder; the list will refresh on re-entry only"
                );
            }
        });
    });
}

async fn watch_folder(
    folder: &Path,
    query: UseQuery<ClusterScreenshotsQuery>,
) -> notify::Result<()> {
    // If there is no screenshots folder it creates it
    std::fs::create_dir_all(folder)?;

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match event {
            Ok(event) if touches_image(&event) => {
                let _ = tx.send(());
            }
            Ok(_) => {}
            Err(err) => tracing::debug!(error = %err, "screenshot watcher reported an error"),
        }
    })?;
    watcher.watch(folder, RecursiveMode::NonRecursive)?;

    let mut pending = false;
    loop {
        tokio::select! {
            biased;

            event = rx.recv() => {
                if event.is_none() {
                    return Ok(());
                }
                pending = true;
            }

            () = quiet_period(pending) => {
                pending = false;
                query.invalidate();
            }
        }
    }
}

async fn quiet_period(pending: bool) {
    if pending {
        tokio::time::sleep(WATCH_QUIET).await;
    } else {
        std::future::pending::<()>().await;
    }
}

fn touches_image(event: &notify::Event) -> bool {
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
    QueriesStorage::<LocalImageQuery>::invalidate_all().await;
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
            ScreenshotAction::Delete { path } => Ok(oneclient_core::delete_screenshot(path)?),
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
