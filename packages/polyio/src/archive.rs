use std::borrow::Cow;
use std::path::Path;

use async_zip::base::read1::ZipOptions;
use async_zip::base::read1::seek::ZipArchiveReader;
use async_zip::spec::constructs::CDR;
use futures_lite::{AsyncBufRead, AsyncSeek};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::{IOError, PolyIOResult};

type FileZipReader = ZipArchiveReader<Compat<tokio::io::BufReader<tokio::fs::File>>>;

async fn open_zip<R: AsyncBufRead + AsyncSeek + Unpin>(
    reader: R,
) -> PolyIOResult<ZipArchiveReader<R>> {
    Ok(ZipArchiveReader::open_with_options(reader, ZipOptions::untrusted()).await?)
}

async fn open_zip_file(zip_path: &Path) -> PolyIOResult<FileZipReader> {
    let file = tokio::fs::File::open(zip_path).await?;
    open_zip(tokio::io::BufReader::new(file).compat()).await
}

fn entry_name(cdr: &CDR) -> Cow<'_, str> {
    String::from_utf8_lossy(cdr.insecure_file_name.as_bytes())
}

fn unix_permissions(cdr: &CDR) -> Option<u32> {
    (cdr.cdrh.v_made_by >> 8 == 3).then_some(cdr.cdrh.exter_attr >> 16)
}

#[tracing::instrument(
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

#[tracing::instrument(
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
    let reader = open_zip(futures_lite::io::Cursor::new(data)).await?;

    extract_entries(
        reader,
        dest_path.as_ref(),
        filter_entries,
        None::<fn(&str) -> String>,
    )
    .await
}

#[tracing::instrument(
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
    let zip_path = zip_path.as_ref().to_path_buf();
    let dest_path = dest_path.as_ref().to_path_buf();

    off_caller_thread(move || async move {
        extract_zip_filtered(
            zip_path,
            dest_path,
            None::<fn(&str) -> bool>,
            None::<fn(&str) -> String>,
        )
        .await
    })
    .await
}

/// Inflating an archive is CPU work sitting between short async writes, and
/// installs are driven from freya's UI-thread executor, so a large extraction
/// run inline stalls rendering for as long as it takes
async fn off_caller_thread<F, Fut>(work: F) -> PolyIOResult<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = PolyIOResult<()>>,
{
    let handle = tokio::runtime::Handle::current();

    tokio::task::spawn_blocking(move || handle.block_on(work()))
        .await
        .map_err(std::io::Error::other)?
}

#[tracing::instrument(
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
    filter_entries: Option<impl Fn(&str) -> bool + Send + Sync>,
    modify_entry_name: Option<impl Fn(&str) -> String>,
) -> PolyIOResult<()> {
    let reader = open_zip_file(zip_path.as_ref()).await?;

    extract_entries(
        reader,
        dest_path.as_ref(),
        filter_entries,
        modify_entry_name,
    )
    .await
}

async fn extract_entries<R: AsyncBufRead + AsyncSeek + Unpin>(
    mut reader: ZipArchiveReader<R>,
    dest_path: &Path,
    filter_entries: Option<impl Fn(&str) -> bool>,
    modify_entry_name: Option<impl Fn(&str) -> String>,
) -> PolyIOResult<()> {
    let inner = reader.inner().clone();

    for (index, cdr) in inner.cdrs().iter().enumerate() {
        let old_name = entry_name(cdr);

        if let Some(filter) = &filter_entries
            && !filter(&old_name)
        {
            continue;
        }

        let name: String = modify_entry_name
            .as_ref()
            .map_or_else(|| old_name.to_string(), |modify| modify(&old_name));

        let path = dest_path.join(crate::sanitize_path(name));

        if old_name.ends_with('/') {
            crate::create_dir_all(&path).await?;
        } else {
            if let Some(parent) = path.parent() {
                crate::create_dir_all(parent).await?;
            }

            let file = tokio::fs::File::create(&path).await?;
            let writer = tokio::io::BufWriter::new(file);
            let entry_reader = reader.file(index).await?;

            futures_lite::io::copy(entry_reader, &mut writer.compat_write()).await?;

            #[cfg(unix)]
            if let Some(mode) = unix_permissions(cdr).filter(|mode| mode & 0o777 != 0) {
                use std::os::unix::fs::PermissionsExt;

                let mode = (mode & 0o777) | 0o600;
                tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).await?;
            }
        }
    }

    Ok(())
}

/// Ceiling on a single entry read by [`read_zip_file_entries`] mod manifests,
/// icons and bundle overrides sit far below it a decompression bomb does not
pub const MAX_ZIP_ENTRY_BYTES: u64 = 16 * 1024 * 1024;

/// Each matching entry is read fully into memory so this is for small files
/// not large archives
///
/// Entries claiming or producing more than [`MAX_ZIP_ENTRY_BYTES`] are skipped
#[tracing::instrument(
    level = "debug",
    skip(zip_path, filter),
    fields(zip_path = %zip_path.as_ref().display())
)]
pub async fn read_zip_file_entries(
    zip_path: impl AsRef<std::path::Path>,
    filter: impl Fn(&str) -> bool,
) -> PolyIOResult<Vec<(String, Vec<u8>)>> {
    let zip_path = zip_path.as_ref();
    let mut reader = open_zip_file(zip_path).await?;
    let inner = reader.inner().clone();

    let mut out = Vec::new();
    for (index, cdr) in inner.cdrs().iter().enumerate() {
        let Some(name) = cdr.insecure_file_name.as_str() else {
            continue;
        };
        let name = name.to_string();

        if name.ends_with('/') || !filter(&name) {
            continue;
        }

        let uncompressed_size = cdr.uncompressed_size()?;
        if uncompressed_size > MAX_ZIP_ENTRY_BYTES {
            tracing::warn!(
                "skipping '{name}' in {}: declares {uncompressed_size} bytes over the {MAX_ZIP_ENTRY_BYTES} byte entry cap",
                zip_path.display(),
            );
            continue;
        }

        let entry_reader = reader.file(index).await?;
        let mut data = Vec::new();
        futures_lite::AsyncReadExt::read_to_end(
            &mut futures_lite::AsyncReadExt::take(entry_reader, MAX_ZIP_ENTRY_BYTES + 1),
            &mut data,
        )
        .await?;

        if data.len() as u64 > MAX_ZIP_ENTRY_BYTES {
            tracing::warn!(
                "skipping '{name}' in {}: decompressed past the {MAX_ZIP_ENTRY_BYTES} byte entry cap",
                zip_path.display()
            );
            continue;
        }

        out.push((name, data));
    }

    Ok(out)
}

