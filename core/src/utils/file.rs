use anyhow::anyhow;
use sha1::{Digest, Sha1};
use std::{fs::File, io::BufReader, path::{Path, PathBuf}};

use crate::{PolyError, PolyResult};

pub fn file_sha1(file: &PathBuf) -> PolyResult<String> {
    let file = File::open(file)?;
    let reader = BufReader::new(file);

    let mut hasher = Sha1::new();
    hasher.update(&reader.buffer());
    let hash = hasher.finalize();

    Ok(format!("{:x}", hash))
}

pub fn extract_archive(archive: &Path, dest: &Path) -> PolyResult<()> {
	let ext = match archive.extension() {
		Some(ext) => ext.to_str().unwrap(),
		None => {
			return Err(PolyError::AnyhowError(anyhow!(
				"unsupported operating system"
			)))
		}
	};

	match ext {
		"zip" => extract_zip(archive, dest),
		"gz" => extract_tar_gz(archive, dest),
		_ => Err(PolyError::AnyhowError(anyhow!(
			"unsupported file extension {:?}",
			ext
		))),
	}
}

pub fn extract_zip(archive: &Path, dest: &Path) -> PolyResult<()> {
	let file = File::open(archive).map_err(|err| PolyError::IOError(err))?;
	let archive = zip::ZipArchive::new(file).map_err(|err| PolyError::ZipError(err));
	archive?
		.extract(dest)
		.map_err(|err| PolyError::ZipError(err))?;
	Ok(())
}

pub fn extract_tar_gz(archive: &Path, dest: &Path) -> PolyResult<()> {
	let file = File::open(archive).map_err(|err| PolyError::IOError(err))?;
	let tar_gz = flate2::read::GzDecoder::new(file);
	let mut archive = tar::Archive::new(tar_gz);
	archive.unpack(dest).map_err(|err| PolyError::IOError(err))?;
	Ok(())
}
