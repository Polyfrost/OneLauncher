#[derive(thiserror::Error, Debug)]
pub enum LauncherError {
	#[error(transparent)]
	DirError(#[from] crate::store::DirectoryError),
	#[error(transparent)]
	IOError(#[from] crate::utils::io::IOError),
	#[error(transparent)]
	IngressError(#[from] crate::store::ingress::IngressError),
	#[error(transparent)]
	JavaError(#[from] crate::api::java::JavaError),
	#[error(transparent)]
	CryptoError(#[from] crate::utils::crypto::CryptoError),
	#[error(transparent)]
	DiscordError(#[from] crate::store::discord::DiscordError),
	#[error(transparent)]
	MetadataError(#[from] crate::store::metadata::MetadataError),
	#[error(transparent)]
	ClusterError(#[from] crate::api::cluster::ClusterError),
	#[error(transparent)]
	MinecraftAuthError(#[from] crate::store::credentials::MinecraftAuthError),
	#[error(transparent)]
	ProcessError(#[from] crate::store::processes::ProcessError),
	#[error(transparent)]
	PackageError(#[from] crate::api::packages::PackageError),
	#[error(transparent)]
	DaoError(#[from] DaoError),

	#[error("json error: {0}")]
	SerdeError(#[from] serde_json::Error),
	#[error(transparent)]
	AnyhowError(#[from] anyhow::Error),
	#[error("database error: {0}")]
	DbError(#[from] sea_orm::DbErr),
	#[error("http error: {0}")]
	ReqwestError(#[from] reqwest::Error),
	#[error("meta error: {0}")]
	InterpulseError(#[from] interpulse::Error),
	#[error(transparent)]
	RegexError(#[from] regex::Error),
	#[error("couldn't acquire semaphore: {0}")]
	SemaphoreError(#[from] tokio::sync::AcquireError),
	#[error(transparent)]
	UrlError(#[from] url::ParseError),

	#[cfg(feature = "tauri")]
	#[error(transparent)]
	TauriError(#[from] tauri::Error),
}

pub type LauncherResult<T> = Result<T, LauncherError>;

#[derive(thiserror::Error, Debug)]
pub enum DaoError {
	#[error("entity was not found")]
	NotFound,
	#[error("entity already exists")]
	AlreadyExists,
	#[error("invalid value '{0}' for {1}")]
	InvalidValue(String, String),
}
