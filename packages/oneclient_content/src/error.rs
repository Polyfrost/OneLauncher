use crate::bundles::BundleError;
use crate::packages::PackageError;

pub type ContentResult<T> = Result<T, ContentError>;

/// The crate's error, funnelling the two subsystem errors plus plumbing.
///
/// `PackageError` and `BundleError` stay distinct because they describe
/// different failures (a provider rejecting a request versus a bundle manifest
/// not matching what is on disk) and callers inside each subsystem still return
/// their own.
#[derive(Debug, thiserror::Error)]
pub enum ContentError {
	#[error(transparent)]
	Package(#[from] PackageError),

	#[error(transparent)]
	Bundle(#[from] BundleError),

	#[error(transparent)]
	Request(#[from] oneclient_net::RequestError),

	#[error(transparent)]
	Db(#[from] oneclient_db::DbError),

	#[error("Database execution failed: {0}")]
	Sql(#[from] sqlx::Error),

	#[error(transparent)]
	Io(#[from] polyio::IOError),

	#[error(transparent)]
	StdIo(#[from] std::io::Error),

	#[error(transparent)]
	Paths(#[from] oneclient_common::PathsError),

	#[error(transparent)]
	Events(#[from] oneclient_events::EventError),

	#[error("Unable to parse JSON: {0}")]
	Json(#[from] serde_json::Error),

	#[error("Invalid URL: {0}")]
	Url(#[from] url::ParseError),

	/// A value that came out of the database or a manifest did not match the
	/// shape the code expects: an unknown content type, a loader id with no
	/// matching variant.
	#[error("invalid content data: {reason}")]
	InvalidData { reason: String },
}

impl ContentError {
	/// Whether this is the user's environment rather than a launcher bug.
	#[must_use]
	pub fn is_transient(&self) -> bool {
		match self {
			Self::Request(source) => source.is_transient(),
			Self::Io(polyio::IOError::IOError(source))
			| Self::Io(polyio::IOError::PathIOError { source, .. })
			| Self::StdIo(source) => {
				use std::io::ErrorKind;
				matches!(
					source.kind(),
					ErrorKind::ConnectionRefused
						| ErrorKind::ConnectionReset
						| ErrorKind::ConnectionAborted
						| ErrorKind::NotConnected
						| ErrorKind::TimedOut
				)
			}
			_ => false,
		}
	}
}
