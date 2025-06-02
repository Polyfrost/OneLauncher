use serde::Serialize;

#[onelauncher_macro::specta]
#[derive(thiserror::Error, Debug, Serialize)]
#[serde(tag = "type", content = "data")]
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
	SerdeError(#[from] #[serde(skip)] serde_json::Error),
	#[error(transparent)]
	AnyhowError(#[from] #[serde(skip)] anyhow::Error),
	#[error("database error: {0}")]
	DbError(#[from] #[serde(skip)] sea_orm::DbErr),
	#[error("http error: {0}")]
	ReqwestError(#[from] #[serde(skip)] reqwest::Error),
	#[error("meta error: {0}")]
	InterpulseError(#[from] #[serde(skip)] interpulse::Error),
	#[error(transparent)]
	RegexError(#[from] #[serde(skip)] regex::Error),
	#[error("couldn't acquire semaphore: {0}")]
	SemaphoreError(#[from] #[serde(skip)] tokio::sync::AcquireError),
	#[error(transparent)]
	UrlError(#[from] #[serde(skip)] url::ParseError),

	#[cfg(feature = "tauri")]
	#[error(transparent)]
	TauriError(#[from] #[serde(skip)] tauri::Error),
}

// impl serde::Serialize for LauncherError {
// 	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
// 	where
// 		S: serde::Serializer,
// 	{
// 		// Serialize the error as a string
// 		serializer.serialize_str(&self.to_string())
// 	}
// }

pub type LauncherResult<T> = Result<T, LauncherError>;

#[onelauncher_macro::specta]
#[derive(thiserror::Error, Debug, Serialize)]
pub enum DaoError {
	#[error("entity was not found")]
	NotFound,
	#[error("entity already exists")]
	AlreadyExists,
	#[error("invalid value '{0}' for {1}")]
	InvalidValue(String, String),
}
