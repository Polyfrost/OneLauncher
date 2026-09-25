use std::path::{Path, PathBuf};
use std::time::Duration;

use freya::prelude::{spawn, use_hook};
use notify::{Event, RecursiveMode, Watcher};
use tokio::sync::mpsc;

const WATCH_QUIET: Duration = Duration::from_millis(400);

pub(super) fn use_folder_watch(
    folder: Option<PathBuf>,
    mode: RecursiveMode,
    relevant: fn(&Path, &Event) -> bool,
    on_change: impl Fn() + 'static,
) {
    use_hook(move || {
        let Some(folder) = folder else {
            return;
        };

        spawn(async move {
            if let Err(err) = watch_folder(&folder, mode, relevant, on_change).await {
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
    mode: RecursiveMode,
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
    watcher.watch(folder, mode)?;

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
                on_change();
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
