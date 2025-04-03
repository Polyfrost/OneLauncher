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
	DaoError(#[from] DaoError),

	#[error(transparent)]
	SerdeError(#[from] serde_json::Error),
	#[error(transparent)]
	AnyhowError(#[from] anyhow::Error),
	#[error(transparent)]
	DbError(#[from] sea_orm::DbErr),
	#[error(transparent)]
	ReqwestError(#[from] reqwest::Error),
	#[error("couldn't acquire semaphore: {0}")]
	SemaphoreError(#[from] tokio::sync::AcquireError),

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
	AlreadyExists
}
