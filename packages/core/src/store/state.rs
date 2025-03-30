use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::{OnceCell, RwLock};

use crate::LauncherResult;

use super::{ingress::IngressProcessor, Settings};

/// The static [`OnceCell<RwLock<State>>`] for storing the global runtime launcher state.
static LAUNCHER_STATE: OnceCell<RwLock<State>> = OnceCell::const_new();

pub struct State {
	pub ingress_processor: IngressProcessor,
	pub settings: Settings,
	pub db: DatabaseConnection,
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

		Ok(RwLock::new(Self {
			ingress_processor,
			settings,
			db,
		}))
	}
}