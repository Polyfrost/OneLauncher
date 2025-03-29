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
	DbError(#[from] sqlx::Error),
	#[error(transparent)]
	DbMigrationError(#[from] sqlx::migrate::MigrateError),
	#[error(transparent)]
	AnyhowError(#[from] anyhow::Error),

	#[cfg(feature = "tauri")]
	#[error(transparent)]
	TauriError(#[from] tauri::Error),
}

pub type LauncherResult<T> = Result<T, LauncherError>;