use std::path::PathBuf;

use async_compression::tokio::bufread::GzipDecoder;
use async_zip::StoredZipEntry;
use async_zip::base::read::WithoutEntry;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio_util::compat::TokioAsyncWriteCompatExt;

/// A wrapper around generic and unhelpful [`std::io::Error`] messages.
#[derive(Debug, thiserror::Error)]
pub enum IOError {
	#[error("invalid absolute path '{0}'")]
	InvalidAbsolutePath(PathBuf),

	#[error("error acessing path: {source}, path: {path}")]
	IOErrorWrapper {
		#[source]
		source: std::io::Error,
		path: String,
	},
	#[error(transparent)]
	IOError(#[from] std::io::Error),
	#[error("json deserialization error: {0}")]
	DeserializeError(#[from] serde_json::Error),
	#[error(transparent)]
	AsyncZipError(#[from] async_zip::error::ZipError),
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
#[error("received a non UTF-8 path: <lossy_path='{}'>", .0.to_string_lossy())]
pub struct NonUtf8PathError(pub Box<std::path::Path>);

/// Attempts to parse a path taken from an environment variable. Returns `None` if the variable is not set.
pub fn env_path(name: &str) -> Option<PathBuf> {
	std::env::var_os(name).map(PathBuf::from)
}

/// An OS specific wrapper of [`std::fs::canonicalize`], but on Windows it outputs the most compatible form of a path instead of UNC.
pub fn canonicalize(path: impl AsRef<std::path::Path>) -> Result<PathBuf, IOError> {
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

/// Asynchronously reads the entire contents of a file into a bytes vector.
pub async fn read(path: impl AsRef<std::path::Path>) -> Result<Vec<u8>, IOError> {
	let path = path.as_ref();
	tokio::fs::read(path)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Asynchronously read a file as JSON and return the deserialized object
pub async fn read_json<T: DeserializeOwned>(
	path: impl AsRef<std::path::Path>,
) -> Result<T, IOError> {
	Ok(serde_json::from_slice(&read(path).await?)?)
}

/// Asynchrously write to a file.
pub async fn write(
	path: impl AsRef<std::path::Path>,
	data: impl AsRef<[u8]>,
) -> Result<(), IOError> {
	let path = path.as_ref();
	tokio::fs::write(path, data)
		.await
		.map_err(|e| IOError::IOErrorWrapper {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Asynchronously write buffered data to a file, creating it if it does not exist.
pub async fn write_buf<F>(path: impl AsRef<std::path::Path>, f: F) -> Result<(), IOError>
where
	F: AsyncFnOnce(&mut tokio::io::BufWriter<tokio::fs::File>) -> Result<(), IOError>,
{
	let path = path.as_ref();
	let file = tokio::fs::File::create(path).await?;
	let mut writer = tokio::io::BufWriter::new(file);

	f(&mut writer).await?;

	writer.flush().await?;

	Ok(())
}

/// Asynchronously write json to a file, creating it if it does not exist.
pub async fn write_json<T: Serialize>(
	path: impl AsRef<std::path::Path>,
	data: T,
) -> Result<(), IOError> {
	write(path, serde_json::to_vec(&data)?).await
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

// /// Copies an async reader into an async writer. Returns the bytes copied
// pub async fn copy_buf<'a, R, W>(reader: &'a mut R, writer: &'a mut W) -> Result<u64, IOError>
// where
// 	R: TokioAsyncReadCompatExt,
// 	W: TokioAsyncWriteCompatExt,
// {
// 	tokio::io::copy_buf(reader, writer)
// 		.await
// 		.map_err(IOError::from)
// }

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

/// Creates a temporary directory.
pub fn tempdir() -> Result<TempDir, IOError> {
	Ok(tempfile::tempdir()?)
}

/// Creates a temporary file.
pub fn tempfile() -> Result<std::fs::File, IOError> {
	Ok(tempfile::tempfile()?)
}

/// Reads a zip archive from a byte array
pub async fn read_zip_entries_bytes<F>(data: Vec<u8>, mut f: F) -> Result<(), IOError>
where
	F: AsyncFnMut(
		usize,
		&StoredZipEntry,
		&mut async_zip::base::read::ZipEntryReader<
			'_,
			futures_lite::io::Cursor<&[u8]>,
			WithoutEntry,
		>,
	) -> Result<(), IOError>,
{
	let reader = async_zip::base::read::mem::ZipFileReader::new(data).await?;

	let entries = reader.file().entries();
	for index in 0..entries.len() {
		let entry = entries.get(index).expect("expected more zip entries");
		let mut entry_reader = reader
			.reader_without_entry(index)
			.await
			.unwrap_or_else(|_| panic!("expected zip entry at index '{index}'"));

		f(index, entry, &mut entry_reader).await?;
	}

	Ok(())
}

/// Unzips a zip archive from a byte array
pub async fn unzip_bytes(
	data: Vec<u8>,
	dest_path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
	unzip_bytes_filtered(data, None::<fn(&str) -> bool>, dest_path).await
}

/// Unzips a zip archive from a byte array
pub async fn unzip_bytes_filtered(
	data: Vec<u8>,
	filter_entries: Option<impl Fn(&str) -> bool>,
	dest_path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
	read_zip_entries_bytes(data, async |_, entry, entry_reader| {
		let file_name = entry.filename().as_str()?;

		if let Some(filter) = &filter_entries
			&& !filter(file_name) {
				return Ok(());
			}

		let path = dest_path.as_ref().join(sanitize_path(file_name));

		let is_dir = entry.dir()?;

		if is_dir {
			create_dir_all(path).await?;
		} else {
			if let Some(parent) = path.parent() {
				create_dir_all(parent).await?;
			}

			let file = tokio::fs::File::create(&path).await?;
			let writer = tokio::io::BufWriter::new(file);

			futures_lite::io::copy(entry_reader, &mut writer.compat_write()).await?;
		}

		Ok(())
	})
	.await?;

	Ok(())
}

/// Unzips a zip archive from a file
pub async fn unzip_file(
	zip_path: impl AsRef<std::path::Path>,
	dest_path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
	let zip_path = zip_path.as_ref();
	let dest_path = dest_path.as_ref();

	let reader = async_zip::tokio::read::fs::ZipFileReader::new(zip_path).await?;
	let entries = reader.file().entries();

	for index in 0..entries.len() {
		let entry = entries.get(index).expect("expected more zip entries");

		let path = dest_path.join(sanitize_path(entry.filename().as_str()?));
		let is_dir = entry.dir()?;

		if is_dir {
			create_dir_all(&path).await?;
		} else {
			if let Some(parent) = path.parent() {
				create_dir_all(parent).await?;
			}

			let file = tokio::fs::File::create(&path).await?;
			let writer = tokio::io::BufWriter::new(file);
			let entry_reader = reader.reader_without_entry(index).await?;

			futures_lite::io::copy(entry_reader, &mut writer.compat_write()).await?;
		}
	}

	Ok(())
}

pub fn sanitize_path(path: impl AsRef<std::path::Path>) -> PathBuf {
	path.as_ref()
		.to_string_lossy()
		.replace('\\', "/")
		.split('/')
		.map(sanitize_filename::sanitize)
		.collect()
}
