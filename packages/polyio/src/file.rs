use std::sync::atomic::{AtomicU64, Ordering};
use std::{fs::Metadata, path::{Path, PathBuf}};

use async_tempfile::{TempDir, TempFile};
use serde::{Serialize, de::DeserializeOwned};

use crate::{IOError, PolyIOResult};

/// HTTP chunks already arrive at ~16 KiB
const MIN_WRITE_BUFFER: usize = 16 * 1024;
const MAX_WRITE_BUFFER: usize = 256 * 1024;
/// Used when the response has no length
const DEFAULT_WRITE_BUFFER: usize = 64 * 1024;

/// Sizing the buffer to the file is a memory-footprint choice not a throughput
/// one a fixed 256 KiB costs ~8 MiB across 32 concurrent small-asset downloads
fn write_buffer_size(size_hint: Option<u64>) -> usize {
	match size_hint {
		Some(0) | None => DEFAULT_WRITE_BUFFER,
		Some(size) => (size.min(MAX_WRITE_BUFFER as u64) as usize).max(MIN_WRITE_BUFFER),
	}
}

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

/// Fails if the directory is non-empty making "remove if empty" atomic with no
/// read-then-delete window
#[tracing::instrument(
    level = "debug",
    skip(path),
    fields(path = %path.as_ref().display())
)]
pub async fn remove_dir(path: impl AsRef<std::path::Path>) -> PolyIOResult<()> {
	let path = path.as_ref();

	tokio::fs::remove_dir(path)
		.await
		.map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})
}

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

/// Streams into a scratch sibling renamed over `path` on success so `path`
/// never holds a truncated file and an existing good file survives a failure
///
/// Unlike [`write_atomic`] this deliberately does not fsync too expensive
/// across thousands of asset downloads and nothing here must survive power loss
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
    let tmp = temp_sibling(path);

    let write = async {
        let file = tokio::fs::File::create(&tmp).await.map_err(IOError::from)?;
        let mut writer = tokio::io::BufWriter::with_capacity(write_buffer_size(size_hint), file);

        while let Some(chunk_result) = futures_lite::StreamExt::next(&mut stream).await {
            let chunk = chunk_result?;
            tokio::io::AsyncWriteExt::write_all(&mut writer, &chunk)
                .await
                .map_err(IOError::from)?;
        }

        tokio::io::AsyncWriteExt::flush(&mut writer)
            .await
            .map_err(IOError::from)?;

        Ok::<_, E>(())
    }
    .await;

    if let Err(err) = write {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(err);
    }

    if let Err(err) = rename(&tmp, path).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(E::from(err));
    }

    Ok(())
}

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

/// Keeps two concurrent atomic writes to the same path off the same scratch file
static ATOMIC_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Sibling rather than temp dir `rename` across mount points fails with
/// `EXDEV` and `/tmp` is often a separate tmpfs on Linux
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

/// Readers see either the old contents or the complete new ones
/// The fsync before the rename stops a crash leaving a correctly-named zero-length file
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

