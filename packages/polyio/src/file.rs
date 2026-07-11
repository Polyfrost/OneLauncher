use std::{fs::Metadata, path::PathBuf};

use async_tempfile::{TempDir, TempFile};
use serde::{Serialize, de::DeserializeOwned};
use tracing::instrument;

use crate::{IOError, PolyIOResult};

/// Returns a stream over the entries within a directory.
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn read_dir(path: impl AsRef<std::path::Path>) -> PolyIOResult<tokio::fs::ReadDir> {
	let path = path.as_ref();

	tokio::fs::read_dir(path)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Creates a directory if they are missing.
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn create_dir(path: impl AsRef<std::path::Path>) -> PolyIOResult<()> {
	let path = path.as_ref();
	if path.exists() {
		return Ok(());
	}

	tokio::fs::create_dir(path)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Recursively creates a directory and all of its parent components if they are missing.
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn create_dir_all(path: impl AsRef<std::path::Path>) -> PolyIOResult<()> {
	let path = path.as_ref();
	tokio::fs::create_dir_all(path)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Removes a directory at this path, after removing all its contents. Use carefully!
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn remove_dir_all(path: impl AsRef<std::path::Path>) -> PolyIOResult<()> {
	let path = path.as_ref();

	tokio::fs::remove_dir_all(path)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Checks if a path exists
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn try_exists(path: impl AsRef<std::path::Path>) -> PolyIOResult<bool> {
    let path = path.as_ref();

    tokio::fs::try_exists(path).await
        .map_err(|e| IOError::PathIOError {
            source: e,
            path: path.to_string_lossy().to_string()
        })
}

/// Creates a future which will open a gzip compressed file for reading and read the entire contents into a string and return said string.
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn read_gz_to_string(path: impl AsRef<std::path::Path>) -> PolyIOResult<String> {
	let mut f = tokio::fs::File::open(path).await?;
	let mut buf = vec![];
	tokio::io::AsyncReadExt::read_to_end(&mut f, &mut buf).await?;

	let mut decoder = async_compression::tokio::bufread::GzipDecoder::new(buf.as_slice());
	let mut dst = String::new();
	tokio::io::AsyncReadExt::read_to_string(&mut decoder, &mut dst).await?;

	Ok(dst)
}

/// Creates a future which will open a file for reading and read the entire contents into a string and return said string.
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn read_to_string(path: impl AsRef<std::path::Path>) -> PolyIOResult<String> {
	let path = path.as_ref();

	tokio::fs::read_to_string(path)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Asynchronously reads the entire contents of a file into a bytes vector.
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn read(path: impl AsRef<std::path::Path>) -> PolyIOResult<Vec<u8>> {
	let path = path.as_ref();

	tokio::fs::read(path)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Asynchronously read a file as JSON and return the deserialized object
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn read_json<T: DeserializeOwned>(
	path: impl AsRef<std::path::Path>,
) -> PolyIOResult<T> {
	serde_json::from_slice(&read(&path).await?)
        .map_err(|err| IOError::JsonFileParseError {
            source: err,
            file: path.as_ref().to_path_buf()
        })
}

/// Asynchrously write to a file.
#[instrument(
    level = "debug",
    skip(path, data),
    fields(path = %path.as_ref().display())
)]
pub async fn write(
	path: impl AsRef<std::path::Path>,
	data: impl AsRef<[u8]>,
) -> PolyIOResult<()> {
	let path = path.as_ref();

	tokio::fs::write(path, data)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Asynchronously write buffered data to a file, creating it if it does not exist.
#[instrument(
    level = "debug",
    skip(path, f),
    fields(path = %path.as_ref().display())
)]
pub async fn write_buf<E, F, Fut>(path: impl AsRef<std::path::Path>, f: F) -> Result<(), E>
where
    E: From<IOError>,
    F: for<'a> FnOnce(&'a mut tokio::io::BufWriter<tokio::fs::File>) -> Fut,
    Fut: std::future::Future<Output = Result<(), E>>,
{
	let path = path.as_ref();
	let file = tokio::fs::File::create(path).await.map_err(IOError::from)?;
	let mut writer = tokio::io::BufWriter::new(file);

	let write_result = f(&mut writer).await;

	let flush_result = tokio::io::AsyncWriteExt::flush(&mut writer).await.map_err(IOError::from);

    write_result?;
    flush_result?;

	Ok(())
}

#[instrument(
    level = "debug",
    skip(path, stream),
    fields(path = %path.as_ref().display())
)]
pub async fn write_stream<S, E>(
    path: impl AsRef<std::path::Path>,
    mut stream: S
) -> Result<(), E>
where
    S: futures_lite::Stream<Item = Result<bytes::Bytes, E>> + Unpin + Send,
    E: From<IOError>,
{
    let path = path.as_ref();
    let file = tokio::fs::File::create(path).await.map_err(IOError::from)?;

    let mut writer = tokio::io::BufWriter::new(file);

    let mut write_result = Ok(());
    while let Some(chunk_result) = futures_lite::StreamExt::next(&mut stream).await {
        let chunk = chunk_result?;

        write_result = tokio::io::AsyncWriteExt::write_all(&mut writer, &chunk).await.map_err(IOError::from);
        if write_result.is_err() {
            break;
        };
    };

    let flush_result = tokio::io::AsyncWriteExt::flush(&mut writer).await.map_err(IOError::from);

    write_result?;
    flush_result?;

    Ok(())
}

