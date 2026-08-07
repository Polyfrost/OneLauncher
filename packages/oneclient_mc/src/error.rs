use oneclient_common::domain::GameLoader;
use thiserror::Error;

pub type McResult<T> = Result<T, McError>;

#[derive(Debug, Error)]
pub enum McError {
	#[error("no client download exists for version {0}")]
	NoClientDownload(String),

	#[error("failed to resolve library artifact path for {0}")]
	LibraryPath(String),

	#[error("invalid game version {0}")]
	InvalidVersion(String),

	#[error("failed to fetch metadata")]
	FetchError,

	#[error("loader {0} does not use a modded manifest")]
	NotModdedManifest(GameLoader),

	#[error("loader {0} does not use a vanilla manifest")]
	NotVanillaManifest(GameLoader),

	#[error("no matching loader found")]
	NoMatchingLoader,

	#[error("requested loader version '{requested}' was not found")]
	RequestedLoaderVersionNotFound { requested: String },

	#[error("no matching version found")]
	NoMatchingVersion,

	#[error("cancelled: {failed} file(s) could not be downloaded")]
	IncompleteInstallCancelled { failed: usize },

	/// A Mojang response was well-formed JSON but not what the API documents
	#[error("minecraft: {0}")]
	Minecraft(String),

	#[error("failed to parse metadata: {0}")]
	ParseError(#[from] serde_json::Error),

	#[error(transparent)]
	Request(#[from] oneclient_net::RequestError),

	#[error(transparent)]
	Io(#[from] polyio::IOError),

	#[error(transparent)]
	StdIo(#[from] std::io::Error),

	#[error(transparent)]
	Paths(#[from] oneclient_common::PathsError),

	#[error("Invalid URL: {0}")]
	Url(#[from] url::ParseError),
}

impl McError {
	/// Environment fault rather than a launcher bug drives Sentry policy upstream
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
