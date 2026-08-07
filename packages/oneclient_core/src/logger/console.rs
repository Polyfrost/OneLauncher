//! Capture is gated on the broadcast channel's receiver count rather than a flag
//! we maintain so a dropped receiver can never leave capture switched on

use std::sync::{Arc, OnceLock};

use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

/// A console falling further behind than this loses its oldest lines which
/// beats blocking the thread that emitted the log
const CAPACITY: usize = 4096;

static CHANNEL: OnceLock<broadcast::Sender<ConsoleLine>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct ConsoleLine {
    pub level: Level,
    /// Pre-formatted as `[HH:MM:SS.mmm] [target/LEVEL]: message` the shape the
    /// log viewer already parses for Minecraft logs
    pub text: Arc<str>,
}

/// Capture runs for exactly as long as this is alive
pub struct ConsoleSubscription(broadcast::Receiver<ConsoleLine>);

impl ConsoleSubscription {
    pub async fn recv(&mut self) -> Result<ConsoleLine, broadcast::error::RecvError> {
        self.0.recv().await
    }
}

/// Nothing is buffered before this call so the console opens empty
pub fn subscribe() -> ConsoleSubscription {
    ConsoleSubscription(channel().subscribe())
}

pub fn is_capturing() -> bool {
    CHANNEL.get().is_some_and(|tx| tx.receiver_count() > 0)
}

pub(super) fn layer<S: Subscriber>() -> impl Layer<S> {
    ConsoleLayer
}

fn channel() -> &'static broadcast::Sender<ConsoleLine> {
    CHANNEL.get_or_init(|| broadcast::channel(CAPACITY).0)
}

struct ConsoleLayer;

impl<S: Subscriber> Layer<S> for ConsoleLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Not `Layer::enabled` that is ANDed into the whole subscriber's
        // decision so returning false there would silence stdout and the file too
        if !is_capturing() {
            return;
        }

        let meta = event.metadata();
        let mut visitor = LineVisitor::default();
        event.record(&mut visitor);

        let prefix = format!(
            "[{}] [{}/{}]: ",
            chrono::Local::now().format("%H:%M:%S%.3f"),
            meta.target(),
            meta.level().as_str(),
        );

        let level = *meta.level();
        let tx = channel();

        // Embedded newlines (a panic backtrace say) are sent line by line so the
        // viewer can select them only the first carries the prefix
        let mut body = visitor.message;
        body.push_str(&visitor.fields);

        let mut rest = body.lines();
        let first = rest.next().unwrap_or_default();
        let _ = tx.send(ConsoleLine {
            level,
            text: format!("{prefix}{first}").into(),
        });

        for line in rest {
            let _ = tx.send(ConsoleLine {
                level,
                text: line.into(),
            });
        }
    }
}

/// Renders non-message fields as `key=value` matching what `fmt` writes to stdout
#[derive(Default)]
struct LineVisitor {
    message: String,
    fields: String,
}

impl Visit for LineVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.fields.push_str(&format!(" {}={value}", field.name()));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message.push_str(&format!("{value:?}"));
        } else {
            self.fields
                .push_str(&format!(" {}={value:?}", field.name()));
        }
    }
}
