use std::{fs, path::PathBuf};

use serde_json::Value;
use thiserror::Error;

use crate::{utils::{file, http}, PolyError, PolyResult};

#[derive(Debug, Error)]
pub enum JavaDownloadError {
	#[error("failed to find a valid java version: {0}")]
	NoJavaVersionFound(String),
	#[error("failed creating a java directory (try running at an elevated permission): {0}")]
	PermissionDenied(String),
	#[error("unable to match your operating system ({0}) with a valid java version")]
	UnsupportedOS(String),
	#[error("unable to match your architecture ({0}) with a valid java version")]
	UnsupportedArch(String),
	#[error("failed to extract the java executable: {0}")]
	ExtractError(String),
	#[error("failed to download the java executable")]
	DownloadError(String),
}

pub async fn download_java(dir: &PathBuf, version: u8) -> PolyResult<PathBuf> {
    if !dir.exists() {
        match fs::create_dir_all(&dir) {
            Ok(_) => (),
            Err(err) => {
                return Err(PolyError::JavaError(JavaDownloadError::PermissionDenied(
                    err.to_string(),
                )))
            }
        };
    }

    let os = std::env::consts::OS;
    let archive_type = match os {
        "windows" => "zip",
        "macos" => "tar.gz",
        "linux" => "tar.gz",
        _ => {
            return Err(PolyError::JavaError(JavaDownloadError::UnsupportedOS(
                os.to_string(),
            )))
        }
    };

    let archive_name = format!("zulu-{}.{}", version.to_string(), archive_type);
    let archive = dir.join(&archive_name);
    let dest = dir.join(format!("zulu-{}-{}", version.to_string(), get_arch()));

    if archive.exists() && dest.exists() {
        let _ = fs::remove_file(archive.as_path());
    } else if archive.exists() && !dest.exists() {
        extract(&archive, &dest)?;
    } else if !archive.exists() && !dest.exists() {
        download(version, os, archive_type, &archive).await?;
        extract(&archive, &dest)?;
    }

    if let Ok(mut files) = fs::read_dir(&dest) {
        let file = match files.nth(0) {
            Some(file) => file.unwrap().path(),
            None => {
                return Err(PolyError::JavaError(JavaDownloadError::NoJavaVersionFound(
                    "unable to get the java executable file".to_string(),
                )))
            }
        };

        return Ok(file.join("bin").join("java"));
    }

    Err(PolyError::JavaError(JavaDownloadError::NoJavaVersionFound(
        "unable to download the java executable file".to_string(),
    )))
}

fn get_arch() -> String {
	let arch = std::env::consts::ARCH;
	match arch {
		"x86" => "x86",
		"x86_64" => "x64",
		"arm" => "aarch32",
		"aarch64" => "aarch64",
		_ => "unsupported",
	}.to_string()
}

async fn download(
	java_version: u8,
	os: &str,
	archive_type: &str,
	archive: &PathBuf,
) -> PolyResult<()> {
	let response = get_java_versions(java_version, os, archive_type).await?;
	let latest = response.as_array().unwrap().first().unwrap();
	let download_url = latest.get("download_url").unwrap().as_str().unwrap();

	if let Err(err) = http::download_file(download_url, archive.as_path()).await {
		return Err(PolyError::JavaError(JavaDownloadError::DownloadError(
			err.to_string(),
		)));
	};

	Ok(())
}

fn extract(archive: &PathBuf, dest: &PathBuf) -> PolyResult<()> {
	if let Err(err) = file::extract_archive(archive.as_path(), dest.as_path()) {
		let _ = fs::remove_file(dest.as_path());
		return Err(PolyError::JavaError(JavaDownloadError::ExtractError(
			err.to_string(),
		)));
	}

	let _ = fs::remove_file(archive.as_path());
	Ok(())
}

async fn get_java_versions(
	java_version: u8,
	os: &str,
	archive_type: &str,
) -> PolyResult<Value> {
	let url = format!(
		"https://api.azul.com/metadata/v1/zulu/packages/?java_version={}&os={}&arch={}&archive_type={}&java_package_type=jre&javafx_bundled=true&release_status=ga&latest=true",
		java_version.to_string(),
		os,
		get_arch(),
		archive_type
	);

	let response = match http::create_client()?.get(&url).send().await {
		Ok(response) => match response.json::<serde_json::Value>().await {
			Ok(json) => json,
			Err(err) => {
				return Err(PolyError::JavaError(JavaDownloadError::NoJavaVersionFound(
					err.to_string(),
				)))
			}
		},
		Err(err) => {
			return Err(PolyError::JavaError(JavaDownloadError::NoJavaVersionFound(
				err.to_string(),
			)))
		}
	};

	if !response.is_array() || response.as_array().unwrap().is_empty() {
		return Err(PolyError::JavaError(JavaDownloadError::NoJavaVersionFound(
			"didn't get the expected java api result".to_string(),
		)));
	}

	Ok(response)
}