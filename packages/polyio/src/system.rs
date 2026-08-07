use std::path::PathBuf;

use crate::{IOError, PolyIOResult};

pub fn env_path(name: &str) -> Option<PathBuf> {
	std::env::var_os(name).map(PathBuf::from)
}

/// Like [`std::fs::canonicalize`] but on Windows emits the most compatible
/// path form instead of UNC
pub fn canonicalize(path: impl AsRef<std::path::Path>) -> PolyIOResult<PathBuf> {
	let path = path.as_ref();
	dunce::canonicalize(path).map_err(|e| IOError::PathIOError {
		source: e,
		path: path.to_string_lossy().to_string(),
	})
}

