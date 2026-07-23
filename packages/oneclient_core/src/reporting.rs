use std::panic;
use std::sync::Arc;
use std::time::Duration;

use sentry::protocol::{Context, Event};
use sentry::{ClientInitGuard, ClientOptions};

use crate::constants::SENTRY_DSN;

const PANIC_FLUSH_TIMEOUT: Duration = Duration::from_secs(2);
const TRACING_FIELDS_CONTEXT: &str = "Rust Tracing Fields";

const ENVIRONMENT: &str = if cfg!(debug_assertions) {
    "development"
} else {
    "production"
};

/// `before_send` hook: drop any event a log call explicitly opted out of by
/// setting the `sentry` field to `false`, e.g.
///
/// ```ignore
/// tracing::error!(sentry = false, "handled MSA failure, don't report as a crash");
/// ```
fn drop_opted_out_events(event: Event<'static>) -> Option<Event<'static>> {
    let opted_out = matches!(
        event.contexts.get(TRACING_FIELDS_CONTEXT),
        Some(Context::Other(fields))
            if fields.get("sentry").and_then(serde_json::Value::as_bool) == Some(false)
    );

    if opted_out { None } else { Some(event) }
}

pub fn init(enabled: bool) -> Option<ClientInitGuard> {
    if !enabled {
        tracing::debug!("crash reporting disabled by settings");
        return None;
    }

    if cfg!(debug_assertions) && option_env!("ONECLIENT_SENTRY_DSN").is_none() {
        tracing::debug!("crash reporting skipped: debug build without an explicit DSN");
        return None;
    }

    let guard = sentry::init((
        SENTRY_DSN,
        ClientOptions {
            release: Some(format!("oneclient@{}", env!("CARGO_PKG_VERSION")).into()),
            environment: Some(ENVIRONMENT.into()),
            attach_stacktrace: true,
            send_default_pii: false,
            before_send: Some(Arc::new(drop_opted_out_events)),
            ..Default::default()
        },
    ));

    if !guard.is_enabled() {
        tracing::warn!("sentry client failed to start; crash reports will not be sent");
        return None;
    }

    install_panic_flush();

    tracing::info!(environment = ENVIRONMENT, "crash reporting enabled");
    Some(guard)
}

fn install_panic_flush() {
    let capture = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        capture(info);

        if let Some(client) = sentry::Hub::current().client() {
            client.flush(Some(PANIC_FLUSH_TIMEOUT));
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_dsn_parses() {
        SENTRY_DSN
            .parse::<sentry::types::Dsn>()
            .expect("bundled sentry DSN should be valid");
    }

    #[test]
    fn opting_out_skips_the_client() {
        assert!(init(false).is_none());
    }

    /// Build an event the way `sentry-tracing` would, given the value of a
    /// `sentry` field on the log call (or `None` for no such field).
    fn event_with_sentry_field(value: Option<bool>) -> Event<'static> {
        let mut event = Event::default();
        if let Some(value) = value {
            let mut fields = std::collections::BTreeMap::new();
            fields.insert("sentry".to_owned(), serde_json::Value::Bool(value));
            event
                .contexts
                .insert(TRACING_FIELDS_CONTEXT.to_owned(), Context::Other(fields));
        }
        event
    }

    #[test]
    fn drops_events_flagged_sentry_false() {
        assert!(drop_opted_out_events(event_with_sentry_field(Some(false))).is_none());
    }

    #[test]
    fn keeps_events_without_opt_out() {
        assert!(drop_opted_out_events(event_with_sentry_field(Some(true))).is_some());
        assert!(drop_opted_out_events(event_with_sentry_field(None)).is_some());
        assert!(drop_opted_out_events(Event::default()).is_some());
    }
}
