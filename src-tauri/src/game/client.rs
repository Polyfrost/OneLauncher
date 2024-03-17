use std::{env, error::Error, fmt::Display, fs::{self, File}, path::PathBuf};

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_http::reqwest::Client;

use crate::utils::http;

use super::{manifest::Manifest, JavaVersion};

#[allow(async_fn_in_trait)]
pub trait GameClient {
    fn from_manifest(manifest: Manifest) -> Self;

    fn get_handle(&self) -> &AppHandle;
    fn get_manifest(&self) -> &Manifest;
    async fn launch(&self) -> Result<(), Box<dyn Error>>;
    async fn install(&self) -> Result<(), Box<dyn Error>>;
    async fn download_java(&self) -> Result<PathBuf, JavaDownloadError> {
        download_java(self.get_handle(), self.get_manifest().java_version.to_owned()).await
    }
}

pub async fn download_java<R: Runtime>(handle: &AppHandle<R>, java_version: JavaVersion) -> Result<PathBuf, JavaDownloadError> {
    let config_dir = match handle.path().app_config_dir() {
        Ok(dir) => dir,
        Err(_) => return Err(JavaDownloadError::UnsupportedOS)
    };

    let java_dir = config_dir.join("java");
    
    if !java_dir.exists() {
        match fs::create_dir_all(&java_dir) {
            Ok(_) => (),
            Err(_) => return Err(JavaDownloadError::PermissionDenied)
        };
    }
    
    // let java_version = &self.get_manifest().java_version;
    let os = env::consts::OS;
    let archive_type = match os {
        "windows" => "zip",
        "macos" => "tar.gz",
        "linux" => "tar.gz",
        _ => return Err(JavaDownloadError::UnsupportedOS)
    };

    let url = format!("
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

    let response = match http::create_client().get(&url).send().await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(json) => json,
            Err(_) => return Err(JavaDownloadError::NoJavaVersionFound)
        },
        Err(_) => return Err(JavaDownloadError::NoJavaVersionFound)
    };

    if !response.is_array() || response.as_array().unwrap().is_empty() {
        return Err(JavaDownloadError::NoJavaVersionFound);
    }

    let latest = response.as_array().unwrap().first().unwrap();
    let download_url = latest.get("download_url").unwrap().as_str().unwrap();
    let archive_name = format!("zulu-{}.{}", java_version.to_string(), archive_type);
    let tmp_archive = java_dir.join(&archive_name);

    if let Err(err) = http::download_file(download_url, tmp_archive.as_path()).await {
        eprintln!("{}", err);
        return Err(JavaDownloadError::DownloadError);
    };

    // TODO: Extract the archive and delete the archive after a SUCCESSFUL extraction.
    // Name the folder containing the JRE `zulu-{version}-{arch}`

    Ok(tmp_archive)
}

pub fn get_arch() -> String {
    let arch = env::consts::ARCH;
    match arch {
        "x86" => "x86",
        "x86_64" => "x64",
        "arm" => "aarch32",
        "aarch64" => "aarch64",
        _ => "unsupported"
    }.to_string()
}

#[derive(Debug)]
pub enum JavaDownloadError {
    NoJavaVersionFound,
    PermissionDenied,
    UnsupportedOS,
    UnsupportedArch,
    DownloadError,
}