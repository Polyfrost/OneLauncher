
use oneclient_events::{GroupedProgressSession, EventBus};
use tokio::sync::mpsc;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let events = EventBus::new(tx);

    events.notify("Welcome").body("Launcher started").send();
    events.notify("Heads up").body("Example error notification").error().send();

    let download = Uuid::new_v4();
    for current in (0..=100).step_by(25) {
        events.progress(download, "Downloading assets", current, 100);
    }

    let session = GroupedProgressSession::start(&events, "Preparing cluster".to_string());
    session.finish();

    drop(events);

    while let Some(notification) = rx.recv().await {
        println!("{notification:?}");
    }
}