/// Asynchronously write json to a file, creating it if it does not exist.
#[instrument(
    level = "debug",
    skip(path, data),
    fields(path = %path.as_ref().display())
)]
pub async fn write_json<T: Serialize>(
	path: impl AsRef<std::path::Path>,
	data: T,
) -> PolyIOResult<()> {
	write(&path, serde_json::to_vec(&data)
        .map_err(|err| IOError::JsonFileParseError {
            source: err,
            file: path.as_ref().to_path_buf()
        })?
    ).await
}

/// Renames a file or directory to a new name, replacing the original file if `to` already exists.
#[instrument(
    level = "debug",
    skip(from, to),
    fields(
        from = %from.as_ref().display(),
        to = %to.as_ref().display()
    )
)]
pub async fn rename(
	from: impl AsRef<std::path::Path>,
	to: impl AsRef<std::path::Path>,
) -> PolyIOResult<()> {
	let from = from.as_ref();
	let to = to.as_ref();

	tokio::fs::rename(from, to)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: from.to_string_lossy().to_string(),
		})
}

/// Copies the contents of one file to another. This function will also copy the permission bits of the original file to the destination file. This function will overwrite the contents of to.
#[instrument(
    level = "debug",
    skip(from, to),
    fields(
        from = %from.as_ref().display(),
        to = %to.as_ref().display()
    )
)]
pub async fn copy(
	from: impl AsRef<std::path::Path>,
	to: impl AsRef<std::path::Path>,
) -> PolyIOResult<u64> {
	let from = from.as_ref();
	let to = to.as_ref();

	tokio::fs::copy(from, to)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: from.to_string_lossy().to_string(),
		})
}

/// Removes a file from the filesystem.
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn remove_file(path: impl AsRef<std::path::Path>) -> PolyIOResult<()> {
	let path = path.as_ref();
	tokio::fs::remove_file(path)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Queries metadata about a path without following symlinks.
///
/// Unlike [`stat`], if `path` is a symlink this returns metadata about the
/// link itself, not its target.
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn symlink_metadata(path: impl AsRef<std::path::Path>) -> PolyIOResult<Metadata> {
	let path = path.as_ref();
	tokio::fs::symlink_metadata(path)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

/// Links a file `link` to `original`.
///
/// On Unix this creates a symlink. On Windows it creates a hard link
/// instead (both paths must then live on the same volume). Either way the
/// caller gets a file at `link` that shares the contents of `original`.
#[instrument(
    level = "debug",
    skip(original, link),
    fields(
        original = %original.as_ref().display(),
        link = %link.as_ref().display()
    )
)]
pub async fn symlink_file(
	original: impl AsRef<std::path::Path>,
	link: impl AsRef<std::path::Path>,
) -> PolyIOResult<()> {
	let original = original.as_ref();
	let link = link.as_ref();

	#[cfg(windows)]
	let res = tokio::fs::hard_link(original, link).await;
	#[cfg(not(windows))]
	let res = tokio::fs::symlink(original, link).await;

	res.map_err(|e| IOError::PathIOError {
		source: e,
		path: link.to_string_lossy().to_string(),
	})
}

/// Links a directory `link` to `original`.
///
/// On Windows this creates a directory *junction* (a reparse point), which
/// needs no elevated privilege unlike a real directory symlink. On Unix it
/// creates an ordinary directory symlink. Remove it with [`remove_symlink_dir`].
#[instrument(
    level = "debug",
    skip(original, link),
    fields(
        original = %original.as_ref().display(),
        link = %link.as_ref().display()
    )
)]
pub async fn symlink_dir(
	original: impl AsRef<std::path::Path>,
	link: impl AsRef<std::path::Path>,
) -> PolyIOResult<()> {
	let original = original.as_ref();
	let link = link.as_ref();

	// tokio has no async junction API, so the Windows path runs the blocking
	// std call on the blocking pool; Unix uses tokio's async symlink directly.
	#[cfg(windows)]
	{
		let path = link.to_string_lossy().to_string();
		let original = original.to_path_buf();
		let link = link.to_path_buf();
		return tokio::task::spawn_blocking(move || {
			std::os::windows::fs::junction_point(&original, &link)
		})
		.await
		.map_err(std::io::Error::other)?
		.map_err(|e| IOError::PathIOError { source: e, path });
	}

	#[cfg(not(windows))]
	tokio::fs::symlink(original, link)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: link.to_string_lossy().to_string(),
		})
}

/// Removes a directory link created by [`symlink_dir`].
///
/// On Windows a junction must be removed with `remove_dir` rather than
/// `remove_file`; this handles the platform difference.
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn remove_symlink_dir(path: impl AsRef<std::path::Path>) -> PolyIOResult<()> {
	let path = path.as_ref();

	#[cfg(windows)]
	let res = tokio::fs::remove_dir(path).await;

	#[cfg(not(windows))]
	let res = tokio::fs::remove_file(path).await;

	res.map_err(|e| IOError::PathIOError {
		source: e,
		path: path.to_string_lossy().to_string(),
	})
}

/// Creates a temporary directory.
#[instrument(level = "debug")]
pub async fn tempdir() -> PolyIOResult<TempDir> {
	Ok(TempDir::new().await?)
}

/// Creates a temporary file.
#[instrument(level = "debug")]
pub async fn tempfile() -> PolyIOResult<TempFile> {
	Ok(TempFile::new().await?)
}

/// Makes sure a path is a valid path
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub fn sanitize_path(path: impl AsRef<std::path::Path>) -> PathBuf {
	path.as_ref()
		.to_string_lossy()
		.replace('\\', "/")
		.split('/')
		.map(sanitize_filename::sanitize)
		.collect()
}

/// Returns file metadata
#[instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn stat(path: impl AsRef<std::path::Path>) -> PolyIOResult<Metadata> {
	tokio::fs::metadata(path).await.map_err(IOError::from)
}