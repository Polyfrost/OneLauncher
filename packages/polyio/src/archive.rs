use std::sync::Arc;

use async_stream::try_stream;
use async_zip::StoredZipEntry;
use async_zip::base::read::WithoutEntry;
use futures_util::{Stream, TryStreamExt, pin_mut};
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tracing::instrument;

use crate::{IOError, PolyIOResult};

/// Reads a zip archive from a byte array
#[instrument(skip(data, f), level = "debug")]
pub async fn read_zip_entries_bytes<F>(data: Vec<u8>, mut f: F) -> PolyIOResult<()>
where
	F: AsyncFnMut(
		usize,
		&StoredZipEntry,
		&mut async_zip::base::read::ZipEntryReader<
			'_,
			futures_lite::io::Cursor<&[u8]>,
			WithoutEntry,
		>,
	) -> PolyIOResult<()>,
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

/// Reads a zip archive from a byte array and returns a stream of entries.
#[instrument(skip(data), level = "debug")]
pub fn stream_zip_entries_bytes(
	data: Vec<u8>,
) -> impl Stream<
	Item = Result<
		(
			usize,
			StoredZipEntry,
			Arc<async_zip::base::read::mem::ZipFileReader>,
		),
		IOError,
	>,
> {
	try_stream! {
		let reader = Arc::new(async_zip::base::read::mem::ZipFileReader::new(data).await?);
		let entries = reader.file().entries();

		for index in 0..entries.len() {
			let entry = entries.get(index).expect("expected more zip entries");

			yield (index, entry.clone(), reader.clone());
		}
	}
}

/// Unzips a zip archive from a byte array
#[instrument(
    level = "debug",
    skip(data, dest_path),
    fields(
        dest_path = %dest_path.as_ref().display()
    )
)]
pub async fn unzip_bytes(
	data: Vec<u8>,
	dest_path: impl AsRef<std::path::Path>,
) -> PolyIOResult<()> {
	unzip_bytes_filtered(data, None::<fn(&str) -> bool>, dest_path).await
}

/// Unzips a zip archive from a byte array
#[instrument(
    level = "debug",
    skip(data, filter_entries, dest_path),
    fields(
        dest_path = %dest_path.as_ref().display()
    )
)]
pub async fn unzip_bytes_filtered(
	data: Vec<u8>,
	filter_entries: Option<impl Fn(&str) -> bool + Send + Sync>,
	dest_path: impl AsRef<std::path::Path>,
) -> PolyIOResult<()> {
	let stream = stream_zip_entries_bytes(data);
	pin_mut!(stream);

	while let Some((index, entry, reader)) = stream.try_next().await? {
		let file_name = entry.filename().as_str()?;

		if let Some(filter) = &filter_entries
			&& !filter(file_name)
		{
			continue;
		}

		let path = dest_path.as_ref().join(crate::sanitize_path(file_name));

		let is_dir = entry.dir()?;

		if is_dir {
			crate::create_dir_all(path).await?;
		} else {
			if let Some(parent) = path.parent() {
				crate::create_dir_all(parent).await?;
			}

			let entry_reader = reader.reader_without_entry(index).await?;

			let file = tokio::fs::File::create(&path).await?;
			let writer = tokio::io::BufWriter::new(file);

			futures_lite::io::copy(entry_reader, &mut writer.compat_write()).await?;
		}
	}

	Ok(())
}

#[instrument(
    level = "debug",
    skip(zip_path, dest_path),
    fields(
        zip_path = %zip_path.as_ref().display(),
        dest_path = %dest_path.as_ref().display()
    )
)]
pub async fn extract_zip(
	zip_path: impl AsRef<std::path::Path>,
	dest_path: impl AsRef<std::path::Path>,
) -> PolyIOResult<()> {
	extract_zip_filtered(
		zip_path,
		dest_path,
		None::<fn(&StoredZipEntry) -> bool>,
		None::<fn(&str) -> String>,
	)
	.await
}

