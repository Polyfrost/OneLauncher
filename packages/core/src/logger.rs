use tracing_subscriber::{prelude::*, util::SubscriberInitExt};
use crate::store::Core;

// Handling for the live development logging
// This will log to the console, and will not log to a file
#[cfg(debug_assertions)]
pub async fn start_logger() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
		.unwrap_or_else(|_| {
			tracing_subscriber::EnvFilter::new(
				Core::get().logger_filter
					.as_ref()
					.unwrap_or(
						&format!("{}=info", env!("CARGO_PKG_NAME"))
					)
			)
		});

	let mut fmt_layer = tracing_subscriber::fmt::layer()
		.with_ansi(true)
		.with_file(true)
		.with_line_number(true)
		.with_level(true)
		.with_thread_names(true)
		.pretty();

	if let Some(span) = Core::get().logger_span_formatting.clone() {
		fmt_layer = fmt_layer.with_span_events(span);
	}

    if let Err(err) = tracing_subscriber::registry()
	    .with(fmt_layer)
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
	
    use crate::store::Dirs;
	use crate::store::Core;

    // Initialize and get logs directory path
    let logs_dir = Dirs::get_launcher_logs_dir().await.unwrap_or_default();

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
            return;
        }
    };

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
			tracing_subscriber::EnvFilter::new(
				Core::get().logger_filter
					.as_ref()
					.unwrap_or(
						&format!("{}=info", env!("CARGO_PKG_NAME"))
					)
			)
		});

	let mut fmt_layer = tracing_subscriber::fmt::layer()
		.compact()
		.with_writer(file)
		.with_ansi(false)
		.with_timer(ChronoLocal::rfc_3339());

	if let Some(span) = Core::get().logger_span_formatting.clone() {
		fmt_layer = fmt_layer.with_span_events(span);
	}

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter)
        .with(tracing_error::ErrorLayer::default())
		.init();
}