pub const MAX_STREAMED_ENTRY_BYTES: u64 = 1024 * 1024 * 1024;

pub struct ZipEntryCursor {
    reader: FileZipReader,
    indices: std::collections::HashMap<String, usize>,
}

impl ZipEntryCursor {
    #[tracing::instrument(level = "debug", skip(zip_path, filter), fields(zip_path = %zip_path.as_ref().display()))]
    pub async fn open(
        zip_path: impl AsRef<std::path::Path>,
        filter: impl Fn(&str) -> bool,
    ) -> PolyIOResult<Self> {
        let reader = open_zip_file(zip_path.as_ref()).await?;

        let mut indices = std::collections::HashMap::new();
        for (index, cdr) in reader.cdrs().iter().enumerate() {
            let Some(name) = cdr.insecure_file_name.as_str() else {
                continue;
            };
            if name.ends_with('/') || !filter(name) {
                continue;
            }
            indices.insert(name.to_string(), index);
        }

        Ok(Self { reader, indices })
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.indices.keys()
    }

    pub async fn read(&mut self, name: &str) -> PolyIOResult<Vec<u8>> {
        let index = *self
            .indices
            .get(name)
            .ok_or_else(|| IOError::FileNotFoundInZip {
                file_name: name.to_string(),
            })?;

        let declared = self.reader.cdrs()[index].uncompressed_size()?;
        if declared > MAX_STREAMED_ENTRY_BYTES {
            return Err(IOError::IOError(std::io::Error::other(format!(
                "'{name}' declares {declared} bytes, over the {MAX_STREAMED_ENTRY_BYTES} byte limit"
            ))));
        }

        let entry_reader = self.reader.file(index).await?;
        let mut data = Vec::with_capacity(usize::try_from(declared).unwrap_or_default());
        futures_lite::AsyncReadExt::read_to_end(
            &mut futures_lite::AsyncReadExt::take(entry_reader, MAX_STREAMED_ENTRY_BYTES + 1),
            &mut data,
        )
        .await?;

        if data.len() as u64 > MAX_STREAMED_ENTRY_BYTES {
            return Err(IOError::IOError(std::io::Error::other(format!(
                "'{name}' decompressed past the {MAX_STREAMED_ENTRY_BYTES} byte limit"
            ))));
        }

        Ok(data)
    }
}

#[tracing::instrument(
    level = "debug",
    skip(zip_path),
    fields(zip_path = %zip_path.as_ref().display())
)]
pub async fn zip_entry_names(zip_path: impl AsRef<std::path::Path>) -> PolyIOResult<Vec<String>> {
    let reader = open_zip_file(zip_path.as_ref()).await?;

    Ok(reader
        .cdrs()
        .iter()
        .filter_map(|cdr| cdr.insecure_file_name.as_str().map(ToString::to_string))
        .collect())
}

/// Returns a zip file entry's bytes without reading the entire file into memory
#[tracing::instrument(level = "debug", skip(reader))]
pub async fn try_read_zip_entry_bytes<R>(reader: R, file_name: &str) -> PolyIOResult<Vec<u8>>
where
    R: tokio::io::AsyncRead + tokio::io::AsyncBufRead + tokio::io::AsyncSeek + Unpin,
{
    let mut zip_reader = open_zip(reader.compat()).await?;

    let index = zip_reader
        .find(file_name.as_bytes())?
        .next()
        .ok_or_else(|| IOError::FileNotFoundInZip {
            file_name: file_name.to_string(),
        })?;

    let mut entry_reader = zip_reader.file(index).await?;

    let mut data: Vec<u8> = Vec::new();

    futures_lite::AsyncReadExt::read_to_end(&mut entry_reader, &mut data).await?;

    Ok(data)
}

#[tracing::instrument(
    level = "debug",
    skip(archive, dest),
    fields(
        archive = %archive.as_ref().display(),
        dest = %dest.as_ref().display(),
    )
)]
pub async fn extract_tar_gz(
    archive: impl AsRef<std::path::Path>,
    dest: impl AsRef<std::path::Path>,
) -> PolyIOResult<()> {
    let archive = archive.as_ref().to_path_buf();
    let dest = dest.as_ref().to_path_buf();

    off_caller_thread(move || async move {
        crate::create_dir_all(&dest).await?;

        let file = tokio::fs::File::open(archive).await?;
        let buf_reader = tokio::io::BufReader::new(file);
        let gzip_decoder = async_compression::tokio::bufread::GzipDecoder::new(buf_reader);

        let mut tar_archive = tokio_tar::ArchiveBuilder::new(gzip_decoder)
            .set_preserve_permissions(true)
            .build();
        tar_archive.unpack(dest).await?;

        Ok(())
    })
    .await
}
