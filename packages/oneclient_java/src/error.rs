use thiserror::Error;

pub type JavaResult<T> = Result<T, JavaError>;

#[derive(Debug, Error)]
pub enum JavaError {
	#[error(transparent)]
	Store(#[from] crate::store::StoreError),

	#[error(transparent)]
	Request(#[from] oneclient_net::RequestError),

	#[error(transparent)]
	Events(#[from] oneclient_events::EventError),

	#[error(transparent)]
	Paths(#[from] oneclient_common::PathsError),

	#[error("Invalid URL: {0}")]
	Url(#[from] url::ParseError),

	#[error("Error spawning Java runtime from path '{path}': {source}")]
	RuntimeCheckError {
		#[source]
		source: std::io::Error,
		path: String,
	},

	#[error("IO Error: {0}")]
	PolyIOError(#[from] polyio::IOError),

	#[error("no java package found for major version {major}")]
	PackageNotFound { major: u32 },

	#[error("no java installations available")]
	MissingJava,

	#[error("java setup was cancelled")]
	Cancelled,

	#[error("selected path is not a valid java installation: {path}")]
	InvalidJavaPath { path: String },

	#[error("selected java is version {found}, but version {expected} is required")]
	VersionMismatch { expected: u32, found: u32 },

	#[error("failed to parse java version '{version}'")]
	ParseVersion { version: String },

	#[error("failed to extract archive '{archive}': {source}")]
	ArchiveExtract {
		archive: String,
		source: std::io::Error,
	},

	#[error("failed to extract archive '{archive}'")]
	ArchiveExtractFailed { archive: String },
}

impl JavaError {
	/// Whether this is an expected outcome or the user's environment, rather
	/// than a launcher bug.
	///
	/// The crate states the fact; turning it into "keep this out of Sentry" is
	/// a policy the composition layer applies, since nothing down here should
	/// know that Sentry exists.
	#[must_use]
	pub fn is_transient(&self) -> bool {
		match self {
			// Expected outcomes: no build exists for the requested version, or
			// the user cancelled setup.
			JavaError::PackageNotFound { .. } | JavaError::Cancelled => true,
			// Environmental IO (e.g. out of disk while extracting the runtime).
			JavaError::RuntimeCheckError { source, .. }
			| JavaError::ArchiveExtract { source, .. } => is_transient_io(source),
			JavaError::PolyIOError(polyio::IOError::IOError(source))
			| JavaError::PolyIOError(polyio::IOError::PathIOError { source, .. }) => {
				is_transient_io(source)
			}
			JavaError::Request(source) => source.is_transient(),
			_ => false,
		}
	}
}

fn is_transient_io(error: &std::io::Error) -> bool {
	use std::io::ErrorKind;

	if let Some(code) = error.raw_os_error() {
		#[cfg(unix)]
		if code == 28 {
			// ENOSPC: out of disk part-way through extracting a runtime.
			return true;
		}
		#[cfg(windows)]
		if code == 112 || code == 39 {
			return true;
		}
		let _ = code;
	}

	matches!(
		error.kind(),
		ErrorKind::ConnectionRefused
			| ErrorKind::ConnectionReset
			| ErrorKind::ConnectionAborted
			| ErrorKind::NotConnected
			| ErrorKind::TimedOut
	)
}