use std::path::PathBuf;

use reqwest::Method;
use serde::Deserialize;

use crate::proxy::send::{init_ingress, send_ingress};
use crate::utils::http::{fetch_advanced, fetch_json};
use crate::utils::io::{self, IOError};
use crate::utils::java::{self, get_java_version, JavaVersion};
use crate::State;

pub async fn filter_java_version(java_version: Option<u32>) -> crate::Result<Vec<JavaVersion>> {
	let java = java::locate_java().await?;
	Ok(if let Some(java_version) = java_version {
		java.into_iter()
			.filter(|j| {
				let jre_version = get_java_version(&j.version);
				if let Ok(jre_version) = jre_version {
					jre_version.1 == java_version
				} else {
					false
				}
			})
			.collect()
	} else {
		java
	})
}

pub async fn install_java(java_version: u32) -> crate::Result<PathBuf> {
	let state = State::get().await?;

	let ingress = init_ingress(
		crate::IngressType::DownloadJava {
			version: java_version,
		},
		100.0,
		"downloading java version",
	)
	.await?;

	#[derive(Deserialize)]
	struct Package {
		pub download_url: String,
		pub name: PathBuf,
	}

	send_ingress(&ingress, 0.0, Some("fetching java api")).await?;
	let packages = fetch_json::<Vec<Package>>(Method::GET, &format!(
        "https://api.azul.com/metadata/v1/zulu/packages?arch={}&java_version={}&os={}&archive_type=zip&javafx_bundled=false&java_package_type=jre&page_size=1",
        std::env::consts::ARCH, java_version, std::env::consts::OS,
    ), None, None, &state.fetch_semaphore).await?;
	send_ingress(&ingress, 10.0, Some("downloading java version")).await?;

	if let Some(download) = packages.first() {
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
				.join(format!("zulu-{}.jre", java_version))
				.join("Contents")
				.join("Home")
				.join("bin")
				.join("java")
		}

		#[cfg(not(target_os = "macos"))]
		{
			base_path = base_path.join("bin").join(java::JAVA_BIN)
		}

		Ok(base_path)
	} else {
		Err(anyhow::anyhow!(
			"no java version found for version {}, os {}, and arch {}",
			java_version,
			std::env::consts::OS,
			std::env::consts::ARCH,
		)
		.into())
	}
}

pub async fn check_java(path: PathBuf) -> crate::Result<Option<JavaVersion>> {
	Ok(java::check_java_instance(&path).await)
}

pub async fn test_java(path: PathBuf, major: u32) -> crate::Result<bool> {
	let jvm = match java::check_java_instance(&path).await {
		Some(jvm) => jvm,
		None => return Ok(false),
	};

	let (maj, _) = get_java_version(&jvm.version)?;
	Ok(maj == major)
}

pub async fn get_max_memory() -> crate::Result<u64> {
	Ok(sys_info::mem_info()
		.map_err(|_| anyhow::anyhow!("failed to get computer memory"))?
		.total)
}