/// Traversal guard for externally supplied paths
/// `Ok(None)` when the path resolves outside every root
/// `Err` only when `path` cannot be canonicalised
/// Roots that cannot be canonicalised (e.g. not yet created) are skipped
#[tracing::instrument(
    level = "debug",
    skip(path, roots),
    fields(path = %path.as_ref().display())
)]
pub fn ensure_under<R: AsRef<Path>>(
	path: impl AsRef<Path>,
	roots: impl IntoIterator<Item = R>,
) -> PolyIOResult<Option<PathBuf>> {
	// Not `std::fs::canonicalize` its Windows `\\?\` UNC output would never
	// `starts_with` the plain-path roots so everything would look like an escape
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

/// `exclude_top` applies only at the top level a nested directory of the same
/// name is still copied
/// Symlinks are followed and copied as their contents
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

pub async fn dir_has_content(dir: &Path) -> bool {
	if !dir.is_dir() {
		return false;
	}

	match read_dir(dir).await {
		Ok(mut entries) => matches!(entries.next_entry().await, Ok(Some(_))),
		Err(_) => false,
	}
}

/// Replaces `to` if it already exists
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

/// Also copies permission bits and overwrites `to`
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

/// Unlike [`stat`] returns metadata about the link itself not its target
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

/// Symlink on Unix hard link on Windows so both paths must be on one volume
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

/// Windows gets a junction which needs no elevated privilege unlike a real
/// directory symlink
/// Remove with [`remove_symlink_dir`]
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

/// A Windows junction must be removed with `remove_dir` not `remove_file`
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

#[tracing::instrument(level = "debug")]
pub async fn tempdir() -> PolyIOResult<TempDir> {
	Ok(TempDir::new().await?)
}

#[tracing::instrument(level = "debug")]
pub async fn tempfile() -> PolyIOResult<TempFile> {
	Ok(TempFile::new().await?)
}

/// Sanitises every component and normalises separators to `/`
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
		assert_eq!(write_buffer_size(Some(10_217)), MIN_WRITE_BUFFER);
		assert_eq!(write_buffer_size(Some(1)), MIN_WRITE_BUFFER);
		assert_eq!(write_buffer_size(Some(64 * 1024)), 64 * 1024);
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

		let leftovers: Vec<_> = std::fs::read_dir(&dir)
			.unwrap()
			.flatten()
			.map(|e| e.file_name().to_string_lossy().to_string())
			.filter(|n| n != "settings.json")
			.collect();
		assert!(leftovers.is_empty(), "left scratch files behind: {leftovers:?}");

		std::fs::remove_dir_all(&dir).unwrap();
	}

	fn failing_stream(
		chunks: Vec<&'static [u8]>,
	) -> impl futures_lite::Stream<Item = Result<bytes::Bytes, IOError>> + Unpin + Send {
		let items = chunks
			.into_iter()
			.map(|c| Ok(bytes::Bytes::from_static(c)))
			.chain(std::iter::once(Err(IOError::IOError(
				std::io::Error::from(std::io::ErrorKind::ConnectionReset),
			))));

		Box::pin(futures_lite::stream::iter(items))
	}

	#[tokio::test]
	async fn write_stream_publishes_only_a_complete_file() {
		let dir = scratch("stream-ok");
		let target = dir.join("object.bin");

		let chunks = vec![
			Ok(bytes::Bytes::from_static(b"hello ")),
			Ok(bytes::Bytes::from_static(b"world")),
		];
		let stream = Box::pin(futures_lite::stream::iter(chunks));

		write_stream::<_, IOError>(&target, stream, Some(11))
			.await
			.unwrap();

		assert_eq!(std::fs::read(&target).unwrap(), b"hello world");

		let leftovers: Vec<_> = std::fs::read_dir(&dir)
			.unwrap()
			.flatten()
			.map(|e| e.file_name().to_string_lossy().to_string())
			.filter(|n| n != "object.bin")
			.collect();
		assert!(leftovers.is_empty(), "left scratch files behind: {leftovers:?}");

		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[tokio::test]
	async fn write_stream_leaves_nothing_when_the_stream_dies() {
		let dir = scratch("stream-drop");
		let target = dir.join("object.bin");

		write_stream::<_, IOError>(&target, failing_stream(vec![b"partial"]), Some(64))
			.await
			.expect_err("a dropped stream must fail");

		assert!(!target.exists(), "left a truncated file at the destination");

		let leftovers: Vec<_> = std::fs::read_dir(&dir)
			.unwrap()
			.flatten()
			.map(|e| e.file_name().to_string_lossy().to_string())
			.collect();
		assert!(leftovers.is_empty(), "left scratch files behind: {leftovers:?}");

		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[tokio::test]
	async fn write_stream_keeps_the_old_file_when_a_rewrite_fails() {
		let dir = scratch("stream-keep");
		let target = dir.join("object.bin");
		std::fs::write(&target, b"known-good").unwrap();

		write_stream::<_, IOError>(&target, failing_stream(vec![b"junk"]), Some(64))
			.await
			.expect_err("a dropped stream must fail");

		assert_eq!(std::fs::read(&target).unwrap(), b"known-good");

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
