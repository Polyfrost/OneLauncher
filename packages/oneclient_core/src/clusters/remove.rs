use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use oneclient_cluster::Cluster;
use oneclient_common::paths;
use oneclient_events::{EventBus, Signal};
use uuid::Uuid;

use crate::LauncherResult;
use crate::state::LauncherState;

const PROGRESS_INTERVAL: Duration = Duration::from_millis(80);

#[tracing::instrument(skip(state))]
pub async fn delete_cluster(state: &LauncherState, cluster_id: i64) -> LauncherResult<Cluster> {
    let cluster = state.clusters.get(cluster_id).await?;
    let dir = cluster.dir()?;

    state.clusters.delete(cluster_id, false).await?;

    let events = &state.services.events;
    events.signal(Signal::ClustersChanged);

    let dir = detach_dir(&dir, cluster_id).await;

    let progress_id = Uuid::new_v4();
    let label = format!("Deleting {}", cluster.name);
    let removed = purge_dir(events, progress_id, &label, &dir).await?;

    if removed == 0 {
        events
            .notify("Instance deleted")
            .body(format!("{} is gone.", cluster.name))
            .send();
    } else {
        events.finish_progress(
            progress_id,
            "Instance deleted",
            format!("Removed {} and its {removed} files.", cluster.name),
        );
    }

    tracing::info!(cluster_id, removed, "deleted cluster");

    Ok(cluster)
}

async fn detach_dir(dir: &Path, cluster_id: i64) -> PathBuf {
    let Some(folder_name) = dir.file_name().map(|name| name.to_string_lossy().into_owned()) else {
        return dir.to_path_buf();
    };

    let detached = dir.with_file_name(format!(
        "{folder_name}.{cluster_id}{}",
        paths::DELETING_SUFFIX
    ));

    match polyio::rename(dir, &detached).await {
        Ok(()) => detached,
        Err(err) => {
            tracing::warn!(
                folder = %folder_name,
                error = %err,
                "could not detach the cluster folder before deleting; removing it in place"
            );
            dir.to_path_buf()
        }
    }
}

async fn purge_dir(
    events: &EventBus,
    progress_id: Uuid,
    label: &str,
    dir: &Path,
) -> LauncherResult<u64> {
    if !polyio::try_exists(dir).await.unwrap_or(false) {
        return Ok(0);
    }

    let files = collect_files(dir).await;
    let total = files.len() as u64;

    if total == 0 {
        polyio::remove_dir_all(dir).await?;
        return Ok(0);
    }

    events.progress(progress_id, label, 0, total);
    let mut last_emit = Instant::now();

    for (index, file) in files.iter().enumerate() {
        if polyio::remove_file(file).await.is_err() {
            let _ = polyio::remove_symlink_dir(file).await;
        }

        let done = index as u64 + 1;
        let now = Instant::now();
        if done == total || now.duration_since(last_emit) >= PROGRESS_INTERVAL {
            events.progress(progress_id, label, done, total);
            last_emit = now;
        }
    }

    if let Err(err) = polyio::remove_dir_all(dir).await {
        events.finish_progress(
            progress_id,
            "Deletion incomplete",
            "Some files could not be removed.",
        );
        return Err(err.into());
    }

    Ok(total)
}

async fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(mut entries) = polyio::read_dir(&dir).await else {
            continue;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };

            if file_type.is_dir() {
                stack.push(entry.path());
            } else {
                files.push(entry.path());
            }
        }
    }

    files
}
