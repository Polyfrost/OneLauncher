use thiserror::Error;

pub type JavaResult<T> = Result<T, JavaError>;

#[derive(Debug, Error)]
pub enum JavaError {
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

impl crate::error::SentryExclusion for JavaError {
	fn is_sentry_excluded(&self) -> bool {
		match self {
			// Expected outcomes: no build exists for the requested version, or the
			// user cancelled setup.
			JavaError::PackageNotFound { .. } | JavaError::Cancelled => true,
			// Environmental IO (e.g. out of disk while extracting the runtime).
			JavaError::RuntimeCheckError { source, .. }
			| JavaError::ArchiveExtract { source, .. } => source.is_sentry_excluded(),
			JavaError::PolyIOError(source) => source.is_sentry_excluded(),
			_ => false,
		}
	}
}