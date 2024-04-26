//! **Watcher Utilities*
//! 
//! Async utilities for watching files with [`notify`].

use std::{path::PathBuf, time::Duration};

use notify::RecommendedWatcher;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use futures::{channel::mpsc::channel, SinkExt, StreamExt};
use crate::store::{Cluster, ClusterPath, Clusters};

pub const CLUSTERS_PATH: &str = "clusters";
pub const CRASH_PATH: &str = "crash-reports";

pub async fn initialize_watcher() -> crate::Result<Debouncer<RecommendedWatcher>> {
    let (mut sender, mut rscv) = channel(1);
    let watcher = new_debouncer(
        Duration::from_secs_f32(2.0),
        move |result: DebounceEventResult| {
            futures::executor::block_on(async {
                sender.send(result).await.unwrap();
            })
        })?;

    tokio::task::spawn(async move {
        let span = tracing::span!(tracing::Level::INFO, "initialize_watcher");
        tracing::info!(parent: &span, "initializing fs watcher...");
        while let Some(result) = rscv.next().await {
            let _span = span.enter();

            match result {
                Ok(mut events) => {
                    let mut paths = Vec::new();
                    events.sort_by(|a, b| a.path.cmp(&b.path));
                    events.iter().for_each(|a| {
                        let mut formatted = PathBuf::new();
                        let mut components = a.path.components();
                        let mut matched = false;

                        for cmp in components.by_ref() {
                            formatted.push(cmp);
                            if matched { break; }
                            // really simple fs watcher to check in any cluster files have been modified by outside forces
                            if cmp.as_os_str() == "clusters" { matched = true; }
                        }

                        let sub = components.next().is_none();
                        let cluster_path = ClusterPath::new(PathBuf::from(
                            formatted.file_name().unwrap_or_default(),
                        ));

                        if a.path
                            .components()
                            .any(|c| c.as_os_str() == CRASH_PATH)
                            && a.path.extension().map(|e| e == "txt").unwrap_or(false)
                        {
                            Cluster::handle_crash(cluster_path);
                        } else if !paths.contains(&formatted) {
                            if sub {
                                Cluster::sync_packages(cluster_path, false);
                                paths.push(formatted);
                            } else {
                                Clusters::sync_clusters(cluster_path);
                            }
                        }
                    });
                }
                Err(err) => tracing::warn!("fs watching error: {err}")
            }
        }
    });

    Ok(watcher)
}
