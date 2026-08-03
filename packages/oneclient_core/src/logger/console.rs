//! The live launcher log console.
//!
//! A [`Layer`] that hands formatted lines to whoever is watching them, and
//! nothing at all when nobody is. The gate is the broadcast channel's own
//! receiver count rather than a flag we maintain: a receiver that goes away
//! because its window closed decrements it on drop, so there is no way to leave
//! capture switched on by accident.

use std::sync::{Arc, OnceLock};

use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

/// How many lines the channel holds for a console that is slow to drain.
///
/// A console that falls further behind than this loses the oldest lines and is
/// told so, which beats blocking the thread that emitted the log.
const CAPACITY: usize = 4096;

static CHANNEL: OnceLock<broadcast::Sender<ConsoleLine>> = OnceLock::new();

/// One captured event.
#[derive(Clone, Debug)]
pub struct ConsoleLine {
    pub level: Level,
    /// Pre-formatted as `[HH:MM:SS.mmm] [target/LEVEL]: message`, which is the
    /// shape the log viewer already parses for Minecraft logs, so launcher
    /// lines get the same colouring for free.
    pub text: Arc<str>,
}

/// A live console. Capture runs for exactly as long as this is alive.
pub struct ConsoleSubscription(broadcast::Receiver<ConsoleLine>);

impl ConsoleSubscription {
    pub async fn recv(&mut self) -> Result<ConsoleLine, broadcast::error::RecvError> {
        self.0.recv().await
    }
}

/// Start watching the launcher's logs.
///
/// Nothing is buffered before this call, so the console opens empty and fills
/// from the moment it is shown.
pub fn subscribe() -> ConsoleSubscription {
    ConsoleSubscription(channel().subscribe())
}

/// Whether anything is listening.
///
/// Two atomic loads and a branch is the entire cost of the console layer while
/// no console window is open: nothing is formatted, nothing is allocated, and
/// nothing is buffered. Before a console has ever been opened the channel does
/// not even exist, so it is one load.
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
        // Gating here rather than in `Layer::enabled`: that one is ANDed into
        // the whole subscriber's decision, so returning `false` from it would
        // silence stdout and the log file too whenever the console is shut.
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

        // A message with embedded newlines — a panic backtrace, say — is sent
        // line by line so the viewer can scroll and select it like any other.
        // Only the first carries the prefix; the rest read as continuations.
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

/// Pulls the `message` field out of an event and renders the rest as
/// `key=value`, matching what the `fmt` layer writes to stdout.
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
