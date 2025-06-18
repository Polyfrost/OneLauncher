use std::{ops::Deref, sync::Arc};

use tokio::sync::OnceCell;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::error::{LauncherError, LauncherResult};

static CORE_STATE: OnceCell<Arc<Core>> = OnceCell::const_new();

pub struct Core(CoreOptions);

/// Used for "customizable constants" for the core
pub struct CoreOptions {
	pub discord_client_id: Option<String>,
	pub launcher_name: String,
	pub launcher_version: String,
	pub launcher_website: String,
	pub fetch_attempts: usize,
	/// Default client id is the same as the one used by the official launcher
	pub msa_client_id: String,
	pub msa_redirect_uri: String,
	pub curseforge_api_key: Option<String>,
	pub logger_span_formatting: Option<FmtSpan>,
	pub logger_filter: Option<String>,
}

impl Default for CoreOptions {
	fn default() -> Self {
		Self {
			discord_client_id: None,
			launcher_name: String::from("Launcher"),
			launcher_version: String::from(env!("CARGO_PKG_VERSION")),
			launcher_website: String::from("https://polyfrost.org/"),
			fetch_attempts: 3,
			msa_client_id: String::from("00000000402b5328"),
			msa_redirect_uri: String::from("https://login.live.com/oauth20_desktop.srf"),
			curseforge_api_key: None,
			logger_span_formatting: None,
			logger_filter: None,
		}
	}
}

impl Core {
	#[must_use]
	pub fn new(options: CoreOptions) -> Arc<Self> {
		Arc::new(Self(options))
	}

	pub async fn initialize(options: CoreOptions) -> LauncherResult<()> {
		CORE_STATE.get_or_try_init(|| async move {
			Ok::<_, LauncherError>(Self::new(options))
		}).await?;

		tracing::info!("core initialized successfully");

		Ok(())
	}

	pub fn get() -> Arc<Self> {
		CORE_STATE
			.get()
			.expect("core was not initialized")
			.clone()
	}
}

impl Deref for Core {
	type Target = CoreOptions;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

