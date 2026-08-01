use reqwest::{Method, Request};
use serde::Deserialize;
use url::Url;

use oneclient_net::RequestClient;
use polyio::Checksum;

use crate::data::{JavaPackage, PackageArchive};
use crate::error::JavaResult;
use crate::platform::{HostArch, HostOs, HostTarget};
use crate::vendors::{JavaRuntimeProvider, JavaVendor};

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
    /// Adoptium's `checksum` is SHA-256; the API has no algorithm field because
    /// it has only ever published one.
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

#[async_trait::async_trait]
impl JavaRuntimeProvider for AdoptiumRuntimeProvider {
    fn vendor(&self) -> JavaVendor {
        JavaVendor::Adoptium
    }

    #[tracing::instrument(level = "debug", skip(self, net))]
    async fn list_packages(
        &self,
        major: Option<u32>,
        net: &RequestClient,
    ) -> JavaResult<Vec<JavaPackage>> {
        // fetched from `/v3/info/available_releases`.
        let (low, high) = match major {
            Some(major) => (major, major),
            None => {
                let info = net
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
        let releases = net
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
                let checksum = binary.package.checksum.as_deref().map(Checksum::sha256);

                packages.push(
                    JavaPackage {
                        archive: PackageArchive::from_filename(&binary.package.name),
                        download_url: binary.package.link,
                        java_version: java_version.clone(),
                        name: binary.package.name,
                        vendor: JavaVendor::Adoptium,
                        checksum: None,
                        size: binary.package.size,
                    }
                    .with_checksum(checksum),
                );
            }
        }

        tracing::debug!(count = packages.len(), "listed Adoptium packages");

        Ok(packages)
    }
}

// One request spanning every GA major via a version range (`[low,high]`),
// newest build per major, instead of one request per major.
fn adoptium_url(low: u32, high: u32) -> JavaResult<Url> {
    let range = format!("%5B{low}%2C{}%5D", high + 1);

    let mut url = Url::parse(&format!(
        "https://api.adoptium.net/v3/assets/version/{range}"
    ))?;

    url.query_pairs_mut()
        .append_pair("os", ADOPTIUM_OS)
        .append_pair("architecture", ADOPTIUM_ARCH)
        // Kits only: a JRE image is missing tooling the game and its mod
        // loaders expect, so the launcher never installs one.
        .append_pair("image_type", "jdk")
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

/// Adoptium alone spells 32-bit x86 `x32`.
const ADOPTIUM_ARCH: &str = match HostTarget::CURRENT.arch {
    HostArch::X86 => "x32",
    HostArch::X86_64 => "x64",
    HostArch::Arm => "arm",
    HostArch::Aarch64 => "aarch64",
};

/// Adoptium says `mac` where the other vendors say `macos`, and publishes musl
/// builds under `alpine-linux` rather than `linux-musl`.
const ADOPTIUM_OS: &str = match HostTarget::CURRENT.os {
    HostOs::Windows => "windows",
    HostOs::MacOs => "mac",
    HostOs::Linux { musl: true } => "alpine-linux",
    HostOs::Linux { musl: false } => "linux",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_kits_are_ever_requested() {
        let url = adoptium_url(21, 21).expect("a valid url");

        assert!(url.query().is_some_and(|q| q.contains("image_type=jdk")));
        assert!(!url.query().is_some_and(|q| q.contains("image_type=jre")));
    }
}
