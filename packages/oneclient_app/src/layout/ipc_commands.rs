use freya::prelude::*;
use freya::router::*;

use crate::hooks::use_dispatch;
use crate::platform;
use crate::routes::Route;
use oneclient_cli::ipc::{self, Incoming, IpcCommand};

pub fn use_ipc_commands() {
    let dispatch = use_dispatch();

    use_hook(move || {
        let Some(mut inbox) = ipc::take_inbox() else {
            return;
        };
        let router = RouterContext::get();

        spawn_in_window(async move {
            while let Some(incoming) = inbox.recv().await {
                let command = match incoming {
                    Incoming::Window(command) => command,
                    Incoming::Run(command, out) => {
                        tokio::spawn(async move {
                            match crate::launcher::state() {
                                Ok(state) => {
                                    oneclient_cli::run(
                                        oneclient_cli::Host::Gui,
                                        state,
                                        command,
                                        out,
                                    )
                                    .await;
                                }
                                Err(_) => {
                                    out.fail("OneClient is still starting, try again in a moment")
                                }
                            }
                        });
                        continue;
                    }
                };
                match command {
                    IpcCommand::Focus => platform::focus_window(),
                    IpcCommand::Launch(folder) => {
                        platform::focus_window();
                        dispatch.request_launch_by_folder(folder);
                    }
                    IpcCommand::ImportShare(code) => {
                        platform::focus_window();
                        dispatch.request_share_import(code);
                    }
                    IpcCommand::Logs => {
                        platform::focus_window();
                        let running = dispatch.station().peek().game.running_clusters().min();
                        if let Some(cluster_id) = running {
                            let _ = router.push(Route::ProcessLogs { cluster_id });
                        }
                    }
                    IpcCommand::Stop => {
                        let running: Vec<i64> =
                            dispatch.station().peek().game.running_clusters().collect();
                        for cluster_id in running {
                            dispatch.kill_cluster(cluster_id);
                        }
                    }
                    IpcCommand::Refresh => {
                        dispatch.reload_settings();
                        spawn_in_window(crate::hooks::invalidate_after_cli());
                        let cloud = dispatch.clone();
                        spawn_in_window(async move { cloud.refresh_cloud_sync().await });
                    }
                    IpcCommand::CheckUpdate => {
                        if let Ok(state) = crate::launcher::state() {
                            crate::updater::spawn_update_check(
                                false,
                                state.services.events.clone(),
                            );
                        }
                    }
                    IpcCommand::Close => platform::close(),
                    IpcCommand::Quit => platform::quit(),
                }
            }
        });
    });
}
