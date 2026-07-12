use reqwest::{Method, Request};
use serde::Deserialize;
use url::Url;

use crate::LauncherResult;
use crate::java::PackageArchive;
use crate::java::data::JavaPackage;
use crate::java::vendors::{JavaRuntimeProvider, JavaVendor};
use crate::state::LauncherServices;

pub struct AdoptiumRuntimeProvider;

// `/v3/info/available_releases`
#[derive(Debug, Deserialize)]
struct AdoptiumAvailableReleases {
    available_releases: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct AdoptiumRelease {
    version_data: AdoptiumVersionData,
    binaries: Vec<AdoptiumBinary>,
}

#[derive(Debug, Deserialize)]
struct AdoptiumVersionData {
    semver: String,
    major: u32,
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

    async fn list_packages(
        &self,
        major: Option<u32>,
        services: &LauncherServices,
    ) -> LauncherResult<Vec<JavaPackage>> {
        // fetched from `/v3/info/available_releases`.
        let (low, high) = match major {
            Some(major) => (major, major),
            None => {
                let info = services
                    .requester
                    .send_as::<AdoptiumAvailableReleases>(Request::new(
                        Method::GET,
                        Url::parse("https://api.adoptium.net/v3/info/available_releases")?,
                    ))
                    .await?;
                let low = info.available_releases.iter().copied().min().unwrap_or(8);
                let high = info.available_releases.iter().copied().max().unwrap_or(low);
                (low, high)
            }
        };

        let url = adoptium_url(low, high)?;
        let releases = services
            .requester
            .send_as::<Vec<AdoptiumRelease>>(Request::new(Method::GET, url))
            .await?;

        let mut packages = Vec::new();

        for release in releases {
            let mut java_version: Vec<u32> = release
                .version_data
                .semver
                .split(|c: char| !c.is_numeric())
                .filter_map(|s| s.parse::<u32>().ok())
                .collect();
            if java_version.first() != Some(&release.version_data.major) {
                java_version.insert(0, release.version_data.major);
            }

            for binary in release.binaries {
                packages.push(JavaPackage {
                    archive: PackageArchive::from_filename(&binary.package.name),
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

// One request spanning every GA major via a version range (`[low,high]`),
// newest build per major, instead of one request per major.
fn adoptium_url(low: u32, high: u32) -> LauncherResult<Url> {
    let range = format!("%5B{low}%2C{}%5D", high + 1);

    let mut url = Url::parse(&format!(
        "https://api.adoptium.net/v3/assets/version/{range}"
    ))?;

    url.query_pairs_mut()
        .append_pair("os", ADOPTIUM_OS)
        .append_pair("architecture", ADOPTIUM_ARCH)
        .append_pair("image_type", "jre")
        .append_pair("jvm_impl", "hotspot")
        .append_pair("project", "jdk")
        .append_pair("heap_size", "normal")
        .append_pair("vendor", "eclipse")
        .append_pair("release_type", "ga")
        .append_pair("sort_method", "DATE")
        .append_pair("sort_order", "DESC")
        .append_pair("page", "0")
        .append_pair("page_size", "50");
    Ok(url)
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
