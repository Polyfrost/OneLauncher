use std::path::PathBuf;

use crate::{IOError, PolyIOResult};

/// Attempts to parse a path taken from an environment variable. Returns `None` if the variable is not set.
pub fn env_path(name: &str) -> Option<PathBuf> {
	std::env::var_os(name).map(PathBuf::from)
}

/// An OS specific wrapper of [`std::fs::canonicalize`], but on Windows it outputs the most compatible form of a path instead of UNC.
pub fn canonicalize(path: impl AsRef<std::path::Path>) -> PolyIOResult<PathBuf> {
	let path = path.as_ref();
	dunce::canonicalize(path).map_err(|e| IOError::PathIOError {
		source: e,
		path: path.to_string_lossy().to_string(),
	})
}

