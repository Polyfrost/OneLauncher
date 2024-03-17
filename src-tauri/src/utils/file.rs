use std::{error::Error, fs::File, path::Path};

pub fn extract_archive(archive: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
    let ext = match archive.extension() {
        Some(ext) => ext.to_str().unwrap(),
        None => return Err("Archive has no extension".into())
    };

    match ext {
        "zip" => extract_zip(archive, dest),
        "gz" => extract_tar_gz(archive, dest),
        _ => Err("Unsupported archive type".into())
    }
}

pub fn extract_zip(archive: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
    let mut archive = zip::ZipArchive::new(File::open(archive)?)?;
    archive.extract(dest)?;
    Ok(())
}

pub fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
    let tar_gz = flate2::read::GzDecoder::new(File::open(archive)?);
    let mut archive = tar::Archive::new(tar_gz);
    archive.unpack(dest)?;
    Ok(())
}