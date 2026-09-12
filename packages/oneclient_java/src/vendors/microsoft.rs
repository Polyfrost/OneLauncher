use futures_util::StreamExt;
use reqwest::{Method, Request};
use url::Url;

use oneclient_net::RequestClient;
use polyio::Checksum;

use crate::data::{JavaPackage, PackageArchive};
use crate::error::JavaResult;
use crate::platform::{HostArch, HostOs, HostTarget};
use crate::vendors::{JavaRuntimeProvider, JavaVendor};

pub struct MicrosoftRuntimeProvider;

/// Microsoft publishes no metadata API at all only static `aka.ms` vanity
/// links so the majors it still builds have to be listed here
const MICROSOFT_MAJORS: &[u32] = &[25, 21, 17, 11];

/// Alpine is the one platform Microsoft never filled out x64 only and nothing
/// newer than 17
const MICROSOFT_ALPINE_MAJORS: &[u32] = &[17, 11];

/// Four small redirects when no major is pinned nowhere near enough to be
/// worth throttling harder
const MAJOR_LOOKUP_CONCURRENCY: usize = 4;

#[async_trait::async_trait]
impl JavaRuntimeProvider for MicrosoftRuntimeProvider {
    fn vendor(&self) -> JavaVendor {
        JavaVendor::Microsoft
    }

    #[tracing::instrument(level = "debug", skip(self, net))]
    async fn list_packages(
        &self,
        major: Option<u32>,
        net: &RequestClient,
    ) -> JavaResult<Vec<JavaPackage>> {
        let Some(arch) = MICROSOFT_ARCH else {
            tracing::debug!(
                "Microsoft publishes no build for this architecture; listing nothing"
            );
            return Ok(Vec::new());
        };

        let wanted: Vec<u32> = MICROSOFT_MAJORS
            .iter()
            .copied()
            .filter(|candidate| major.is_none_or(|filter| filter == *candidate))
            .filter(|candidate| publishes_host_build(*candidate, arch))
            .collect();

        let mut packages: Vec<JavaPackage> = futures_util::stream::iter(
            wanted
                .into_iter()
                .map(|major| async move { resolve_package(major, arch, net).await }),
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
        tracing::debug!(count = packages.len(), "listed Microsoft packages");
        Ok(packages)
    }
}

/// The checksum file names the build it belongs to so one request resolves the
/// exact version, the filename and the hash the major-only link would download
#[tracing::instrument(level = "debug", skip(net))]
async fn resolve_package(
    major: u32,
    arch: &str,
    net: &RequestClient,
) -> JavaResult<Option<JavaPackage>> {
    let url = checksum_url(major, arch)?;
    let response = net.send(Request::new(Method::GET, url)).await?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(oneclient_net::RequestError::from)?;

    if !status.is_success() {
        tracing::debug!(
            major,
            status = status.as_u16(),
            "Microsoft checksum file is unavailable; skipping the major"
        );
        return Ok(None);
    }

    let Some(build) = parse_checksum_file(&body, major) else {
        tracing::debug!(
            major,
            "Microsoft published no build under this link; skipping the major"
        );
        return Ok(None);
    };

    Ok(Some(
        JavaPackage {
            archive: MICROSOFT_EXT.1,
            download_url: download_url(&build.filename),
            java_version: build.java_version,
            name: build.filename,
            vendor: JavaVendor::Microsoft,
            checksum: None,
            size: None,
        }
        .with_checksum(Some(build.checksum)),
    ))
}

struct MicrosoftBuild {
    filename: String,
    java_version: Vec<u32>,
    checksum: Checksum,
}

/// `aka.ms` answers a link it does not know with a 200 and a search page
/// rather than a 404 so only the body can say whether a build exists
fn parse_checksum_file(body: &str, major: u32) -> Option<MicrosoftBuild> {
    let line = body.lines().next()?.trim();
    let (hex, filename) = line.split_once(char::is_whitespace)?;
    let filename = filename.trim();

    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }

    let java_version = version_from_filename(filename)?;

    // A link that quietly moved to another major would otherwise install the
    // wrong runtime under the requested one's name
    if java_version.first() != Some(&major) {
        tracing::warn!(
            major,
            filename,
            "Microsoft's major link resolved to a different major; skipping it"
        );
        return None;
    }

    Some(MicrosoftBuild {
        filename: filename.to_string(),
        java_version,
        checksum: Checksum::sha256(hex),
    })
}

