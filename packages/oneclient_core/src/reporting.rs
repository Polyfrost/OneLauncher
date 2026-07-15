use std::panic;
use std::time::Duration;

use sentry::{ClientInitGuard, ClientOptions};

use crate::constants::SENTRY_DSN;

const PANIC_FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

const ENVIRONMENT: &str = if cfg!(debug_assertions) {
	"development"
} else {
	"production"
};

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
}
