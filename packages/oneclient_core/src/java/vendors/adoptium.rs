use reqwest::{Method, Request};
use serde::Deserialize;
use url::Url;

use crate::LauncherResult;
use crate::java::PackageArchive;
use crate::java::data::JavaPackage;
use crate::java::vendors::{JavaRuntimeProvider, JavaVendor};
use crate::state::LauncherServices;

pub struct AdoptiumRuntimeProvider;

#[derive(Debug, Deserialize)]
struct AdoptiumRelease {
    version_data: AdoptiumVersionData,
    binaries: Vec<AdoptiumBinary>,
}

#[derive(Debug, Deserialize)]
struct AdoptiumVersionData {
    semver: String,
}

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    name: String,
    link: String,
}

#[async_trait::async_trait]
impl JavaRuntimeProvider for AdoptiumRuntimeProvider {
    fn vendor(&self) -> JavaVendor {
        JavaVendor::Adoptium
    }

    async fn list_packages_by_major(
        &self,
        major: u32,
        services: &LauncherServices,
    ) -> LauncherResult<Vec<JavaPackage>> {
        let url = adoptium_url(major)?;
        let releases = services
            .requester
            .send_as::<Vec<AdoptiumRelease>>(Request::new(Method::GET, url))
            .await?;

        let mut packages = Vec::new();
        
        for release in releases {
            let java_version: Vec<u32> = release
                .version_data
                .semver
                .split(|c: char| !c.is_numeric())
                .filter_map(|s| s.parse::<u32>().ok())
                .collect();

            for binary in release.binaries {
                packages.push(JavaPackage {
                    archive: PackageArchive::Zip,
                    download_url: binary.package.link,
                    java_version: java_version.clone(),
                    name: binary.package.name,
                    vendor: JavaVendor::Adoptium,
                });
            }
        }

        Ok(packages)
    }
}

fn adoptium_url(major: u32) -> LauncherResult<Url> {
    Ok(Url::parse(&format!(
        "https://api.adoptium.net/v3/assets/feature_releases/{major}/ga?os={ADOPTIUM_OS}&architecture={ADOPTIUM_ARCH}&image_type=jre&jvm_impl=hotspot&project=jdk&heap_size=normal&vendor=eclipse&page=0&page_size=5"
    ))?)
}

const ADOPTIUM_ARCH: &str = cfg_select! {
    target_arch = "x86" => "x32",
    target_arch = "x86_64" => "x64",
    target_arch = "arm" => "arm",
    target_arch = "aarch64" => "aarch64",
};

const ADOPTIUM_OS: &str = cfg_select! {
    target_os = "windows" => "windows",
    target_os = "macos" => "mac",       
    target_os = "linux" => cfg_select! {
        target_env = "musl" => "alpine-linux",
        _ => "linux",
    },
};
