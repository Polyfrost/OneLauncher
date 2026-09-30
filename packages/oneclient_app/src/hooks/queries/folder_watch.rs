use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use freya::prelude::{spawn, use_hook};
use notify::{Event, RecursiveMode, Watcher};
use tokio::sync::mpsc;

const WATCH_QUIET: Duration = Duration::from_millis(400);

pub(crate) fn use_folder_watch(
    folder: Option<PathBuf>,
    watch_children: bool,
    relevant: fn(&Path, &Event) -> bool,
    on_change: impl Fn() + 'static,
) {
    use_hook(move || {
        let Some(folder) = folder else {
            return;
        };

        spawn(async move {
            if let Err(err) = watch_folder(&folder, watch_children, relevant, on_change).await {
                tracing::warn!(
                    folder = %folder.display(),
                    error = %err,
                    "not watching the folder; the list will refresh on re-entry only"
                );
            }
        });
    });
}

async fn watch_folder(
    folder: &Path,
    watch_children: bool,
    relevant: fn(&Path, &Event) -> bool,
    on_change: impl Fn(),
) -> notify::Result<()> {
    std::fs::create_dir_all(folder)?;

    let root = folder.to_path_buf();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut watcher =
        notify::recommended_watcher(move |event: notify::Result<Event>| match event {
            Ok(event) if relevant(&root, &event) => {
                let _ = tx.send(());
            }
            Ok(_) => {}
            Err(err) => tracing::debug!(error = %err, "folder watcher reported an error"),
        })?;
    watcher.watch(folder, RecursiveMode::NonRecursive)?;

    let mut children = HashSet::new();
    if watch_children {
        sync_children(&mut watcher, folder, &mut children);
    }

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
                if watch_children {
                    sync_children(&mut watcher, folder, &mut children);
                }
                on_change();
            }
        }
    }
}

fn sync_children(watcher: &mut impl Watcher, folder: &Path, watched: &mut HashSet<PathBuf>) {
    let current: HashSet<PathBuf> = std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default();

    let mut paths = watcher.paths_mut();
    for gone in watched.difference(&current) {
        let _ = paths.remove(gone);
    }
    for added in current.difference(watched) {
        if let Err(err) = paths.add(added, RecursiveMode::NonRecursive) {
            tracing::debug!(path = %added.display(), error = %err, "could not watch subfolder");
        }
    }
    if let Err(err) = paths.commit() {
        tracing::debug!(error = %err, "could not update subfolder watches");
    }

    *watched = current;
}

async fn quiet_period(pending: bool) {
    if pending {
        tokio::time::sleep(WATCH_QUIET).await;
    } else {
        std::future::pending::<()>().await;
    }
}
