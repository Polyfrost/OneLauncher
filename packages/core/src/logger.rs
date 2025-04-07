// Handling for the live development logging
// This will log to the console, and will not log to a file
#[cfg(debug_assertions)]
pub async fn start_logger() {
    use tracing_subscriber::{fmt::format::FmtSpan, prelude::*, util::SubscriberInitExt};

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
		.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(format!("{}=info", env!("CARGO_PKG_NAME"))));

    if let Err(err) = tracing_subscriber::registry()
	    .with(
			tracing_subscriber::fmt::layer()
				.with_ansi(true)
				.with_file(true)
				.with_line_number(true)
				.with_level(true)
				.with_thread_names(true)
				.with_span_events(FmtSpan::FULL)
				.pretty()
		)
        .with(filter)
        .with(tracing_error::ErrorLayer::default())
		.try_init() {
			eprintln!("Could not set default logger: {err}");
		}
}

// Handling for the live production logging
// This will log to a file in the logs directory, and will not show any logs in the console
#[cfg(not(debug_assertions))]
pub async fn start_logger() {
    use chrono::Local;
    use std::fs::OpenOptions;
    use tracing_subscriber::fmt::time::ChronoLocal;
    use tracing_subscriber::prelude::*;

    use crate::io::Dirs;

    // Initialize and get logs directory path
    let logs_dir = Dirs::get().await?.launcher_logs_dir();

    let log_file_name = format!("launcher_{}.log", Local::now().format("%Y%m%d_%H%M%S"));
    let log_file_path = logs_dir.join(log_file_name);

    if let Err(err) = std::fs::create_dir_all(&logs_dir) {
        eprintln!("Could not create logs directory: {err}");
    }

    let file = match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
    {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Could not start open log file: {e}");
            return None;
        }
    };

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(format!("{}=info", env!("CARGO_PKG_NAME"))));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
				.compact()
                .with_writer(file)
                .with_ansi(false)
                .with_timer(ChronoLocal::rfc_3339()),
        )
        .with(filter)
        .with(tracing_error::ErrorLayer::default())
		.init();
}
