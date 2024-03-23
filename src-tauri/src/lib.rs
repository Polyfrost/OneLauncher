use thiserror::Error;

pub mod auth;
pub mod game;
pub mod utils;
pub mod constants;

/// A standardized [`Error`] type that is used across the launcher
#[derive(Debug, Error)]
pub enum PolyError {
    #[error("a tauri runtime error occured (this should not happen): {0}")]
    /// Wrapper around [`tauri::Error`] to handle expected Tauri errors.
    TauriError(#[from] tauri::Error),
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
    #[error("failed to query the database: {0}")]
    /// Wrapper around [`prisma_client_rust::QueryError`] to handle DB query errors.
    DBQueryError(#[from] prisma_client_rust::QueryError),
    #[error("failed to establish a connection to the database: {0}")]
    /// Wrapper around [`prisma_client_rust::NewClientError`] to handle errors occuring while programmatically connecting to Prisma.
    DBConnectionError(#[from] prisma_client_rust::NewClientError),
    #[error("failed to establish a HTTP connection: {0}")]
    /// Wrapper around [`reqwest::Error`] to handle HTTP errors.
    HTTPError(#[from] tauri_plugin_http::reqwest::Error),
    #[error("failed to download java: {0}")]
    /// Wrapper around [`game::client::JavaDownloadError`] to handle Java executable downloading.
    JavaError(#[from] game::client::JavaDownloadError),
    #[error("file management failed: {0}")]
    /// Wrapper around [`utils::file::FileError`] to handle file management errors.
    FileError(#[from] utils::file::FileError),
}

/// Alias for a [`Result`] with the error type [`PolyError`].
pub type PolyResult<T> = std::result::Result<T, PolyError>;
