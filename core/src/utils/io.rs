//! **IO Utilities**
//!
//! Wrapper around [`tokio::io`] for our error system.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::anyhow;
use tauri::async_runtime::spawn_blocking;
use tempfile::NamedTempFile;

/// A wrapper around generic and unhelpful [`std::io::Error`] messages.
#[derive(Debug, thiserror::Error)]
pub enum IOError {
	#[error("{source}, path: {path}")]
	IOErrorWrapper {
		#[source]
		source: std::io::Error,
		path: String,
	},
	#[error(transparent)]
	ZipError(#[from] zip::result::ZipError),
	#[error(transparent)]
	IOError(#[from] std::io::Error),
}

impl IOError {
	pub fn from(source: std::io::Error) -> Self {
		Self::IOError(source)
	}

	pub fn from_zip(source: zip::result::ZipError) -> Self {
		Self::ZipError(source)
	}

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

/// Extract an archive based on its file extension, supporting ZIP files and GZ files.
pub fn extract_archive(archive: &PathBuf, dest: &PathBuf) -> crate::Result<()> {
	let ext = match archive.extension() {
		Some(ext) => ext.to_str().unwrap(),
		None => return Err(anyhow!("unsupported operating system").into()),
	};

	match ext {
		"zip" => extract_zip(archive, dest).map_err(|err| crate::ErrorKind::IOError(err).into()),
		"gz" => extract_tar_gz(archive, dest).map_err(|err| crate::ErrorKind::IOError(err).into()),
		_ => Err(anyhow!("unsupported file extension {:?}", ext).into()),
	}
}

pub fn extract_zip(archive: &PathBuf, dest: &PathBuf) -> Result<(), IOError> {
	let file = File::open(archive).map_err(|err| IOError::with_path(err, archive.as_path()))?;
	let archive = zip::ZipArchive::new(file).map_err(IOError::from_zip);
	archive?
		.extract(dest)
		.map_err(IOError::from_zip)?;
	Ok(())
}

pub fn extract_tar_gz(archive: &PathBuf, dest: &PathBuf) -> Result<(), IOError> {
	let file = File::open(archive).map_err(IOError::from)?;
	let tar_gz = flate2::read::GzDecoder::new(file);
	let mut archive = tar::Archive::new(tar_gz);
	archive.unpack(dest).map_err(IOError::from)?;
	Ok(())
}
