//! **OneLauncher Errors**
//!
//! Public wrappers around common OneLauncher errors.

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

	/// Wrapper around [`interpulse::Error`] to handle interpulse errors.
	#[error("metadata error: {0}")]
	MetadataError(#[from] interpulse::Error),

	/// Wrapper around [`serde_json::Error`] to handle Serde JSON parsing errors.
	#[error("failed to manage serialization of json: {0}")]
	SerdeError(#[from] serde_json::Error),

	/// Wrapper around [`serde_ini::de::Error`] to handle Serde INI parsing errors.
	#[error("failed to manage deserialization of ini: {0}")]
	INIError(#[from] serde_ini::de::Error),

	/// Wrapper around [`reqwest::Error`] to handle HTTP errors.
	#[error("failed to establish a HTTP connection: {0}")]
	HTTPError(#[from] reqwest::Error),

	/// Wrapper around [`zip::result::ZipError`] to handle zip errors.
	#[error("failed to manage zip files: {0}")]
	ZipError(#[from] zip::result::ZipError),

	/// Wrapper around [`async_zip::error::ZipError`] to handle async zip errors.
	#[error("failed to manage zip files asynchronously: {0}")]
	AsyncZipError(#[from] async_zip::error::ZipError),

	/// Wrapper around [`anyhow::Error`] to handle generic errors.
	#[error(transparent)]
	AnyhowError(#[from] anyhow::Error),

	/// Wrapper around [`chrono::ParseError`] to handle date parsing errors.
	#[error("failed to parse a date: {0}")]
	ChronoError(#[from] chrono::ParseError),

	/// Wrapper around [`crate::state::DirectoryError`] to handle directory errors.
	#[error("failed to get directory: {0}")]
	DirectoryError(#[from] crate::store::DirectoryError),

	/// Wrapper around [`crate::api::proxy::ProxyError`] to handle ingress errors.
	#[error("failed to handle api communication: {0}")]
	ProxyError(#[from] crate::api::proxy::ProxyError),

	/// Wrapper around [`crate::store::MinecraftAuthError`] to handle Minecraft auth errors.
	#[error("failed to authenticate with microsoft: {0}")]
	AuthError(#[from] crate::store::MinecraftAuthError),

	/// Wrapper around [`crate::api::cluster::create::CreateClusterError`] to handle Cluster creation errors.
	#[error("failed to create clusters: {0}")]
	CreateClusterError(#[from] crate::api::cluster::create::CreateClusterError),

	/// Wrapper around [`notify:Error`] to handle file watching errors.
	#[error("failed to watch file {0}")]
	NotifyError(#[from] notify::Error),

	/// Wrapper around [`regex::Error`] to handle RegExp errors.
	#[error("string verification with regex failed: {0}")]
	RegexError(#[from] regex::Error),

	/// Wrapper around [`reqwest::header::ToStrError`] to handle header conversion errors.
	#[error("failed to convert header to string: {0}")]
	HeaderToStrError(#[from] reqwest::header::ToStrError),

	/// Wrapper around [`async_tungstenite::tungstenite::Error`] to handle WebSocket errors.
	#[error("failed to handle websocket connection: {0}")]
	WebSocketError(#[from] async_tungstenite::tungstenite::Error),

	/// Indicates an error with SHA checksum.
	#[error("checksum {0} did not match {1}!")]
	HashError(String, String),

	/// Wrapper around [`crate::utils::io::IOError`] to handle wrapped std::io errors.
	#[error("error handling IO operations: {0}")]
	IOError(#[from] crate::utils::io::IOError),

	/// Wrapper around [`std::io::Error`] to handle non-wrapped std::io errors.
	#[error("I/O (std) error: {0}")]
	StdIOError(#[from] std::io::Error),

	/// Wrapper around [`crate::utils::java::JavaError`] to handle java errors.
	#[error("there was an error managing java installations: {0}")]
	JavaError(#[from] crate::utils::java::JavaError),

	/// Wrapper around [`crate::store::StrongholdError`] to handle stronghold errors.
	#[error("failed to manage credentials with IOTA stronghold: {0}")]
	StrongholdError(#[from] crate::store::StrongholdError),

	/// Wrapper around [`tauri::Error`] to handle Tauri errors when the feature flag is enabled
	#[cfg(feature = "tauri")]
	#[error("Tauri error: {0}")]
	TauriError(#[from] tauri::Error),
}

#[derive(Debug)]
pub struct Error {
	pub raw: std::sync::Arc<ErrorKind>,
	pub source: tracing_error::TracedError<std::sync::Arc<ErrorKind>>,
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
		let error = Into::<ErrorKind>::into(source);
		let boxed_error = std::sync::Arc::new(error);

		Self {
			raw: boxed_error.clone(),
			source: boxed_error.in_current_span(),
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
