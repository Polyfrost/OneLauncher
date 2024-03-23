use thiserror::Error;

pub mod auth;
pub mod constants;
pub mod game;
pub mod utils;

pub struct AppState {
    // pub ,
}

/// A standardized [`Error`] type that is used across the launcher
#[derive(Debug, Error)]
pub enum PolyError {
	#[error("a tauri runtime error occured (this should not happen): {0}")]
	/// Wrapper around [`tauri::Error`] to handle expected Tauri errors.
	TauriError(#[from] tauri::Error),
	#[error("failed to manage shell task: {0}")]
	/// Wrapper around [`tauri_plugin_shell::Error`] to handle shell errors.
	TauriShellError(#[from] tauri_plugin_shell::Error),
	#[error("failed to manage a tokio semaphore: {0}")]
	/// Wrapper around [`tokio::sync::AcquireError`] to handle sempahore errors.
	TokioError(#[from] tokio::sync::AcquireError),
	#[error("failed to compress a file with flate: {0}")]
	/// Wrapper around [`flate2::CompressError`] to handle flate compression errors.
	FlateError(#[from] flate2::CompressError),
	// im actually the funniest person alive dont @ me
	#[error("failed to decompress a file with flate: {0}")]
	/// Wrapper around [`flate2::DecompressError`] to handle flate decompression errors.
	DeflateError(#[from] flate2::DecompressError),
	#[error("failed to parse uuids: {0}")]
	/// Wrapper around [`uuid::Error`] to handle UUID parsing errors.
	UUIDError(#[from] uuid::Error),
	#[error("failed to manage serialization of json: {0}")]
	/// Wrapper around [`serde_json::Error`] to handle Serde JSON parsing errors.
	SerdeError(#[from] serde_json::Error),
	#[error("failed to manage writing and reading of files: {0}")]
	/// Wrapper around [`std::io::Error`] only to be used if a more defined error isn't available (e.g. when using `zip`)
	IOError(#[from] std::io::Error),
	#[error("failed to establish a HTTP connection: {0}")]
	/// Wrapper around [`reqwest::Error`] to handle HTTP errors.
	HTTPError(#[from] tauri_plugin_http::reqwest::Error),
	#[error("failed to download java: {0}")]
	/// Wrapper around [`game::client::JavaDownloadError`] to handle Java executable downloading.
	JavaError(#[from] game::java::JavaDownloadError),
	/// Wrapper around [`zip::result::ZipError`] to handle zip errors.
	#[error("failed to manage zip files: {0}")]
	ZipError(#[from] zip::result::ZipError),
	#[error("failed to authenticate: {0}")]
	/// Wrapper around [`auth::AuthenticationError`] to handle authentication errors.
	AuthError(#[from] auth::AuthenticationError),
    /// Wrapper around [`anyhow::Error`] to handle generic errors.
	#[error(transparent)]
	AnyhowError(#[from] anyhow::Error),
    /// Wrapper around [`chrono::ParseError`] to handle date parsing errors.
    #[error("failed to parse a date: {0}")]
    ChronoError(#[from] chrono::ParseError),
}

/// Alias for a [`Result`] with the error type [`PolyError`].
pub type PolyResult<T> = anyhow::Result<T, PolyError>;
