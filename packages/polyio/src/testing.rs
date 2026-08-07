use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A uniquely-named directory under the system temp dir removed when dropped
#[derive(Debug)]
pub struct ScratchDir(PathBuf);

impl ScratchDir {
	/// `tag` only makes a leaked directory identifiable if the destructor is
	/// skipped
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
