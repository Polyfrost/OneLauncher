use std::path::PathBuf;

use freya::prelude::spawn_forever;

use crate::hooks::Actions;

pub fn database_path() -> Option<PathBuf> {
    oneclient_common::paths::database_file().ok()
}

pub fn snapshots() -> Vec<PathBuf> {
    database_path()
        .map(|path| oneclient_db::backup::list(&path))
        .unwrap_or_default()
}

pub fn open_data_folder() {
    let Ok(dir) = oneclient_common::paths::launcher_dir() else {
        return;
    };

    crate::platform::open_path(&dir.to_string_lossy());
}

pub fn restore_latest(actions: &Actions) {
    let Some(path) = database_path() else { return };
    let Some(snapshot) = snapshots().into_iter().next() else {
        set_error(actions, "There are no snapshots to restore.".to_owned());
        return;
    };

    match oneclient_db::backup::restore(&path, &snapshot) {
        Ok(_) => retry(actions),
        Err(err) => set_error(actions, format!("Couldn't restore the snapshot: {err}")),
    }
}

pub fn reset(actions: &Actions) {
    let Some(path) = database_path() else { return };

    match oneclient_db::backup::reset(&path) {
        Ok(_) => retry(actions),
        Err(err) => set_error(actions, format!("Couldn't reset the database: {err}")),
    }
}

fn retry(actions: &Actions) {
    let station = actions.station();
    let events = actions.events();

    {
        let mut guard = station.clone().write_channel(crate::state::AppChannel::Launcher);
        guard.launcher.error = None;
        guard.launcher.snapshots = snapshots().len();
    }

    spawn_forever(async move {
        if let Err(err) = crate::events::start_launcher(station, events).await {
            crate::events::report_startup_failure(&station, &err);
        }
    });
}

fn set_error(actions: &Actions, message: String) {
    let mut guard = actions
        .station()
        .write_channel(crate::state::AppChannel::Launcher);
    guard.launcher.error = Some(message);
    guard.launcher.snapshots = snapshots().len();
}
