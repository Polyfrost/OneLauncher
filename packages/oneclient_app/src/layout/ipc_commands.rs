use freya::prelude::*;
use freya::router::*;

use crate::hooks::use_dispatch;
use crate::ipc::{self, IpcCommand};
use crate::platform;
use crate::routes::Route;

pub fn use_ipc_commands() {
    let dispatch = use_dispatch();

    use_hook(move || {
        let Some(mut inbox) = ipc::take_inbox() else {
            return;
        };
        let router = RouterContext::get();

        spawn_forever(async move {
            while let Some(command) = inbox.recv().await {
                match command {
                    IpcCommand::Focus => platform::focus_window(),
                    IpcCommand::Launch(folder) => {
                        platform::focus_window();
                        dispatch.request_launch_by_folder(folder);
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
                    IpcCommand::Close => platform::close(),
                    IpcCommand::Quit => platform::quit(),
                }
            }
        });
    });
}
