//! **OneLauncher Inferral**
//!
//! Infers package metadata beyond just looking up the file hashes.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use async_zip::tokio::read::fs::ZipFileReader;
use serde::Deserialize;

use crate::prelude::PackageType;
use crate::store::{read_icon, Package, PackageMetadata};
use crate::utils::http::{FetchSemaphore, IoSemaphore};

/// A structure representing a package file that hasn't been analyzed.
pub struct UnanalyzedFile {
	path: String,
	file_name: String,
	package_type: PackageType,
	size: u64,
	cache: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
/// A structure representing a `jar/META-INF/mods.toml` Forge metadata file.
pub struct ForgeModsInfo {
	pub mods: Vec<ForgeMods>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
/// A structure representing `jar/META-INF/mods.toml` Forge mod metadata.
pub struct ForgeMods {
	mod_id: String,
	version: Option<String>,
	display_name: Option<String>,
	description: Option<String>,
	logo_file: Option<String>,
	authors: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
/// A structure representing a `jar/mcmod.info` Forge metadata file.
pub struct ForgeMcMod {
	modid: String,
	name: String,
	description: Option<String>,
	version: Option<String>,
	author_list: Option<Vec<String>>,
	logo_file: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
/// A structure representing `jar/fabric.mod.json` Fabric author metadata.
pub enum FabricAuthor {
	String(String),
	Object { name: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
/// A structure representing a `jar/fabric.mod.json` Fabric metadata file.
pub struct FabricMod {
	id: String,
	version: String,
	name: Option<String>,
	description: Option<String>,
	authors: Vec<FabricAuthor>,
	icon: Option<String>,
}

#[derive(Deserialize)]
/// A structure representing `jar/quilt.mod.json` Quilt metadata.
pub struct QuiltMetadata {
	name: Option<String>,
	description: Option<String>,
	contributors: Option<HashMap<String, String>>,
	icon: Option<String>,
}

#[derive(Deserialize)]
/// A structure representing a `jar/quilt.mod.json` Quilt metadata file.
pub struct QuiltMod {
	id: String,
	version: String,
	metadata: Option<QuiltMetadata>
}

#[derive(Deserialize)]
/// A structure representing a `zip/pack.mcmeta` package metadata file.
pub struct Pack {
	description: Option<String>,
}

async fn process<F, M>(
	zipfr: &ZipFileReader,
	disabled: bool,
	file_name: String,
	sha512: String,
	index: usize,
	reader_fn: F,
	meta_fn: M,
) -> crate::Result<Package>
where
	F: Fn(&ZipFileReader, usize) -> Pin<Box<dyn Future<Output = crate::Result<String>>>>,
	M: Fn(String) -> Option<PackageMetadata>,
{
	let filestr = reader_fn(zipfr, index).await?;

	if let Some(meta) = meta_fn(filestr) {
		Ok(Package {
			sha512,
			disabled,
			file_name,
			meta,
		})
	} else {
		Ok(Package {
			sha512,
			disabled,
			file_name,
			meta: PackageMetadata::Unknown,
		})
	}
}

fn forge_metadata(filestr: String) -> Option<PackageMetadata> {
	if let Ok(pkg) = toml::from_str::<ForgeModsInfo>(&filestr) {
		if let Some(pkg) = pkg.mods.first() {
			return Some(PackageMetadata::Mapped {
				title: Some(
					pkg.display_name
						.clone()
						.unwrap_or_else(|| pkg.mod_id.clone()),
				),
				description: pkg.description.clone(),
				authors: pkg.authors.clone().map(|x| vec![x]).unwrap_or_default(),
				version: pkg.version.clone(),
				icon: pkg.logo_file.clone(),
				package_type: Some(PackageType::Mod),
			});
		}
	}

	None
}

pub async fn infer(
	hash: String,
	path: PathBuf,
	cache_dir: PathBuf,
	io_semaphore: &IoSemaphore,
	fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Package> {
	let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
	let disabled = file_name.ends_with(".disabled");
	let zipfr = match ZipFileReader::new(path.clone()).await {
		Ok(zipfr) => zipfr,
		Err(_) => {
			return Ok(Package {
				sha512: hash,
				disabled,
				meta: PackageMetadata::Unknown,
				file_name,
			})
		}
	};

	let zip_idx = zipfr
		.file()
		.entries()
		.iter()
		.position(|f| f.filename().as_str().unwrap_or_default() == "META-INF/mods.toml");
	if let Some(idx) = zip_idx {
		let mut filestr = String::new();
		if zipfr
			.reader_with_entry(idx)
			.await?
			.read_to_string_checked(&mut filestr)
			.await
			.is_ok()
		{
			if let Ok(pkg) = toml::from_str::<ForgeModsInfo>(&filestr) {
				if let Some(pkg) = pkg.mods.first() {
					return Ok(Package {
						sha512: hash,
						disabled,
						file_name,
						meta: PackageMetadata::Mapped {
							title: Some(
								pkg.display_name
									.clone()
									.unwrap_or_else(|| pkg.mod_id.clone()),
							),
							description: pkg.description.clone(),
							authors: pkg.authors.clone().map(|x| vec![x]).unwrap_or_default(),
							version: pkg.version.clone(),
							icon: read_icon(pkg.logo_file.clone(), &cache_dir, &path, io_semaphore)
								.await?,
							package_type: Some(PackageType::Mod),
						},
					});
				}
			}
		}
	}

	let zip_idx = zipfr
		.file()
		.entries()
		.iter()
		.position(|f| f.filename().as_str().unwrap_or_default() == "mcmod.info");
	if let Some(idx) = zip_idx {
		let mut filestr = String::new();
		if zipfr
			.reader_with_entry(idx)
			.await?
			.read_to_string_checked(&mut filestr)
			.await
			.is_ok()
		{
			if let Ok(pkg) = serde_json::from_str::<ForgeMcMod>(&filestr) {
				let icon = read_icon(pkg.logo_file, &cache_dir, &path, io_semaphore).await?;

				return Ok(Package {
					sha512: hash,
					disabled,
					file_name,
					meta: PackageMetadata::Mapped {
						title: Some(if pkg.name.is_empty() {
							pkg.modid
						} else {
							pkg.name
						}),
						description: pkg.description,
						authors: pkg.author_list.unwrap_or_default(),
						version: pkg.version,
						icon,
						package_type: Some(PackageType::Mod),
					},
				});
			}
		}
	}
}
