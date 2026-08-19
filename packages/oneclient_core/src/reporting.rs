use std::panic;
use std::sync::Arc;
use std::time::Duration;

use sentry::protocol::{Breadcrumb, Context, Event};
use sentry::{ClientInitGuard, ClientOptions};

use oneclient_common::constants::SENTRY_DSN;

const PANIC_FLUSH_TIMEOUT: Duration = Duration::from_secs(2);
const TRACING_FIELDS_CONTEXT: &str = "Rust Tracing Fields";

const ENVIRONMENT: &str = if cfg!(debug_assertions) {
    "development"
} else {
    "production"
};

/// `before_send` hook drops events whose log call set `sentry = false`
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

fn drop_opted_out_breadcrumbs(breadcrumb: Breadcrumb) -> Option<Breadcrumb> {
    let opted_out = breadcrumb
        .data
        .get("sentry")
        .and_then(serde_json::Value::as_bool)
        == Some(false);

    if opted_out { None } else { Some(breadcrumb) }
}

pub fn init(enabled: bool) -> Option<ClientInitGuard> {
    if oneclient_common::consent::declined() {
        tracing::debug!("crash reporting disabled: terms and privacy policy declined");
        return None;
    }

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
            before_breadcrumb: Some(Arc::new(drop_opted_out_breadcrumbs)),
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