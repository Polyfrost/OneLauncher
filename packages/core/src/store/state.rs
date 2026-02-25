use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::{OnceCell, RwLock};

use crate::LauncherResult;
use crate::store::Dirs;

use super::Settings;
use super::credentials::CredentialsStore;
use super::discord::DiscordRPC;
use super::ingress::IngressProcessor;
use super::metadata::Metadata;
use super::processes::ProcessStore;

/// The static [`OnceCell<RwLock<State>>`] for storing the global runtime launcher state.
static LAUNCHER_STATE: OnceCell<Arc<State>> = OnceCell::const_new();

pub struct State {
	pub ingress_processor: IngressProcessor,
	pub settings: RwLock<Settings>,
	pub db: DatabaseConnection,
	pub credentials: RwLock<CredentialsStore>,
	pub metadata: RwLock<Metadata>,
	pub processes: ProcessStore,
	/// Discord RPC client. Wrapped in `RwLock` so it can be lazily initialized
	/// after startup if Discord wasn't running when the launcher launched.
	pub rpc: RwLock<Option<DiscordRPC>>,
}

impl State {
	/// Get the current global launcher state (or initialize it)
	pub async fn get() -> LauncherResult<Arc<Self>> {
		Ok(LAUNCHER_STATE
			.get_or_try_init(Self::initialize)
			.await?
			.clone())
	}

	pub fn initialized() -> bool {
		LAUNCHER_STATE.initialized()
	}

	#[tracing::instrument]
	async fn initialize() -> LauncherResult<Arc<Self>> {
		crate::utils::io::create_dir_all(Dirs::get().await?.base_dir()).await?;

		let ingress_processor = IngressProcessor::new();
		let settings = Settings::new().await;
		let db = super::db::create_pool().await?;
		let credentials = CredentialsStore::initialize().await?;
		let metadata = Metadata::new();
		let processes = ProcessStore::new();
		let rpc = match DiscordRPC::initialize() {
			Ok(rpc) => Some(rpc),
			Err(err) => {
				tracing::warn!("{}", err);
				None
			}
		};

		Ok(Arc::new(Self {
			ingress_processor,
			settings: RwLock::new(settings),
			db,
			credentials: RwLock::new(credentials),
			metadata: RwLock::new(metadata),
			processes,
			rpc: RwLock::new(rpc),
		}))
	}

	/// Get or lazily initialize the Discord RPC.
	///
	/// If RPC was `None` at startup (Discord wasn't running), this will attempt
	/// to initialize it now. Returns `true` if RPC is available after the call.
	pub async fn ensure_rpc(&self) -> bool {
		// Fast path: already have a client
		if self.rpc.read().await.is_some() {
			return true;
		}

		// Try to initialize for the first time
		match DiscordRPC::initialize() {
			Ok(rpc) => {
				*self.rpc.write().await = Some(rpc);
				tracing::info!("Discord RPC lazily initialized");
				true
			}
			Err(err) => {
				tracing::debug!("Discord RPC lazy init failed: {err}");
				false
			}
		}
	}
}
