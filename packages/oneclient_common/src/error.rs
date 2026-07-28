pub type PathsResult<T> = Result<T, PathsError>;

/// The only way path resolution can fail: the platform did not give us a data
/// directory to work in. It stays narrow because callers that hit this cannot
/// do anything except surface it, and keeping it separate from the
/// launcher-wide error keeps this crate a leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathsError {
	#[error("Could not resolve launcher data directory")]
	DataDirUnavailable,
}
