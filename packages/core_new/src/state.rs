use std::sync::Arc;

use tokio::sync::{OnceCell, RwLock};

use crate::LauncherResult;

/// The static [`OnceCell<RwLock<State>>`] for storing the global runtime launcher state.
static LAUNCHER_STATE: OnceCell<RwLock<State>> = OnceCell::const_new();

pub struct State {

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

	async fn initialize() -> LauncherResult<RwLock<Self>> {
		Ok(RwLock::new(Self {

		}))
	}
}