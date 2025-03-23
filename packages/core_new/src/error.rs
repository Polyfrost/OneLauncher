#[derive(thiserror::Error, Debug)]
pub enum LauncherError {
	#[error(transparent)]
	DirError(#[from] crate::io::DirectoryError),
}

pub type LauncherResult<T> = Result<T, LauncherError>;