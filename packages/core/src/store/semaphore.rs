use std::sync::Arc;

use tokio::sync::{OnceCell, Semaphore};

static STORE_STATE: OnceCell<Arc<SemaphoreStore>> = OnceCell::const_new();

pub struct SemaphoreStore {
	pub fetch: Arc<Semaphore>
}

impl SemaphoreStore {
	pub async fn get() -> Arc<Self> {
		STORE_STATE.get_or_init(|| async { Self::initialize() }).await.clone()
	}

	#[tracing::instrument]
	fn initialize() -> Arc<Self> {
		Arc::new(Self {
			fetch: Arc::new(Semaphore::new(10)), // Set the limit to 10
		})
	}

	pub async fn fetch() -> Arc<Semaphore> {
		Self::get().await.fetch.clone()
	}
}