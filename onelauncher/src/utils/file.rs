use anyhow::anyhow;
use sha1::{Digest, Sha1};
use std::{
	fs::File,
	io::BufReader,
	path::PathBuf,
};

use crate::ErrorKind;

pub fn file_sha1(file: &PathBuf) -> crate::Result<String> {
	let mut file = File::open(file)?;
	let mut hasher = Sha1::new();
    let mut reader = BufReader::new(&mut file);
    std::io::copy(&mut reader, &mut hasher)?;
    let hash = hasher.finalize();

	Ok(format!("{:x}", hash))
}

pub fn extract_archive(archive: &PathBuf, dest: &PathBuf) -> crate::Result<()> {
	let ext = match archive.extension() {
		Some(ext) => ext.to_str().unwrap(),
		None => return Err(ErrorKind::AnyhowError(anyhow!("unsupported operating system")).into()),
	};

	match ext {
		"zip" => extract_zip(archive, dest),
		"gz" => extract_tar_gz(archive, dest),
		_ => Err(ErrorKind::AnyhowError(anyhow!("unsupported file extension {:?}", ext)).into()),
	}
}

pub fn extract_zip(archive: &PathBuf, dest: &PathBuf) -> crate::Result<()> {
	let file = File::open(archive).map_err(|err| ErrorKind::IOError(err))?;
	let archive = zip::ZipArchive::new(file).map_err(|err| ErrorKind::ZipError(err));
	archive?
		.extract(dest)
		.map_err(|err| ErrorKind::ZipError(err))?;
	Ok(())
}

pub fn extract_tar_gz(archive: &PathBuf, dest: &PathBuf) -> crate::Result<()> {
	let file = File::open(archive).map_err(|err| ErrorKind::IOError(err))?;
	let tar_gz = flate2::read::GzDecoder::new(file);
	let mut archive = tar::Archive::new(tar_gz);
	archive
		.unpack(dest)
		.map_err(|err| ErrorKind::IOError(err))?;
	Ok(())
}