/// Splitting the whole filename the way the other vendors split their version
/// strings would fold the `64` of `x64` into the version
fn version_from_filename(filename: &str) -> Option<Vec<u32>> {
    let (version, _) = filename
        .strip_prefix("microsoft-jdk-")?
        .split_once('-')?;

    let parts: Vec<u32> = version
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;

    (!parts.is_empty()).then_some(parts)
}

/// Microsoft never published a JRE so every link is a kit by construction
fn checksum_url(major: u32, arch: &str) -> JavaResult<Url> {
    Ok(Url::parse(&format!(
        "https://aka.ms/download-jdk/microsoft-jdk-{major}-{MICROSOFT_OS}-{arch}.{}.sha256sum.txt",
        MICROSOFT_EXT.0
    ))?)
}

/// Prefer the versioned link the checksum names over the major-only one it was
/// read through so the download cannot drift onto a newer build mid-release
fn download_url(filename: &str) -> String {
    format!("https://aka.ms/download-jdk/{filename}")
}

fn publishes_host_build(major: u32, arch: &str) -> bool {
    if HostTarget::CURRENT.is_musl() {
        return arch == "x64" && MICROSOFT_ALPINE_MAJORS.contains(&major);
    }
    true
}

/// Microsoft doesn't supply 32-bit
const MICROSOFT_ARCH: Option<&str> = match HostTarget::CURRENT.arch {
    HostArch::X86_64 => Some("x64"),
    HostArch::Aarch64 => Some("aarch64"),
    HostArch::X86 | HostArch::Arm => None,
};

/// Microsoft files musl builds under `alpine` where the other vendors say
/// `linux-musl`
const MICROSOFT_OS: &str = match HostTarget::CURRENT.os {
    HostOs::Windows => "windows",
    HostOs::MacOs => "macos",
    HostOs::Linux { musl: true } => "alpine",
    HostOs::Linux { musl: false } => "linux",
};

const MICROSOFT_EXT: (&str, PackageArchive) = (
    HostTarget::CURRENT.archive_ext(),
    HostTarget::CURRENT.archive(),
);

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str =
        "bf27a5d6298c736af8daf5b8c883098e83291446e5766118d8a5ea6a2617195d  microsoft-jdk-21.0.12-windows-x64.zip";

    #[test]
    fn only_kits_are_ever_requested() {
        let url = checksum_url(21, "x64").expect("a valid url");

        assert!(url.as_str().contains("microsoft-jdk-21-"));
        assert!(!url.as_str().contains("jre"));
    }

    #[test]
    fn a_checksum_file_carries_the_build_it_belongs_to() {
        let build = parse_checksum_file(LINE, 21).expect("a build");

        assert_eq!(build.filename, "microsoft-jdk-21.0.12-windows-x64.zip");
        assert_eq!(build.java_version, vec![21, 0, 12]);
        assert_eq!(build.checksum.algorithm, polyio::ChecksumAlgorithm::Sha256);
        assert!(build.checksum.is_well_formed());
    }

    #[test]
    fn the_search_page_a_dead_link_redirects_to_is_not_a_build() {
        assert!(parse_checksum_file("<!doctype html><html lang=\"en\">", 21).is_none());
        assert!(parse_checksum_file("", 21).is_none());
    }

    #[test]
    fn a_link_resolving_to_another_major_is_refused() {
        assert!(parse_checksum_file(LINE, 17).is_none());
    }

    #[test]
    fn the_architecture_never_leaks_into_the_version() {
        assert_eq!(
            version_from_filename("microsoft-jdk-21.0.12-windows-x64.zip"),
            Some(vec![21, 0, 12])
        );
        assert_eq!(
            version_from_filename("microsoft-jdk-25.0.4-linux-aarch64.tar.gz"),
            Some(vec![25, 0, 4])
        );
    }

    #[test]
    fn debug_symbol_builds_are_never_mistaken_for_runtimes() {
        assert_eq!(
            version_from_filename("microsoft-jdk-debugsymbols-21.0.12-windows-x64.zip"),
            None
        );
    }

    #[test]
    fn alpine_only_gets_the_majors_microsoft_actually_built_for_it() {
        for major in MICROSOFT_MAJORS {
            let published = publishes_host_build(*major, "x64");
            if HostTarget::CURRENT.is_musl() {
                assert_eq!(published, MICROSOFT_ALPINE_MAJORS.contains(major));
            } else {
                assert!(published);
            }
        }

        if HostTarget::CURRENT.is_musl() {
            assert!(!publishes_host_build(17, "aarch64"));
        }
    }
}
