use freya::prelude::spawn_forever;
use oneclient_db::models::ClusterId;

use super::actions::Actions;
use crate::components::IconType;
use crate::launcher;
use crate::shortcut::{self, ShortcutRequest};
use crate::state::{AppChannel, LaunchBlock};

impl Actions {
    pub fn request_launch_by_folder(&self, folder: String) {
        self.station()
            .write_channel(AppChannel::PendingLaunch)
            .pending_launch = Some(folder);
    }

    pub fn take_pending_launch(&self) -> Option<String> {
        let waiting = self.station().peek().pending_launch.is_some();
        if !waiting {
            return None;
        }

        self.station()
            .write_channel(AppChannel::PendingLaunch)
            .pending_launch
            .take()
    }

    #[must_use]
    pub fn launch_block(&self, cluster_id: ClusterId) -> Option<LaunchBlock> {
        let station = self.station();
        let snapshot = station.peek();
        let parallel = snapshot.settings.settings.allow_parallel_running_clusters;
        snapshot.game.launch_block(cluster_id, parallel)
    }

    pub fn report_missing_shortcut_target(&self, folder: &str) {
        self.notify("Shortcut is out of date")
            .body(format!(
                "No version folder named \"{folder}\" is installed any more. The shortcut can be recreated from the version's page."
            ))
            .error()
            .icon(IconType::LinkExternal01)
            .send();
    }

    pub fn create_cluster_shortcut(&self, cluster_id: ClusterId) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };

            let cluster = match state.clusters.get(cluster_id).await {
                Ok(cluster) => cluster,
                Err(err) => {
                    actions.report_shortcut_failure(&format!("{err:#}"));
                    return;
                }
            };

            let Some(dir) = pick_shortcut_dir().await else {
                return;
            };

            let request = ShortcutRequest {
                cluster_name: cluster.name.clone(),
                folder_name: cluster.folder_name.clone(),
                dir: dir.clone(),
            };

            match tokio::task::spawn_blocking(move || shortcut::create(&request)).await {
                Ok(Ok(path)) => {
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| cluster.name.clone());

                    actions
                        .notify("Shortcut created")
                        .body(format!("Saved as {name} in {}", dir.display()))
                        .icon(IconType::LinkExternal01)
                        .send();
                }
                Ok(Err(err)) => actions.report_shortcut_failure(&format!("{err:#}")),
                Err(err) => actions.report_shortcut_failure(&err.to_string()),
            }
        });
    }

    fn report_shortcut_failure(&self, reason: &str) {
        tracing::error!("could not create a cluster shortcut: {reason}");
        self.notify("Couldn't create the shortcut")
            .body(reason.to_string())
            .error()
            .send();
    }
}

async fn pick_shortcut_dir() -> Option<std::path::PathBuf> {
    let mut dialog = rfd::AsyncFileDialog::new().set_title("Where should the shortcut go?");
    if let Some(desktop) = shortcut::default_dir() {
        dialog = dialog.set_directory(desktop);
    }

    dialog
        .pick_folder()
        .await
        .map(|handle| handle.path().to_path_buf())
}
