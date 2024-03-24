use thiserror::Error;

pub mod constants;
pub mod utils;
pub mod game;
pub mod auth;

/// A standardized [`Error`] type that is used across the launcher
#[derive(Debug, Error)]
pub enum PolyError {
	/// Wrapper around [`tokio::sync::AcquireError`] to handle sempahore errors.
	#[error("failed to manage a tokio semaphore: {0}")]
	TokioError(#[from] tokio::sync::AcquireError),

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

	/// Wrapper around [`game::java::JavaDownloadError`] to handle Java executable downloading.
	#[error("failed to download java: {0}")]
	JavaError(#[from] game::java::JavaDownloadError),

	/// Wrapper around [`auth::AuthenticationError`] to handle authentication errors.
	#[error("failed to authenticate: {0}")]
	AuthError(#[from] auth::AuthenticationError),

	/// Wrapper around [`utils::dirs::DirectoryError`] to handle directory errors.
	#[error("failed to get directory: {0}")]
	DirectoryError(#[from] utils::dirs::DirectoryError),

    	/// Wrapper around [`game::client::ClientManagerError`] to handle directory errors.
	#[error("failed to manage clients: {0}")]
	ClientManagerError(#[from] game::client::ClientManagerError),
}

impl From<PolyError> for String {
    fn from(value: PolyError) -> Self {
        value.to_string()
    }
}

/// Alias for a [`Result`] with the error type [`PolyError`].
pub type PolyResult<T> = anyhow::Result<T, PolyError>;