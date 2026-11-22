use futures_util::StreamExt;
use reqwest::{Method, Request};
use url::Url;

use oneclient_net::RequestClient;
use polyio::Checksum;

use crate::data::{JavaPackage, PackageArchive};
use crate::error::JavaResult;
use crate::platform::{HostArch, HostOs, HostTarget};
use crate::vendors::{JavaRuntimeProvider, JavaVendor};

pub struct GraalVmRuntimeProvider;

pub(crate) const INSTALL_PREFIX: &str = "graalvm-jdk-";

const GRAALVM_MAJORS: &[u32] = &[25, 21];

const MAJOR_LOOKUP_CONCURRENCY: usize = 4;

#[async_trait::async_trait]
impl JavaRuntimeProvider for GraalVmRuntimeProvider {
    fn vendor(&self) -> JavaVendor {
        JavaVendor::GraalVm
    }

    #[tracing::instrument(level = "debug", skip(self, net))]
    async fn list_packages(
        &self,
        major: Option<u32>,
        net: &RequestClient,
    ) -> JavaResult<Vec<JavaPackage>> {
        let Some(platform) = GRAALVM_PLATFORM else {
            tracing::debug!("Oracle publishes no GraalVM build for this platform. Listing nothing");
            return Ok(Vec::new());
        };

        let wanted: Vec<u32> = GRAALVM_MAJORS
            .iter()
            .copied()
            .filter(|candidate| major.is_none_or(|filter| filter == *candidate))
            .collect();

        let mut packages: Vec<JavaPackage> = futures_util::stream::iter(
            wanted
                .into_iter()
                .map(|major| async move { resolve_package(major, platform, net).await }),
        )
        .buffer_unordered(MAJOR_LOOKUP_CONCURRENCY)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<JavaResult<Vec<Option<JavaPackage>>>>()?
        .into_iter()
        .flatten()
        .collect();

        packages.sort_by_key(|p| std::cmp::Reverse(p.java_version.first().copied().unwrap_or(0)));
        tracing::debug!(count = packages.len(), "listed GraalVM packages");
        Ok(packages)
    }
}

#[tracing::instrument(level = "debug", skip(net))]
async fn resolve_package(
    major: u32,
    platform: &str,
    net: &RequestClient,
) -> JavaResult<Option<JavaPackage>> {
    let filename = format!("{INSTALL_PREFIX}{major}_{platform}_bin.{}", GRAALVM_EXT.0);
    let url = download_url(major, &filename);
    let response = net
        .send(Request::new(
            Method::GET,
            Url::parse(&format!("{url}.sha256"))?,
        ))
        .await?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(oneclient_net::RequestError::from)?;

    if !status.is_success() {
        tracing::debug!(
            major,
            status = status.as_u16(),
            "GraalVM checksum file is unavailable. Skipping the major"
        );
        return Ok(None);
    }

    let Some(hex) = parse_checksum(&body) else {
        tracing::debug!(
            major,
            "GraalVM checksum file is malformed. Skipping the major"
        );
        return Ok(None);
    };

    Ok(Some(
        JavaPackage {
            archive: GRAALVM_EXT.1,
            download_url: url,
            java_version: vec![major],
            name: format!(
                "{INSTALL_PREFIX}{major}_{platform}_bin_{}.{}",
                &hex[..12],
                GRAALVM_EXT.0
            ),
            vendor: JavaVendor::GraalVm,
            checksum: None,
            size: None,
        }
        .with_checksum(Some(Checksum::sha256(hex))),
    ))
}

fn parse_checksum(body: &str) -> Option<&str> {
    let hex = body.split_whitespace().next()?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

fn download_url(major: u32, filename: &str) -> String {
    format!("https://download.oracle.com/graalvm/{major}/latest/{filename}")
}

const GRAALVM_PLATFORM: Option<&str> = match (HostTarget::CURRENT.os, HostTarget::CURRENT.arch) {
    (HostOs::Windows, HostArch::X86_64) => Some("windows-x64"),
    (HostOs::MacOs, HostArch::X86_64) => Some("macos-x64"),
    (HostOs::MacOs, HostArch::Aarch64) => Some("macos-aarch64"),
    (HostOs::Linux { musl: false }, HostArch::X86_64) => Some("linux-x64"),
    (HostOs::Linux { musl: false }, HostArch::Aarch64) => Some("linux-aarch64"),
    _ => None,
};

const GRAALVM_EXT: (&str, PackageArchive) = (
    HostTarget::CURRENT.archive_ext(),
    HostTarget::CURRENT.archive(),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_file_is_parsed() {
        let hex = "a98948c3a1ad037fe2dde5c80b06b214a5f80c120d8fc2453d155508cf677beb";
        assert!(parse_checksum(&format!("{hex}\n")).is_some());
        assert!(parse_checksum("<html><head><title>Error Page 404</title>").is_none());
        assert!(parse_checksum("").is_none());
    }

    #[test]
    fn community_edition_is_not_oracle_graalvm() {
        use std::str::FromStr;
        assert_eq!(
            JavaVendor::from_str("GraalVM").unwrap(),
            JavaVendor::GraalVm
        );
        assert_ne!(
            JavaVendor::from_str("GraalVM Community").unwrap(),
            JavaVendor::GraalVm
        );
    }
}
