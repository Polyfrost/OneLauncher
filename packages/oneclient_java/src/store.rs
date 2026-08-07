//! A port rather than a concrete database handle so this crate does not depend
//! on the launcher's schema

use std::sync::Arc;

use crate::data::JavaRuntime;

/// Opaque so this crate never learns what is behind the port
#[derive(Debug, thiserror::Error)]
#[error("java store: {0}")]
pub struct StoreError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

impl StoreError {
	pub fn new(source: impl std::error::Error + Send + Sync + 'static) -> Self {
		Self(Box::new(source))
	}
}

pub type StoreResult<T> = Result<T, StoreError>;

#[async_trait::async_trait]
pub trait JavaStore: Send + Sync {
	async fn upsert(&self, runtime: &JavaRuntime) -> StoreResult<JavaRuntime>;

	async fn get_by_path(&self, absolute_path: &str) -> StoreResult<Option<JavaRuntime>>;

	async fn latest_by_major(&self, major: u32) -> StoreResult<Option<JavaRuntime>>;

	async fn delete_by_path(&self, absolute_path: &str) -> StoreResult<()>;

	async fn list(&self) -> StoreResult<Vec<JavaRuntime>>;
}

#[derive(Debug, Default)]
pub struct MemoryJavaStore {
	runtimes: std::sync::Mutex<Vec<JavaRuntime>>,
}

impl MemoryJavaStore {
	#[must_use]
	pub fn new() -> Arc<Self> {
		Arc::new(Self::default())
	}
}

#[async_trait::async_trait]
impl JavaStore for MemoryJavaStore {
	async fn upsert(&self, runtime: &JavaRuntime) -> StoreResult<JavaRuntime> {
		let mut runtimes = self.runtimes.lock().expect("java store poisoned");
		runtimes.retain(|existing| existing.absolute_path != runtime.absolute_path);
		runtimes.push(runtime.clone());
		Ok(runtime.clone())
	}

	async fn get_by_path(&self, absolute_path: &str) -> StoreResult<Option<JavaRuntime>> {
		let runtimes = self.runtimes.lock().expect("java store poisoned");
		Ok(runtimes
			.iter()
			.find(|runtime| runtime.absolute_path == absolute_path)
			.cloned())
	}

	async fn latest_by_major(&self, major: u32) -> StoreResult<Option<JavaRuntime>> {
		let runtimes = self.runtimes.lock().expect("java store poisoned");
		Ok(runtimes
			.iter()
			.filter(|runtime| runtime.major == major)
			.max_by(|a, b| a.version.cmp(&b.version))
			.cloned())
	}

	async fn delete_by_path(&self, absolute_path: &str) -> StoreResult<()> {
		let mut runtimes = self.runtimes.lock().expect("java store poisoned");
		runtimes.retain(|runtime| runtime.absolute_path != absolute_path);
		Ok(())
	}

	async fn list(&self) -> StoreResult<Vec<JavaRuntime>> {
		let runtimes = self.runtimes.lock().expect("java store poisoned");
		let mut out = runtimes.clone();
		out.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| b.version.cmp(&a.version)));
		Ok(out)
	}
}
