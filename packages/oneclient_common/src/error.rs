pub type PathsResult<T> = Result<T, PathsError>;

/// Kept separate from the launcher-wide error so this crate stays a leaf
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathsError {
	#[error("Could not resolve launcher data directory")]
	DataDirUnavailable,
}
