use std::sync::atomic::{AtomicU64, Ordering};
use std::{fs::Metadata, path::{Path, PathBuf}};

use async_tempfile::{TempDir, TempFile};
use serde::{Serialize, de::DeserializeOwned};

use crate::{IOError, PolyIOResult};

/// Smallest write buffer. HTTP chunks already arrive at ~16 KiB, so anything
/// below this is buffering that was already the right size.
const MIN_WRITE_BUFFER: usize = 16 * 1024;
/// Largest write buffer, for multi-megabyte downloads.
const MAX_WRITE_BUFFER: usize = 256 * 1024;
/// Used when the response has no length: assume something mid-sized.
const DEFAULT_WRITE_BUFFER: usize = 64 * 1024;

/// Picks a write buffer that fits the file rather than a fixed large one.
///
/// This is a memory-footprint choice, not a throughput one: measured against
/// the real asset-size distribution, 64 KiB / 256 KiB / file-sized all write at
/// the same rate (~200 MB/s, ~25x faster than the download feeding them), so
/// the buffer is nowhere near the critical path. What a fixed 256 KiB *does*
/// cost is 8 MiB of buffers across 32 concurrent asset downloads, each holding
/// a ~10 KiB object.
fn write_buffer_size(size_hint: Option<u64>) -> usize {
	match size_hint {
		Some(0) | None => DEFAULT_WRITE_BUFFER,
		Some(size) => (size.min(MAX_WRITE_BUFFER as u64) as usize).max(MIN_WRITE_BUFFER),
	}
}

/// Returns a stream over the entries within a directory.
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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

#[tracing::instrument(
    level = "debug",
    skip(path, stream),
    fields(path = %path.as_ref().display())
)]
pub async fn write_stream<S, E>(
    path: impl AsRef<std::path::Path>,
    mut stream: S,
    size_hint: Option<u64>,
) -> Result<(), E>
where
    S: futures_lite::Stream<Item = Result<bytes::Bytes, E>> + Unpin + Send,
    E: From<IOError>,
{
    let path = path.as_ref();
    let file = tokio::fs::File::create(path).await.map_err(IOError::from)?;

    let mut writer = tokio::io::BufWriter::with_capacity(write_buffer_size(size_hint), file);

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
#[tracing::instrument(
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

/// Counter for [`temp_sibling`] names, so two concurrent atomic writes to the
/// same path can't pick the same scratch file.
static ATOMIC_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch path next to `path`, so the subsequent rename stays within one
/// filesystem. A temp dir would not: `rename` across mount points fails with
/// `EXDEV`, which is exactly the case on Linux where `/tmp` is often a tmpfs.
fn temp_sibling(path: &Path) -> PathBuf {
	let n = ATOMIC_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
	let stem = path
		.file_name()
		.map(|n| n.to_string_lossy().to_string())
		.unwrap_or_else(|| "tmp".to_string());

	let name = format!(".{stem}.{}.{n}.tmp", std::process::id());
	match path.parent() {
		Some(parent) => parent.join(name),
		None => PathBuf::from(name),
	}
}

/// Writes a file such that a reader only ever sees the old contents or the
/// complete new ones, never a half-written file.
///
/// Writes to a sibling scratch file, fsyncs it, then renames over `path`.
/// Without the fsync the rename can land before the data does, so a crash
/// leaves a correctly-named zero-length file, worse than the torn write this
/// is meant to prevent. Parent directories are created if missing.
#[tracing::instrument(
    level = "debug",
    skip(path, data),
    fields(path = %path.as_ref().display())
)]
pub async fn write_atomic(
	path: impl AsRef<Path>,
	data: impl AsRef<[u8]>,
) -> PolyIOResult<()> {
	let path = path.as_ref();

	if let Some(parent) = path.parent()
		&& !parent.as_os_str().is_empty()
	{
		create_dir_all(parent).await?;
	}

	let tmp = temp_sibling(path);
	let ctx = |e: std::io::Error| IOError::PathIOError {
		source: e,
		path: tmp.to_string_lossy().to_string(),
	};

	let write = async {
		let mut file = tokio::fs::File::create(&tmp).await.map_err(ctx)?;
		tokio::io::AsyncWriteExt::write_all(&mut file, data.as_ref())
			.await
			.map_err(ctx)?;
		file.sync_all().await.map_err(ctx)?;
		Ok::<_, IOError>(())
	}
	.await;

	if let Err(err) = write {
		let _ = tokio::fs::remove_file(&tmp).await;
		return Err(err);
	}

	if let Err(err) = rename(&tmp, path).await {
		let _ = tokio::fs::remove_file(&tmp).await;
		return Err(err);
	}

	Ok(())
}

