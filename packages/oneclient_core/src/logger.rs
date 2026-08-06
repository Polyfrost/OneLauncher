use std::io;
use std::sync::OnceLock;

use parking_lot::Mutex;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, reload};

use crate::LauncherResult;

pub mod console;

/// The launcher's own crates. These are the ones anyone reading a log actually
/// wants to hear from, so they get an explicit directive instead of falling
/// back to the base level.
const APP_TARGETS: &[&str] = &[
    "oneclient_app",
    "oneclient_auth",
    "oneclient_cluster",
    "oneclient_common",
    "oneclient_content",
    "oneclient_core",
    "oneclient_db",
    "oneclient_discord",
    "oneclient_events",
    "oneclient_java",
    "oneclient_mc",
    "oneclient_net",
    "oneclient_polyplus",
    "polyio",
];

/// Dependencies that talk on every request, query, redraw or DNS lookup.
///
/// The release base level already covers them, but the debug filter raises the
/// base to `info` so that an unfamiliar crate misbehaving still shows up — and
/// that would let this lot back in. A log drowned in `hyper` connection
/// bookkeeping helps nobody, so they are pinned regardless of the base.
const NOISY_TARGETS: &[&str] = &[
    "calloop",
    "freya_core",
    "freya_winit",
    "h2",
    "hickory_proto",
    "hickory_resolver",
    "html5ever",
    "hyper",
    "hyper_util",
    "mio",
    "quinn",
    "reqwest",
    "rustls",
    "smithay_client_toolkit",
    // Only `sqlx::query`: `sqlx` proper reports pool and migration problems we
    // do want, it is the statement-per-line log that is unreadable.
    "sqlx::query",
    "tokio_util",
    "tower",
    "want",
    "winit",
    "zbus",
];

/// Directives for a normal run: our crates at `info`, everything else silent
/// unless it is a warning.
pub fn default_directives() -> String {
    directives("info", "warn")
}

/// Directives for "Log Debug Info": our crates at `debug`, and a raised base so
/// an unlisted dependency's `info` still reaches the log.
pub fn debug_directives() -> String {
    directives("debug", "info")
}

fn directives(app_level: &str, base_level: &str) -> String {
    let mut out = String::from(base_level);
    for target in NOISY_TARGETS {
        out.push_str(&format!(",{target}=warn"));
    }
    for target in APP_TARGETS {
        out.push_str(&format!(",{target}={app_level}"));
    }
    out
}

/// The filter layer sits directly on the [`Registry`], so that is the subscriber
/// type the reload handle is parameterised over.
type FilterHandle = reload::Handle<EnvFilter, Registry>;

static FILTER: OnceLock<FilterHandle> = OnceLock::new();
static ACTIVE_DIRECTIVES: Mutex<String> = Mutex::new(String::new());

#[derive(Debug, thiserror::Error)]
pub enum FilterError {
    #[error("invalid log filter directives: {0}")]
    Invalid(String),

    #[error("the logger has not been initialized")]
    NotInitialized,
}

/// The directives currently in force, for anything that wants to show them.
pub fn active_directives() -> String {
    ACTIVE_DIRECTIVES.lock().clone()
}

/// Swap the active filter without restarting the launcher.
///
/// This is what makes the live console worth having: a developer can widen the
/// filter, watch the noise, and narrow it again in the same session. A string
/// that fails to parse leaves the previous filter in place rather than blanking
/// the log.
pub fn set_filter(directives: &str) -> Result<(), FilterError> {
    let filter =
        EnvFilter::try_new(directives).map_err(|err| FilterError::Invalid(err.to_string()))?;

    FILTER
        .get()
        .ok_or(FilterError::NotInitialized)?
        .reload(filter)
        .map_err(|err| FilterError::Invalid(err.to_string()))?;

    *ACTIVE_DIRECTIVES.lock() = directives.to_string();
    tracing::info!(%directives, "log filter reloaded");

    Ok(())
}

pub fn init_debug() -> LauncherResult<()> {
    init_filtered(debug_directives)
}

pub fn init() -> LauncherResult<()> {
    init_filtered(default_directives)
}

pub fn init_filtered(filter: impl FnOnce() -> String) -> LauncherResult<()> {
    // `RUST_LOG` wins outright rather than being merged in: someone who set it
    // asked for exactly those directives, and appending ours would quietly
    // override the targets they care about.
    let directives = std::env::var(EnvFilter::DEFAULT_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(filter);

    let env_filter = EnvFilter::try_new(&directives).unwrap_or_else(|_| {
        eprintln!(
            "ignoring unparseable {} filter: {directives}",
            EnvFilter::DEFAULT_ENV
        );
        EnvFilter::new(default_directives())
    });

    let (filter, handle) = reload::Layer::new(env_filter);
    let _ = FILTER.set(handle);
    *ACTIVE_DIRECTIVES.lock() = directives;

    let stdout_layer = tracing_subscriber::fmt::layer().with_writer(io::stdout);

    let sentry_layer = sentry_tracing::layer();

    #[cfg(debug_assertions)]
    {
        tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .with(console::layer())
            .with(sentry_layer)
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        let logs_dir = oneclient_common::paths::logs_dir()?;
        std::fs::create_dir_all(&logs_dir)?;

        let log_path = logs_dir.join(format!("{}.log", chrono::Local::now().to_rfc3339().replace(':', "-")));

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(file);

        tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .with(file_layer)
            .with(console::layer())
            .with(sentry_layer)
            .init();

        tracing::info!(path = %log_path.display(), "writing logs to file");
    }

    Ok(())
}
