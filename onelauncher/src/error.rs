use tracing_error::InstrumentError;

/// A standardized [`thiserror::Error`] type that is used across the launcher
#[derive(thiserror::Error, Debug)]
pub enum ErrorKind {
	/// Wrapper around [`tokio::sync::AcquireError`] to handle sempahore errors.
	#[error("failed to manage a tokio semaphore: {0}")]
	AcquireError(#[from] tokio::sync::AcquireError),

	/// Wrapper around [`tokio::sync::oneshot::error::RecvError`] to handle recieve errors.
	#[error("tokio recieve error: {0}")]
	RecvError(#[from] tokio::sync::oneshot::error::RecvError),

	/// Wrapper around [`tokio::task::JoinError`] to handle tokio join handle errors.
	#[error("tokio join handle error: {0}")]
	JoinError(#[from] tokio::task::JoinError),

	/// Wrapper around [`flate2::CompressError`] to handle flate compression errors.
	#[error("failed to compress a file with flate: {0}")]
	FlateError(#[from] flate2::CompressError),

	/// Wrapper around [`flate2::DecompressError`] to handle flate decompression errors.
	#[error("failed to decompress a file with flate: {0}")]
	DeflateError(#[from] flate2::DecompressError),

	/// Wrapper around [`uuid::Error`] to handle UUID parsing errors.
	#[error("failed to parse uuids: {0}")]
	UUIDError(#[from] uuid::Error),

	/// Wrapper around [`serde_json::Error`] to handle Serde JSON parsing errors.
	#[error("failed to manage serialization of json: {0}")]
	SerdeError(#[from] serde_json::Error),

	/// Wrapper around [`std::io::Error`] only to be used if a more defined error isn't available (e.g. when using `zip`)
	#[error("failed to manage writing and reading of files: {0}")]
	IOError(#[from] std::io::Error),

	/// Wrapper around [`reqwest::Error`] to handle HTTP errors.
	#[error("failed to establish a HTTP connection: {0}")]
	HTTPError(#[from] reqwest::Error),

	/// Wrapper around [`zip::result::ZipError`] to handle zip errors.
	#[error("failed to manage zip files: {0}")]
	ZipError(#[from] zip::result::ZipError),

	/// Wrapper around [`anyhow::Error`] to handle generic errors.
	#[error(transparent)]
	AnyhowError(#[from] anyhow::Error),

	/// Wrapper around [`chrono::ParseError`] to handle date parsing errors.
	#[error("failed to parse a date: {0}")]
	ChronoError(#[from] chrono::ParseError),

	/// Wrapper around [`crate::game::java::JavaDownloadError`] to handle Java executable downloading.
	#[error("failed to download java: {0}")]
	JavaError(#[from] crate::game::java::JavaDownloadError),

	/// Wrapper around [`crate::auth::AuthenticationError`] to handle authentication errors.
	#[error("failed to authenticate: {0}")]
	AuthError(#[from] crate::auth::AuthenticationError),

	/// Wrapper around [`crate::utils::dirs::DirectoryError`] to handle directory errors.
	#[error("failed to get directory: {0}")]
	DirectoryError(#[from] crate::utils::dirs::DirectoryError),

	/// Wrapper around [`crate::game::client::ClientManagerError`] to handle directory errors.
	#[error("failed to manage clients: {0}")]
	ClientManagerError(#[from] crate::game::client::ClientManagerError),
}

#[derive(Debug)]
pub struct Error {
	source: tracing_error::TracedError<ErrorKind>,
}

impl From<Error> for String {
	fn from(value: Error) -> Self {
		value.to_string()
	}
}

impl std::error::Error for Error {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		self.source.source()
	}
}

impl std::fmt::Display for Error {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(fmt, "{}", self.source)
	}
}

impl<E: Into<ErrorKind>> From<E> for Error {
	fn from(source: E) -> Self {
		Self {
			source: Into::<ErrorKind>::into(source).in_current_span(),
		}
	}
}

impl ErrorKind {
	pub fn as_error(self) -> Error {
		self.into()
	}
}

/// Alias for a [`core::result::Result`] with the error type [`Error`].
pub type Result<T> = anyhow::Result<T, Error>;
