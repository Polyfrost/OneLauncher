#[derive(thiserror::Error, Debug)]
pub enum LauncherError {
	#[error(transparent)]
	DirError(#[from] crate::store::DirectoryError),
	#[error(transparent)]
	IOError(#[from] crate::utils::io::IOError),
	#[error(transparent)]
	IngressError(#[from] crate::store::ingress::IngressError),

	#[error(transparent)]
	SerdeError(#[from] serde_json::Error),
	#[error(transparent)]
	AnyhowError(#[from] anyhow::Error),
	#[error(transparent)]
	DbError(#[from] sea_orm::DbErr),

	#[cfg(feature = "tauri")]
	#[error(transparent)]
	TauriError(#[from] tauri::Error),
}

pub type LauncherResult<T> = Result<T, LauncherError>;