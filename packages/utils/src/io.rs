//! **IO Utilities**
//!
//! Async wrapper around [`tokio::fs`] and [`std::io::Error`] for our error system.

use async_compression::tokio::bufread::GzipDecoder;
use std::io::Write;
use tempfile::NamedTempFile;
use tokio::io::AsyncReadExt;
use tokio::task::spawn_blocking;

/// A wrapper around generic and unhelpful [`std::io::Error`] messages.
#[derive(Debug, thiserror::Error)]
#[serde(tag = "type", content = "data")]
pub enum IOError {
	/// A wrapped [`std::io::Error`] along with the path involved in the error.
	#[error("error acessing path: {source}, path: {path}")]
	IOErrorWrapper {
		#[source]
		source: std::io::Error,
		path: String,
	},
	/// A wrapped [`zip::result::ZipError`].
	#[error(transparent)]
	ZipError(#[from] zip::result::ZipError),
	/// A wrapped [`std::io::Error`].
	#[error(transparent)]
	IOError(#[from] std::io::Error),
}

impl<P: AsRef<std::path::Path>> From<(P, std::io::Error)> for IOError {
	fn from((path, source): (P, std::io::Error)) -> Self {
		Self::IOErrorWrapper {
			source,
			path: path.as_ref().to_string_lossy().to_string(),
		}
	}
}

#[derive(Debug, thiserror::Error)]
#[serde(tag = "type", content = "data")]
#[error("received a non UTF-8 path: <lossy_path='{}'>", .0.to_string_lossy())]
pub struct NonUtf8PathError(pub Box<std::path::Path>);

impl IOError {
	/// Converts a [`std::io::Error`] into an [`IOError`].
	#[must_use]
	pub const fn from(source: std::io::Error) -> Self {
		Self::IOError(source)
	}

	/// Converts a [`zip::result::ZipError`] into an [`IOError`].
	#[must_use]
	pub const fn from_zip(source: zip::result::ZipError) -> Self {
		Self::ZipError(source)
	}

	/// Converts a [`std::io::Error`] and the path involved in the error into an [`IOErrror`].
	pub fn with_path(source: std::io::Error, path: impl AsRef<std::path::Path>) -> Self {
		let path = path.as_ref().to_string_lossy().to_string();

		Self::IOErrorWrapper { source, path }
	}
}

/// An OS specific wrapper of [`std::fs::canonicalize`], but on Windows it outputs the most compatible form of a path instead of UNC.
pub fn canonicalize(path: impl AsRef<std::path::Path>) -> Result<std::path::PathBuf, IOError> {
	let path = path.as_ref();
	dunce::canonicalize(path).map_err(|e| IOError::IOErrorWrapper {
		source: e,
		path: path.to_string_lossy().to_string(),
	})
}

/// Returns a stream over the entries within a directory.
pub async fn read_dir(path: impl AsRef<std::path::Path>) -> Result<tokio::fs::ReadDir, IOError> {
	let path = path.as_ref();
	tokio::fs::read_dir(path)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Creates a directory if they are missing.
pub async fn create_dir(path: impl AsRef<std::path::Path>) -> Result<(), IOError> {
	let path = path.as_ref();
	if path.exists() {
		return Ok(());
	}

	tokio::fs::create_dir(path)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Recursively creates a directory and all of its parent components if they are missing.
pub async fn create_dir_all(path: impl AsRef<std::path::Path>) -> Result<(), IOError> {
	let path = path.as_ref();
	tokio::fs::create_dir_all(path)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Removes a directory at this path, after removing all its contents. Use carefully!
pub async fn remove_dir_all(path: impl AsRef<std::path::Path>) -> Result<(), IOError> {
	let path = path.as_ref();
	tokio::fs::remove_dir_all(path)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Creates a future which will open a gzip compressed file for reading and read the entire contents into a string and return said string.
pub async fn read_gz_to_string(path: impl AsRef<std::path::Path>) -> Result<String, IOError> {
	let mut f = tokio::fs::File::open(path).await?;
	let mut buf = vec![];
	f.read_to_end(&mut buf).await?;

	let mut decoder = GzipDecoder::new(buf.as_slice());
	let mut dst = String::new();
	decoder.read_to_string(&mut dst).await?;

	Ok(dst)
}

/// Creates a future which will open a file for reading and read the entire contents into a string and return said string.
pub async fn read_to_string(path: impl AsRef<std::path::Path>) -> Result<String, IOError> {
	let path = path.as_ref();
	tokio::fs::read_to_string(path)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Reads the entire contents of a file into a bytes vector.
pub async fn read(path: impl AsRef<std::path::Path>) -> Result<Vec<u8>, IOError> {
	let path = path.as_ref();
	tokio::fs::read(path)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Asynchrously write to a tempfile that is then transferred to an official [`AsRef<Path>`].
pub async fn write(
	path: impl AsRef<std::path::Path>,
	data: impl AsRef<[u8]>,
) -> Result<(), IOError> {
	let path = path.as_ref().to_owned();
	let data = data.as_ref().to_owned();
	spawn_blocking(move || {
		let cloned_path = path.clone();
		sync_write(data, path).map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: cloned_path.to_string_lossy().to_string(),
		})
	})
	.await
	.map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "tokio task failed"))??;

	Ok(())
}

/// Write to a tempfile that is then transferred to an official [`AsRef<Path>`].
fn sync_write(
	data: impl AsRef<[u8]>,
	path: impl AsRef<std::path::Path>,
) -> Result<(), std::io::Error> {
	let mut tempfile = NamedTempFile::new_in(path.as_ref().parent().ok_or_else(|| {
		std::io::Error::new(
			std::io::ErrorKind::Other,
			"failed to get parent directory of a tempfile",
		)
	})?)?;
	tempfile.write_all(data.as_ref())?;
	let tmp_path = tempfile.into_temp_path();
	let path = path.as_ref();
	// this is a sorta dangerous call but shouldnt matter because of async
	tmp_path.persist(path)?;
	std::io::Result::Ok(())
}

/// Renames a file or directory to a new name, replacing the original file if `to` already exists.
pub async fn rename(
	from: impl AsRef<std::path::Path>,
	to: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
	let from = from.as_ref();
	let to = to.as_ref();
	tokio::fs::rename(from, to)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: from.to_string_lossy().to_string(),
		})
}

/// Copies the contents of one file to another. This function will also copy the permission bits of the original file to the destination file. This function will overwrite the contents of to.
pub async fn copy(
	from: impl AsRef<std::path::Path>,
	to: impl AsRef<std::path::Path>,
) -> Result<u64, IOError> {
	let from = from.as_ref();
	let to = to.as_ref();
	tokio::fs::copy(from, to)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: from.to_string_lossy().to_string(),
		})
}

/// Removes a file from the filesystem.
pub async fn remove_file(path: impl AsRef<std::path::Path>) -> Result<(), IOError> {
	let path = path.as_ref();
	tokio::fs::remove_file(path)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

///// Returns the SHA512 hash of a file.
// pub fn sha512(path: impl AsRef<std::path::Path>) -> Result<String, IOError> {
// 	let path = path.as_ref();
// 	let mut file = std::fs::File::open(path)?;
// 	let mut hasher = sha2::Sha512::new();
// 	std::io::copy(&mut file, &mut hasher)?;

// 	Ok(format!("{:x}", hasher.finalize()))
// }