/// [`write_atomic`] for a serializable value, mirroring [`write_json`].
#[tracing::instrument(
    level = "debug",
    skip(path, data),
    fields(path = %path.as_ref().display())
)]
pub async fn write_json_atomic<T: Serialize>(
	path: impl AsRef<Path>,
	data: T,
) -> PolyIOResult<()> {
	let bytes = serde_json::to_vec(&data).map_err(|err| IOError::JsonFileWrite {
		source: err,
		file: path.as_ref().to_path_buf(),
	})?;

	write_atomic(path, bytes).await
}

/// Canonicalises `path` and returns it only if it lies under one of `roots`.
///
/// This is the guard for paths that arrive from outside (a log or screenshot
/// the UI asked to open) so that `../` cannot walk out of the launcher's own
/// directories. Returns `Ok(None)` when the path resolves outside every root;
/// `Err` only when `path` itself cannot be canonicalised.
///
/// Roots that cannot be canonicalised (typically because they don't exist yet)
/// are skipped rather than treated as a match.
#[tracing::instrument(
    level = "debug",
    skip(path, roots),
    fields(path = %path.as_ref().display())
)]
pub fn ensure_under<R: AsRef<Path>>(
	path: impl AsRef<Path>,
	roots: impl IntoIterator<Item = R>,
) -> PolyIOResult<Option<PathBuf>> {
	// `canonicalize` rather than `std::fs::canonicalize`: on Windows the latter
	// returns a `\\?\` UNC path while the roots are plain paths, so `starts_with`
	// would never match and every path would look like an escape.
	let canon = crate::canonicalize(path)?;

	for root in roots {
		if let Ok(root) = crate::canonicalize(root)
			&& canon.starts_with(&root)
		{
			return Ok(Some(canon));
		}
	}

	Ok(None)
}

/// Recursively copies `src` into `dst`, creating directories as needed.
///
/// Entries named in `exclude_top` are skipped, but only at the top level; a
/// nested directory of the same name is still copied. Symlinks are followed,
/// copied as their target's contents.
#[tracing::instrument(level = "debug", skip(exclude_top))]
pub async fn copy_dir(src: &Path, dst: &Path, exclude_top: &[&str]) -> PolyIOResult<()> {
	let mut stack: Vec<(PathBuf, PathBuf, bool)> =
		vec![(src.to_path_buf(), dst.to_path_buf(), true)];

	while let Some((cur_src, cur_dst, is_top)) = stack.pop() {
		let mut entries = read_dir(&cur_src).await?;
		while let Some(entry) = entries.next_entry().await? {
			let name = entry.file_name();

			if is_top
				&& let Some(name_str) = name.to_str()
				&& exclude_top.iter().any(|e| e.eq_ignore_ascii_case(name_str))
			{
				continue;
			}

			let child_src = entry.path();
			let child_dst = cur_dst.join(&name);
			let file_type = entry.file_type().await?;

			if file_type.is_dir() {
				create_dir_all(&child_dst).await?;

				stack.push((child_src, child_dst, false));
			} else if file_type.is_file() {
				if let Some(parent) = child_dst.parent() {
					create_dir_all(parent).await?;
				}

				copy(&child_src, &child_dst).await?;
			}
		}
	}

	Ok(())
}

/// Whether `dir` is a directory containing at least one entry.
pub async fn dir_has_content(dir: &Path) -> bool {
	if !dir.is_dir() {
		return false;
	}

	match read_dir(dir).await {
		Ok(mut entries) => matches!(entries.next_entry().await, Ok(Some(_))),
		Err(_) => false,
	}
}

