#[cfg(debug_assertions)]
use crate::store::Core;
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::SubscriberInitExt;

// Handling for the live development logging
// This will log to both the console and a file in the logs directory
#[cfg(debug_assertions)]
pub async fn start_logger() {
	use chrono::Local;
	use std::fs::OpenOptions;
	use tracing_subscriber::fmt::time::ChronoLocal;

	use crate::store::Dirs;

	let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
		tracing_subscriber::EnvFilter::new(
			Core::get()
				.logger_filter
				.clone()
				.unwrap_or_else(|| format!("{}=debug", env!("CARGO_PKG_NAME"))),
		)
	});

	let mut console_fmt_layer = tracing_subscriber::fmt::layer()
		.with_ansi(true)
		.with_file(true)
		.with_line_number(true)
		.with_level(true)
		.with_thread_names(true)
		.pretty();

	if let Some(span) = Core::get().logger_span_formatting.clone() {
		console_fmt_layer = console_fmt_layer.with_span_events(span);
	}

	let logs_dir = Dirs::get_launcher_logs_dir().await.unwrap_or_default();
	let log_file_name = format!("launcher_{}.log", Local::now().format("%Y%m%d_%H%M%S"));
	let log_file_path = logs_dir.join(log_file_name);

	if let Err(err) = std::fs::create_dir_all(&logs_dir) {
		eprintln!("Could not create logs directory: {err}");
	}

	let init_result = match OpenOptions::new()
		.create(true)
		.append(true)
		.open(&log_file_path)
	{
		Ok(file) => {
			let mut file_fmt_layer = tracing_subscriber::fmt::layer()
				.compact()
				.with_writer(file)
				.with_ansi(false)
				.with_timer(ChronoLocal::rfc_3339());

			if let Some(span) = Core::get().logger_span_formatting.clone() {
				file_fmt_layer = file_fmt_layer.with_span_events(span);
			}

			tracing_subscriber::registry()
				.with(console_fmt_layer)
				.with(file_fmt_layer)
				.with(filter)
				.with(tracing_error::ErrorLayer::default())
				.try_init()
		}
		Err(err) => {
			eprintln!("Could not open log file in dev mode: {err}");

			tracing_subscriber::registry()
				.with(console_fmt_layer)
				.with(filter)
				.with(tracing_error::ErrorLayer::default())
				.try_init()
		}
	};

	if let Err(err) = init_result {
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

	use crate::store::{Core, Dirs};

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

	let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
		tracing_subscriber::EnvFilter::new(
			Core::get()
				.logger_filter
				.as_ref()
				.unwrap_or(&format!("{}=info", env!("CARGO_PKG_NAME"))),
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
