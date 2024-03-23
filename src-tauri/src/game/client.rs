use std::{collections::HashMap, env, fs, path::PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use thiserror::Error;
use uuid::Uuid;

use crate::{
	utils::{file, http},
	PolyError, PolyResult,
};

use super::{
	clients::vanilla::{vanilla_manifest, VanillaClient, VanillaManifest},
	JavaVersion,
};

#[macro_export]
macro_rules! create_manifest {
    ($manifest_name:ident { $($name:ident: $type:ty),* }) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $manifest_name {
            $(pub $name: $type,)*
        }
    };
}

#[macro_export]
macro_rules! create_client {
    ($client_name:ident { $($name:ident: $type:ty),* }) => {
        #[derive(Debug, Clone)]
        pub struct $client_name {
            handle: tauri::AppHandle,
            details: crate::game::client::GameClientDetails,
            $(pub $name: $type,)*
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "manifest")]
pub enum GameClientType {
	Vanilla(VanillaManifest),
	// Forge(ForgeManifest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameClientDetails {
	pub uuid: Uuid,
	pub name: String,
	pub version: String,
	pub main_class: String,
	pub java_version: JavaVersion,
	pub startup_args: Vec<String>,
	pub client_type: GameClientType,
}

pub fn get_game_client(handle: AppHandle, details: GameClientDetails) -> Box<dyn GameClient> {
	Box::new(match details.client_type {
		GameClientType::Vanilla(_) => VanillaClient::new(handle, details),
		// GameClientType::Forge(_) => Box::new(ForgeClient::new(handle, details)),
	})
}

#[async_trait]
pub trait GameClient: Send + Sync {
	fn new(handle: AppHandle, details: GameClientDetails) -> Self
	where
		Self: Sized;

	async fn get_all_version_urls() -> PolyResult<HashMap<String, String>>
	where
		Self: Sized;
	async fn get_minecraft_manifest(
		version: String,
	) -> PolyResult<vanilla_manifest::MinecraftManifest>
	where
		Self: Sized;

	// async fn get_details_from_version(version: String) -> Result<GameClientDetails, Box<dyn Error>> where Self: Sized;

	fn get_handle(&self) -> &AppHandle;
	fn get_details(&self) -> &GameClientDetails;
	fn get_client(&self) -> &GameClientType {
		&self.get_details().client_type
	}

	async fn launch(&self) -> PolyResult<()>;
	async fn setup(&self) -> PolyResult<()>;

	async fn download_java(&self) -> PolyResult<PathBuf> {
		let config_dir = match self.get_handle().path().app_config_dir() {
			Ok(dir) => dir,
			// SURELY THERES A BETTER WAY TO DO THIS IDK LOLL
			Err(err) => {
				return Err(PolyError::JavaError(JavaDownloadError::UnsupportedOS(
					err.to_string(),
				)))
			}
		};

		let java_dir = config_dir.join("java");

		if !java_dir.exists() {
			match fs::create_dir_all(&java_dir) {
				Ok(_) => (),
				Err(err) => {
					return Err(PolyError::JavaError(JavaDownloadError::PermissionDenied(
						err.to_string(),
					)))
				}
			};
		}

		let java_version = &self.get_details().java_version;
		let os = env::consts::OS;
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

		let archive_name = format!("zulu-{}.{}", java_version.to_string(), archive_type);
		let archive = java_dir.join(&archive_name);
		let dest = java_dir.join(format!("zulu-{}-{}", java_version.to_string(), get_arch()));

		if archive.exists() && dest.exists() {
			let _ = fs::remove_file(archive.as_path());
		} else if archive.exists() && !dest.exists() {
			extract(&archive, &dest)?;
		} else if !archive.exists() && !dest.exists() {
			download(java_version, os, archive_type, &archive).await?;
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
}

#[async_trait]
pub trait ModdedGameClient: GameClient {
	async fn install_mods(&self) -> PolyResult<()>;
}

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

fn get_arch() -> String {
	let arch = env::consts::ARCH;
	match arch {
		"x86" => "x86",
		"x86_64" => "x64",
		"arm" => "aarch32",
		"aarch64" => "aarch64",
		_ => "unsupported",
	}
	.to_string()
}

async fn get_java_versions(
	java_version: &JavaVersion,
	os: &str,
	archive_type: &str,
) -> PolyResult<Value> {
	let url = format!(
		"
        https://api.azul.com/metadata/v1/zulu/packages/\
        ?java_version={}\
        &os={}\
        &arch={}\
        &archive_type={}\
        &java_package_type=jre&javafx_bundled=true&release_status=ga&latest=true",
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

async fn download(
	java_version: &JavaVersion,
	os: &str,
	archive_type: &str,
	archive: &PathBuf,
) -> PolyResult<()> {
	let response = get_java_versions(java_version, os, archive_type).await?;
	let latest = response.as_array().unwrap().first().unwrap();
	let download_url = latest.get("download_url").unwrap().as_str().unwrap();

	if let Err(err) = http::download_file(download_url, archive.as_path()).await {
		// eprintln!("{}", err);
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
