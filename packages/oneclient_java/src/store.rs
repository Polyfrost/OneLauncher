//! Where located and installed runtimes are remembered.
//!
//! Defined as a port rather than a concrete database handle so this crate does
//! not depend on the launcher's schema. `java_versions` is a leaf table with a
//! single owner (this crate) and no joins, so the interface stays five methods
//! wide, unlike the artifact and cluster tables where a trait would just be the
//! SQL schema with extra steps.
//!
//! So `oneclient_java` has no database in its dependency tree, and its examples
//! and tests can run against an in-memory store instead of standing up a real
//! SQLite file.

use std::sync::Arc;

use crate::data::JavaRuntime;

/// Whatever went wrong in the backing store, kept opaque so this crate never
/// learns what is behind the port.
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
	/// Records a runtime, replacing any entry with the same path.
	async fn upsert(&self, runtime: &JavaRuntime) -> StoreResult<JavaRuntime>;

	async fn get_by_path(&self, absolute_path: &str) -> StoreResult<Option<JavaRuntime>>;

	/// The newest recorded runtime for a major version, if any.
	async fn latest_by_major(&self, major: u32) -> StoreResult<Option<JavaRuntime>>;

	async fn delete_by_path(&self, absolute_path: &str) -> StoreResult<()>;

	async fn list(&self) -> StoreResult<Vec<JavaRuntime>>;
}

/// An in-memory [`JavaStore`], for tests and for examples that only want to
/// exercise the vendor providers.
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
