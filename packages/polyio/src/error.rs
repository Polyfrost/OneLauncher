use std::path::PathBuf;

pub type PolyIOResult<T> = Result<T, IOError>;

#[derive(Debug, thiserror::Error)]
pub enum IOError {
	#[error("Invalid absolute path '{path}'")]
	InvalidAbsolutePath {
        path: PathBuf,
    },

	#[error("Couldn't find file '{file_name}' in zip")]
	FileNotFoundInZip {
        file_name: String,
    },

	#[error("An error occurred whilst accessing path '{path}': {source}")]
	PathIOError {
		#[source]
		source: std::io::Error,
		path: String,
	},

	#[error(transparent)]
	IOError(
		#[from]
		std::io::Error,
	),

	#[error("Failed to parse JSON for file '{file}': {source}")]
	JsonFileParseError {
		#[source]
		source: serde_json::Error,
        file: PathBuf
    },

    #[error("Failed to convert to JSON for file '{file}': {source}")]
    JsonFileWrite {
        #[source]
        source: serde_json::Error,
        file: PathBuf
    },

	#[error(transparent)]
	AsyncZipError(
		#[from]
		async_zip::error::ZipError,
	),

	#[error("Temporary file error: {0}")]
	TempFileError(
		#[from]
		async_tempfile::Error,
	),
}