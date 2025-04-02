use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::{OnceCell, RwLock};

use crate::{utils::http::{self, FetchSemaphore}, LauncherResult};

use super::{discord::DiscordRPC, ingress::IngressProcessor, Settings};

/// The static [`OnceCell<RwLock<State>>`] for storing the global runtime launcher state.
static LAUNCHER_STATE: OnceCell<RwLock<State>> = OnceCell::const_new();

pub struct State {
	pub ingress_processor: IngressProcessor,
	pub settings: RwLock<Settings>,
	pub db: DatabaseConnection,
	pub rpc: Option<DiscordRPC>,

	pub client: Arc<reqwest::Client>,
	pub(crate) fetch_semaphore: FetchSemaphore,
}

impl State {
	/// Get the current global launcher state (or initialize it)
	pub async fn get() -> LauncherResult<Arc<tokio::sync::RwLockReadGuard<'static, Self>>> {
		Ok(Arc::new(
			LAUNCHER_STATE
				.get_or_try_init(Self::initialize)
				.await?
				.read()
				.await,
		))
	}

	/// Get and lock writing to the current global launcher state (or initalize it)
	pub async fn get_and_write() -> LauncherResult<tokio::sync::RwLockWriteGuard<'static, Self>> {
		Ok(LAUNCHER_STATE
			.get_or_try_init(Self::initialize)
			.await?
			.write()
			.await)
	}

	pub fn initialized() -> bool {
		LAUNCHER_STATE.initialized()
	}

	#[tracing::instrument]
	async fn initialize() -> LauncherResult<RwLock<Self>> {
		let ingress_processor = IngressProcessor::new();
		let settings = Settings::new().await;
		let db = super::db::create_pool().await?;
		
		let rpc = match DiscordRPC::initialize() {
			Ok(rpc) => Some(rpc),
			Err(err) => {
				tracing::warn!("{}", err);
				None
			},
		};

		let reqwest_client = http::create_client()?;
		let fetch_semaphore = FetchSemaphore::new(settings.max_concurrent_requests);

		Ok(RwLock::new(Self {
			ingress_processor,
			settings: RwLock::new(settings),
			db,
			rpc,

			client: Arc::new(reqwest_client),
			fetch_semaphore,
		}))
	}
}