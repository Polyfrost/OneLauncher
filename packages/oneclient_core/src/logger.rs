use std::io;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::LauncherResult;

const DEFAULT_DEBUG_FILTER: &str = "oneclient_app=debug,oneclient_core=debug,oneclient_db=debug,polyio=debug,reqwest=debug,sqlx=debug";
const DEFAULT_FILTER: &str = "oneclient_app=info,oneclient_core=info,oneclient_db=info,polyio=info";

pub fn init_debug() -> LauncherResult<()> {
    init_filtered(|| EnvFilter::new(DEFAULT_DEBUG_FILTER))
}

pub fn init() -> LauncherResult<()> {
    init_filtered(|| EnvFilter::new(DEFAULT_FILTER))
}

pub fn init_filtered(filter: impl FnOnce() -> EnvFilter) -> LauncherResult<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| filter());
    let stdout_layer = tracing_subscriber::fmt::layer().with_writer(io::stdout);

    #[cfg(debug_assertions)]
    {
        tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        let logs_dir = crate::paths::logs_dir()?;
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
            .init();

        tracing::info!(path = %log_path.display(), "writing logs to file");
    }

    Ok(())
}
