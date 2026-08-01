use reqwest::{Method, Request};
use serde::Deserialize;
use url::Url;

use oneclient_net::RequestClient;
use polyio::Checksum;

use crate::data::{JavaPackage, PackageArchive};
use crate::error::JavaResult;
use crate::platform::{HostArch, HostOs, HostTarget};
use crate::vendors::{JavaRuntimeProvider, JavaVendor};

pub struct LibericaRuntimeProvider;

#[derive(Debug, Deserialize)]
struct LibericaRelease {
    #[serde(rename = "downloadUrl")]
    download_url: String,
    filename: String,
    #[serde(rename = "featureVersion")]
    feature_version: u32,
    #[serde(rename = "updateVersion", default)]
    update_version: u32,
    #[serde(rename = "buildVersion", default)]
    build_version: u32,
    version: String,
    /// Liberica is the one vendor that publishes SHA-1 rather than SHA-256.
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

#[async_trait::async_trait]
impl JavaRuntimeProvider for LibericaRuntimeProvider {
    fn vendor(&self) -> JavaVendor {
        JavaVendor::Liberica
    }

    #[tracing::instrument(level = "debug", skip(self, net))]
    async fn list_packages(
        &self,
        major: Option<u32>,
        net: &RequestClient,
    ) -> JavaResult<Vec<JavaPackage>> {
        let url = liberica_url(major)?;
        let releases = net
            .send_as::<Vec<LibericaRelease>>(Request::new(Method::GET, url))
            .await?;

        let mut releases = releases;
        releases.sort_by_key(|r| {
            std::cmp::Reverse((r.feature_version, r.update_version, r.build_version))
        });

        let packages: Vec<JavaPackage> = releases
            .into_iter()
            .map(|r| {
                let mut java_version: Vec<u32> = r
                    .version
                    .split(|c: char| !c.is_numeric())
                    .filter_map(|s| s.parse::<u32>().ok())
                    .collect();
                if java_version.first() != Some(&r.feature_version) {
                    java_version.insert(0, r.feature_version);
                }

                let checksum = r.sha1.as_deref().map(Checksum::sha1);

                JavaPackage {
                    archive: PackageArchive::from_filename(&r.filename),
                    download_url: r.download_url,
                    java_version,
                    name: r.filename,
                    vendor: JavaVendor::Liberica,
                    checksum: None,
                    size: r.size,
                }
                .with_checksum(checksum)
            })
            .collect();

        tracing::debug!(count = packages.len(), "listed Liberica packages");

        Ok(packages)
    }
}

fn liberica_url(major: Option<u32>) -> JavaResult<Url> {
    let mut url = Url::parse("https://api.bell-sw.com/v1/liberica/releases")?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("os", LIBERICA_OS)
            .append_pair("arch", LIBERICA_ARCH)
            // Kits only, never a bare runtime image.
            .append_pair("bundle-type", "jdk")
            .append_pair("bitness", LIBERICA_BITNESS)
            .append_pair("package-type", LIBERICA_PACKAGE)
            .append_pair("installation-type", "archive")
            .append_pair(
                "fields",
                // `sha1` and `size` are only returned when named here.
                "downloadUrl,filename,featureVersion,updateVersion,buildVersion,version,sha1,size",
            )
            .append_pair("output", "json");
        if let Some(major) = major {
            q.append_pair("version-feature", &major.to_string());
        }
    }
    Ok(url)
}

/// Liberica takes architecture family and bitness as separate parameters, so
/// x86/x86_64 collapse to `x86` and both ARM variants to `arm`.
const LIBERICA_ARCH: &str = match HostTarget::CURRENT.arch {
    HostArch::X86 | HostArch::X86_64 => "x86",
    HostArch::Arm | HostArch::Aarch64 => "arm",
};

const LIBERICA_BITNESS: &str = HostTarget::CURRENT.bitness();

const LIBERICA_OS: &str = match HostTarget::CURRENT.os {
    HostOs::Windows => "windows",
    HostOs::MacOs => "macos",
    HostOs::Linux { musl: true } => "linux-musl",
    HostOs::Linux { musl: false } => "linux",
};

const LIBERICA_PACKAGE: &str = HostTarget::CURRENT.archive_ext();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_kits_are_ever_requested() {
        let url = liberica_url(Some(21)).expect("a valid url");

        assert!(url.query().is_some_and(|q| q.contains("bundle-type=jdk")));
        assert!(!url.query().is_some_and(|q| q.contains("bundle-type=jre")));
    }
}
