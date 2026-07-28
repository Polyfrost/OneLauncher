//! Helpers for tests. Gated behind the `testing` feature so they stay out of
//! release builds; enable it as a dev-dependency.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A uniquely-named directory under the system temp dir, removed when dropped.
///
/// Prefer this over a bare `std::env::temp_dir().join(...)`: hand-rolled
/// versions of this leak their directory on every run, because there is nothing
/// to remove it once the test finishes.
///
/// The name embeds the process id and a counter, so concurrent tests (and
/// concurrent *runs*) cannot collide.
#[derive(Debug)]
pub struct ScratchDir(PathBuf);

impl ScratchDir {
	/// Creates the directory. `tag` is only there to make a stray directory
	/// identifiable if a test aborts hard enough to skip the destructor.
	#[must_use]
	pub fn new(tag: &str) -> Self {
		static COUNTER: AtomicU64 = AtomicU64::new(0);
		let n = COUNTER.fetch_add(1, Ordering::Relaxed);

		let path = std::env::temp_dir()
			.join(format!("oneclient-{tag}-{}-{n}", std::process::id()));
		std::fs::create_dir_all(&path)
			.unwrap_or_else(|e| panic!("failed to create scratch dir {}: {e}", path.display()));

		Self(path)
	}

	#[must_use]
	pub fn path(&self) -> &Path {
		&self.0
	}

	#[must_use]
	pub fn join(&self, name: impl AsRef<Path>) -> PathBuf {
		self.0.join(name)
	}
}

impl AsRef<Path> for ScratchDir {
	fn as_ref(&self) -> &Path {
		&self.0
	}
}

impl Drop for ScratchDir {
	fn drop(&mut self) {
		let _ = std::fs::remove_dir_all(&self.0);
	}
}
