use std::cell::Cell;

use freya::prelude::*;
use freya::router::*;

use crate::hooks::use_dispatch;
use crate::ipc::{self, IpcCommand};
use crate::platform;
use crate::routes::Route;
use crate::theme;

thread_local! {
    /// The open main window's router, `None` while it's closed to the background
    static ROUTER: Cell<Option<RouterContext>> = const { Cell::new(None) };
    /// Logs asked for while the window was closed, opened once it's back
    static PENDING_LOGS: Cell<Option<i64>> = const { Cell::new(None) };
}

pub fn use_ipc_commands() {
    let dispatch = use_dispatch();

    use_hook(move || {
        let router = RouterContext::get();
        ROUTER.set(Some(router));
        if let Some(cluster_id) = PENDING_LOGS.take() {
            let _ = router.push(Route::ProcessLogs { cluster_id });
        }

        // The inbox is taken once, the loop is global so the tray and second launches
        // still reach a launcher whose window was closed to the background
        let Some(mut inbox) = ipc::take_inbox() else {
            return;
        };

        spawn_global(async move {
            while let Some(command) = inbox.recv().await {
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
                    IpcCommand::Theme(id) => {
                        if theme::is_known(&id) {
                            dispatch.edit_settings(|settings| settings.theme = id);
                        }
                    }
                    IpcCommand::Logs => {
                        platform::focus_window();
                        let running = dispatch.station().peek().game.running_clusters().min();
                        if let Some(cluster_id) = running {
                            match ROUTER.get() {
                                Some(router) => {
                                    let _ = router.push(Route::ProcessLogs { cluster_id });
                                }
                                None => PENDING_LOGS.set(Some(cluster_id)),
                            }
                        }
                    }
                    IpcCommand::Stop => {
                        let running: Vec<i64> =
                            dispatch.station().peek().game.running_clusters().collect();
                        for cluster_id in running {
                            dispatch.kill_cluster(cluster_id);
                        }
                    }
                    IpcCommand::Close => platform::close(),
                    IpcCommand::Quit => platform::quit(),
                }
            }
        });
    });

    use_drop(|| ROUTER.set(None));
}
