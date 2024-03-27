use tracing_appender::non_blocking::WorkerGuard;

#[cfg(debug_assertions)]
pub fn start_logger() -> Option<WorkerGuard> {
	use tracing_subscriber::prelude::*;

	let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
		tracing_subscriber::EnvFilter::new("onelauncher=info,onelauncher_gui=info")
	});

	let subscriber = tracing_subscriber::registry()
		.with(tracing_subscriber::fmt::layer())
		.with(filter)
		.with(tracing_error::ErrorLayer::default());

	tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

	None
}

#[cfg(not(debug_assertions))]
pub fn start_logger() -> Option<WorkerGuard> {
	use crate::game::client::DirectoryInfo;
	use tracing_appender::rolling::{RollingFileAppender, Rotation};
	use tracing_subscriber::fmt::time::ChronoLocal;
	use tracing_subscriber::prelude::*;

	let logs_dir = if let Some(directory) = DirectoryInfo::logs_dir() {
		directory
	} else {
		eprintln!("failed to start logger");
		return None;
	};

	let filter = tracing_subscriber::EnvFilter::try_from_default_env()
		.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("onelauncher=info"));

	let file_appender = RollingFileAppender::new(Rotation::DAILY, logs_dir, "onelauncher.log");
	let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

	let subscriber = tracing_subscriber::registry()
		.with(
			tracing_subscriber::fmt::layer()
				.with_writer(non_blocking)
				.with_ansi(false)
				.with_timer(ChronoLocal::rfc3339()),
		)
		.with(filter)
		.with(tracing_error::ErrorLayer::default());

	tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

	Some(guard);
}