/// Unzips a zip archive from a file
#[instrument(
    level = "debug",
    skip(zip_path, dest_path, filter_entries, modify_entry_name),
    fields(
        zip_path = %zip_path.as_ref().display(),
        dest_path = %dest_path.as_ref().display()
    )
)]
pub async fn extract_zip_filtered(
	zip_path: impl AsRef<std::path::Path>,
	dest_path: impl AsRef<std::path::Path>,
	filter_entries: Option<impl Fn(&StoredZipEntry) -> bool + Send + Sync>,
	modify_entry_name: Option<impl Fn(&str) -> String>,
) -> PolyIOResult<()> {
	let zip_path = zip_path.as_ref();
	let dest_path = dest_path.as_ref();

	let reader = async_zip::tokio::read::fs::ZipFileReader::new(zip_path).await?;
	let entries = reader.file().entries();

	for index in 0..entries.len() {
		let entry = entries.get(index).expect("expected more zip entries");

		if let Some(filter) = &filter_entries
			&& !filter(entry)
		{
			continue;
		}

		let old_name = entry.filename().as_str()?;
		let name: String = modify_entry_name
			.as_ref()
			.map_or_else(|| old_name.to_string(), |modify| modify(old_name));

		let name = crate::sanitize_path(name);

		let path = dest_path.join(name);
		let is_dir = entry.dir()?;

		if is_dir {
			crate::create_dir_all(&path).await?;
		} else {
			if let Some(parent) = path.parent() {
				crate::create_dir_all(parent).await?;
			}

			let file = tokio::fs::File::create(&path).await?;
			let writer = tokio::io::BufWriter::new(file);
			let entry_reader = reader.reader_without_entry(index).await?;

			futures_lite::io::copy(entry_reader, &mut writer.compat_write()).await?;
		}
	}

	Ok(())
}

/// Reads the bytes of every non-directory zip entry whose name passes `filter`.
///
/// Returns `(entry_name, bytes)` pairs. Each matching entry is read fully into
/// memory, so this is meant for small files (e.g. bundle config overrides), not
/// large archives.
#[instrument(
    level = "debug",
    skip(zip_path, filter),
    fields(zip_path = %zip_path.as_ref().display())
)]
pub async fn read_zip_file_entries(
	zip_path: impl AsRef<std::path::Path>,
	filter: impl Fn(&str) -> bool,
) -> PolyIOResult<Vec<(String, Vec<u8>)>> {
	let zip_path = zip_path.as_ref();
	let reader = async_zip::tokio::read::fs::ZipFileReader::new(zip_path).await?;
	let entries = reader.file().entries();

	let mut out = Vec::new();
	for index in 0..entries.len() {
		let entry = entries.get(index).expect("expected more zip entries");
		let Ok(name) = entry.filename().as_str() else {
			continue;
		};
		let name = name.to_string();

		if entry.dir().unwrap_or(false) || !filter(&name) {
			continue;
		}

		let mut entry_reader = reader.reader_without_entry(index).await?;
		let mut data = Vec::new();
		futures_lite::AsyncReadExt::read_to_end(&mut entry_reader, &mut data).await?;
		out.push((name, data));
	}

	Ok(out)
}

/// Returns a zip file entry's bytes without reading the entire file into memory.
#[instrument(
    level = "debug",
    skip(reader)
)]
pub async fn try_read_zip_entry_bytes<R>(reader: R, file_name: &str) -> PolyIOResult<Vec<u8>>
where
	R: tokio::io::AsyncRead + tokio::io::AsyncBufRead + tokio::io::AsyncSeek + Unpin,
{
	use tokio_util::compat::TokioAsyncReadCompatExt;
	let compat = reader.compat();

	let mut zip_reader = async_zip::base::read::seek::ZipFileReader::new(compat).await?;

	let index = zip_reader
		.file()
		.entries()
		.iter()
		.position(|entry| entry.filename().as_str().is_ok_and(|n| n == file_name))
		.ok_or_else(|| IOError::FileNotFoundInZip {
            file_name: file_name.to_string()
        })?;

	let mut entry_reader = zip_reader.reader_without_entry(index).await?;

	let mut data: Vec<u8> = Vec::new();

	futures_lite::AsyncReadExt::read_to_end(&mut entry_reader, &mut data).await?;

	Ok(data)
}

#[instrument(
    level = "debug",
    skip(archive, dest),
    fields(
        archive = %archive.as_ref().display(),
        dest = %dest.as_ref().display(),
    )
)]
pub async fn extract_tar_gz(archive: impl AsRef<std::path::Path>, dest: impl AsRef<std::path::Path>) -> PolyIOResult<()> {
	crate::create_dir_all(&dest).await?;

    let file = tokio::fs::File::open(archive).await?;
    let buf_reader = tokio::io::BufReader::new(file);
    let gzip_decoder = async_compression::tokio::bufread::GzipDecoder::new(buf_reader);

    let mut tar_archive = tokio_tar::Archive::new(gzip_decoder);
    tar_archive.unpack(dest).await?;

    Ok(())
}