/// Renames a file or directory to a new name, replacing the original file if `to` already exists.
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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
#[tracing::instrument(
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

	#[cfg(windows)]
	{
		let path = link.to_string_lossy().to_string();
		let original = original.to_path_buf();
		let link = link.to_path_buf();
		return tokio::task::spawn_blocking(move || junction::create(&original, &link))
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
#[tracing::instrument(
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
#[tracing::instrument(level = "debug")]
pub async fn tempdir() -> PolyIOResult<TempDir> {
	Ok(TempDir::new().await?)
}

/// Creates a temporary file.
#[tracing::instrument(level = "debug")]
pub async fn tempfile() -> PolyIOResult<TempFile> {
	Ok(TempFile::new().await?)
}

/// Sanitises every component of a path and normalises separators to `/`.
#[tracing::instrument(
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
#[tracing::instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn stat(path: impl AsRef<std::path::Path>) -> PolyIOResult<Metadata> {
	tokio::fs::metadata(path).await.map_err(IOError::from)
}
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn write_buffer_fits_the_file() {
		// A typical Minecraft asset object.
		assert_eq!(write_buffer_size(Some(10_217)), MIN_WRITE_BUFFER);
		assert_eq!(write_buffer_size(Some(1)), MIN_WRITE_BUFFER);
		// Mid-sized files get exactly what they need.
		assert_eq!(write_buffer_size(Some(64 * 1024)), 64 * 1024);
		// Large downloads are capped rather than matched.
		assert_eq!(write_buffer_size(Some(25_000_000)), MAX_WRITE_BUFFER);
		assert_eq!(write_buffer_size(Some(u64::MAX)), MAX_WRITE_BUFFER);
	}

	#[test]
	fn write_buffer_falls_back_without_a_length() {
		assert_eq!(write_buffer_size(None), DEFAULT_WRITE_BUFFER);
		assert_eq!(write_buffer_size(Some(0)), DEFAULT_WRITE_BUFFER);
	}

	fn scratch(tag: &str) -> PathBuf {
		static N: AtomicU64 = AtomicU64::new(0);
		let dir = std::env::temp_dir().join(format!(
			"polyio-{tag}-{}-{}",
			std::process::id(),
			N.fetch_add(1, Ordering::Relaxed)
		));
		std::fs::create_dir_all(&dir).unwrap();
		dir
	}

	#[tokio::test]
	async fn write_atomic_creates_missing_parents() {
		let dir = scratch("atomic-parents");
		let target = dir.join("a").join("b").join("settings.json");

		write_atomic(&target, b"{}").await.unwrap();

		assert_eq!(std::fs::read(&target).unwrap(), b"{}");
		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[tokio::test]
	async fn write_atomic_replaces_and_leaves_no_scratch_files() {
		let dir = scratch("atomic-replace");
		let target = dir.join("settings.json");

		write_atomic(&target, b"old").await.unwrap();
		write_atomic(&target, b"new-and-longer").await.unwrap();

		assert_eq!(std::fs::read(&target).unwrap(), b"new-and-longer");

		// The scratch sibling must not survive a successful write.
		let leftovers: Vec<_> = std::fs::read_dir(&dir)
			.unwrap()
			.flatten()
			.map(|e| e.file_name().to_string_lossy().to_string())
			.filter(|n| n != "settings.json")
			.collect();
		assert!(leftovers.is_empty(), "left scratch files behind: {leftovers:?}");

		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[test]
	fn ensure_under_accepts_a_path_inside_a_root() {
		let dir = scratch("under-inside");
		let nested = dir.join("clusters").join("logs");
		std::fs::create_dir_all(&nested).unwrap();
		let file = nested.join("latest.log");
		std::fs::write(&file, b"").unwrap();

		let resolved = ensure_under(&file, [&dir]).unwrap();
		assert!(resolved.is_some());

		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[test]
	fn ensure_under_rejects_a_traversal_out_of_every_root() {
		let dir = scratch("under-escape");
		let root = dir.join("clusters");
		let outside = dir.join("secrets");
		std::fs::create_dir_all(&root).unwrap();
		std::fs::create_dir_all(&outside).unwrap();
		let file = outside.join("auth.json");
		std::fs::write(&file, b"").unwrap();

		// The classic shape: a path that only escapes once resolved.
		let sneaky = root.join("..").join("secrets").join("auth.json");
		assert_eq!(ensure_under(&sneaky, [&root]).unwrap(), None);

		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[test]
	fn ensure_under_skips_roots_that_do_not_exist() {
		let dir = scratch("under-missing-root");
		let file = dir.join("a.log");
		std::fs::write(&file, b"").unwrap();

		let missing = dir.join("not-created-yet");
		let resolved = ensure_under(&file, [&missing, &dir]).unwrap();
		assert!(resolved.is_some(), "a missing root must not shadow a real one");

		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[tokio::test]
	async fn copy_dir_excludes_only_at_the_top_level() {
		let dir = scratch("copy-dir");
		let src = dir.join("src");
		let dst = dir.join("dst");
		std::fs::create_dir_all(src.join("mods")).unwrap();
		std::fs::create_dir_all(src.join("keep").join("mods")).unwrap();
		std::fs::write(src.join("mods").join("top.jar"), b"top").unwrap();
		std::fs::write(src.join("keep").join("mods").join("nested.jar"), b"nested").unwrap();
		std::fs::write(src.join("options.txt"), b"opts").unwrap();

		copy_dir(&src, &dst, &["mods"]).await.unwrap();

		assert!(!dst.join("mods").exists(), "top-level `mods` should be excluded");
		assert!(dst.join("options.txt").exists());
		assert!(
			dst.join("keep").join("mods").join("nested.jar").exists(),
			"exclusion must not apply below the top level"
		);

		std::fs::remove_dir_all(&dir).unwrap();
	}
}
