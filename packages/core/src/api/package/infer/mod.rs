//! **OneLauncher Inferral**
//!
//! Infers package metadata beyond just looking up the file hashes.

use std::path::PathBuf;

use async_zip::tokio::read::fs::ZipFileReader;
use serde::Deserialize;

use crate::store::{read_icon, Package};
use crate::utils::http::{FetchSemaphore, IoSemaphore};

pub async fn infer(
	hash: String,
	path: PathBuf,
	cache_dir: PathBuf,
	io_semaphore: &IoSemaphore,
	fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Package> {
	let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
	let zipfr = if let Ok(zipfr) = ZipFileReader::new(path.clone()).await {
		zipfr
	} else {
		return Ok(Package {
			sha512: hash,
			disabled: file_name.ends_with(".disabled"),
			meta: crate::store::PackageMetadata::Unknown,
			file_name,
		});
	};

	let zip_idx = zipfr.file().entries().iter().position(|f| f.filename().as_str().unwrap_or_default() == "META-INF/mods.toml");
	if let Some(idx) = zip_idx {
		let mut filestr = String::new();
		if zipfr
			.reader_with_entry(idx)
			.await?
			.read_to_string_checked(&mut filestr)
			.await
			.is_ok()
		{
			if let Ok(pkg) = toml::from_str::<ForgeModInfo>(&filestr) {
				if let Some(pkg) = pkg.mods.first() {
					let icon = read_icon(
						pkg.logo_file.clone(),
						&cache_dir,
						&path,
						io_semaphore,
					).await?;

					return Ok(Package {
						sha512: hash,
						disabled: file_name.ends_with(".disabled"),
						file_name,
						meta: crate::store::PackageMetadata::Mapped {
							title: Some(pkg.display_name.clone().unwrap_or_else(|| pkg.mod_id.clone())),
							description: pkg.description.clone(),
							authors: pkg.authors.clone().map(|x| vec![x]).unwrap_or_default(),
							version: pkg.version.clone(),
							icon,
							package_type: Some(crate::prelude::PackageType::Mod),
						}
					});
				}
			}
		}
	}

	let zip_idx = zipfr.file().entries().iter().position(|f| f.filename().as_str().unwrap_or_default() == "mcmod.info");
	if let Some(idx) = zip_idx {

	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeModInfo {
	pub mods: Vec<ForgeMod>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeMod {
	mod_id: String,
	version: Option<String>,
	display_name: Option<String>,
	description: Option<String>,
	logo_file: Option<String>,
	authors: Option<String>,
}
