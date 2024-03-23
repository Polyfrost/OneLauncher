use std::{fs::File, path::Path};
use anyhow::anyhow;
use thiserror::Error;

use crate::{PolyError, PolyResult};

pub fn extract_archive(archive: &Path, dest: &Path) -> PolyResult<()> {
    let ext = match archive.extension() {
        Some(ext) => ext.to_str().unwrap(),
        None => return Err(anyhow!("unsupported archive type"))
    };

    match ext {
        "zip" => extract_zip(archive, dest),
        "gz" => extract_tar_gz(archive, dest),
        _ => Err(PolyError::FileError(FileError::UnsupportedArch))
    }
}

pub fn extract_zip(archive: &Path, dest: &Path) -> PolyResult<()> {
    let mut archive = zip::ZipArchive::new(File::open(archive)?)
        .map_err(|err| PolyError::FileError(FileError::ZipError(err)));
    archive?.extract(dest).map_err(|err| PolyError::FileError(FileError::ZipError(err)));
    Ok(())
}

pub fn extract_tar_gz(archive: &Path, dest: &Path) -> PolyResult<()> {
    let tar_gz = flate2::read::GzDecoder::new(File::open(archive)?);
    let mut archive = tar::Archive::new(tar_gz);
    archive.unpack(dest)?;
    Ok(())
}
