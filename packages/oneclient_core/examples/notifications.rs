
use oneclient_core::notification::{GroupedProgressSession, NotificationService};
use tokio::sync::mpsc;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let notifier = NotificationService::new(tx);

    notifier.send_info("Welcome", "Launcher started");
    notifier.send_error("Heads up", "Example error notification");

    let download = Uuid::new_v4();
    for current in (0..=100).step_by(25) {
        notifier.send_progress(&download, "Downloading assets", current, 100);
    }

    let session = GroupedProgressSession::start(&notifier, "Preparing cluster".to_string());
    session.finish();

    drop(notifier);

    while let Some(notification) = rx.recv().await {
        println!("{notification:?}");
    }
}
