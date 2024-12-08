use std::path::PathBuf;

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::proxy::send::{init_ingress, send_ingress};
use crate::utils::http::{fetch_advanced, fetch_json};
use crate::utils::java::{self, get_java_version, JavaVersion};
use crate::State;
use onelauncher_utils::io::{self, IOError};

pub async fn filter_java_version(java_version: Option<u32>) -> crate::Result<Vec<JavaVersion>> {
	let java = java::locate_java().await?;
	Ok(if let Some(java_version) = java_version {
		java.into_iter()
			.filter(|j| {
				let jre_version = get_java_version(&j.version);
				jre_version.map_or(false, |jre_version| jre_version.1 == java_version)
			})
			.collect()
	} else {
		java
	})
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Serialize, Deserialize)]
pub struct JavaZuluPackage {
	pub download_url: String,
	pub name: PathBuf,
	pub java_version: Vec<u32>,
}

pub async fn get_zulu_packages() -> crate::Result<Vec<JavaZuluPackage>> {
	let state = State::get().await?;
	fetch_json::<Vec<JavaZuluPackage>>(
		Method::GET,
		format!(
			"https://api.azul.com/metadata/v1/zulu/packages/?os={}&arch={}&archive_type=zip&java_package_type=jre&javafx_bundled=false&latest=true&release_status=ga&availability_types=CA&certifications=tck&page=1&page_size=100",
			std::env::consts::OS,
			std::env::consts::ARCH
		).as_str(),
		None,
		None,
		&state.fetch_semaphore,
	)
	.await
}

// TODO: support more than just zulu ?
#[onelauncher_macros::memory]
pub async fn install_java_from_major(java_version: u32) -> crate::Result<PathBuf> {
	let packages = get_zulu_packages().await?;
	let package = packages
		.into_iter()
		.find(|p| p.java_version.contains(&java_version))
		.ok_or(anyhow::anyhow!(
			"Could not find a java package for version {}",
			java_version
		))?;

	install_java_from_package(package).await
}

#[onelauncher_macros::memory]
pub async fn install_java_from_package(download: JavaZuluPackage) -> crate::Result<PathBuf> {
	let state = State::get().await?;
	let java_version = *download.java_version.get(0).unwrap_or(&0);

	let ingress = init_ingress(
		crate::IngressType::DownloadJava {
			version: java_version,
		},
		100.0,
		"downloading java version",
	)
	.await?;

	send_ingress(&ingress, 0.0, Some("downloading java version")).await?;

	let file = fetch_advanced(
		Method::GET,
		&download.download_url,
		None,
		None,
		None,
		Some((&ingress, 80.0)),
		&state.fetch_semaphore,
	)
	.await?;

	let path = state.directories.java_dir().await;
	let mut archive =
		zip::ZipArchive::new(std::io::Cursor::new(file)).map_err(IOError::from_zip)?;

	if let Some(file) = archive.file_names().next() {
		if let Some(dir) = file.split('/').next() {
			let path = path.join(dir);
			if path.exists() {
				io::remove_dir_all(path).await?;
			}
		}
	}

	send_ingress(&ingress, 0.0, Some("extracing java binary")).await?;
	archive.extract(&path).map_err(IOError::from_zip)?;
	send_ingress(&ingress, 10.0, Some("extracted java binary")).await?;
	let mut base_path = path.join(
		download
			.name
			.file_stem()
			.unwrap_or_default()
			.to_string_lossy()
			.to_string(),
	);

	#[cfg(target_os = "macos")]
	{
		base_path = base_path
			.join(format!("zulu-{java_version}.jre"))
			.join("Contents")
			.join("Home")
			.join("bin")
			.join("java");
	}

	#[cfg(not(target_os = "macos"))]
	{
		base_path = base_path.join("bin").join(crate::constants::JAVA_BIN);
	}

	send_ingress(&ingress, 100.0, Some("installed java binary")).await?;

	Ok(base_path)
}

pub async fn check_java(path: PathBuf) -> crate::Result<Option<JavaVersion>> {
	Ok(java::check_java_instance(&path).await)
}

pub async fn test_java(path: PathBuf, major: u32) -> crate::Result<bool> {
	let Some(jvm) = java::check_java_instance(&path).await else {
		return Ok(false);
	};

	let (maj, _) = get_java_version(&jvm.version)?;
	Ok(maj == major)
}