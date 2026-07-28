use reqwest::{Method, Request};
use serde::Deserialize;
use url::Url;

use oneclient_net::RequestClient;

use crate::data::{JavaPackage, PackageArchive};
use crate::error::JavaResult;
use crate::platform::{HostArch, HostOs, HostTarget};
use crate::vendors::{JavaRuntimeProvider, JavaVendor};

pub struct ZuluRuntimeProvider;

#[derive(Debug, Deserialize)]
struct ZuluPackage {
	download_url: String,
	name: String,
	java_version: Vec<u32>,
}

#[async_trait::async_trait]
impl JavaRuntimeProvider for ZuluRuntimeProvider {
	fn vendor(&self) -> JavaVendor {
		JavaVendor::Zulu
	}

	#[tracing::instrument(level = "debug", skip(self, net))]
	async fn list_packages(&self, major: Option<u32>, net: &RequestClient) -> JavaResult<Vec<JavaPackage>> {
        let url = zulu_url(major)?;
        let packages = net.send_as::<Vec<ZuluPackage>>(Request::new(Method::GET, url)).await?;
        let packages: Vec<JavaPackage> = packages.into_iter().map(map_zulu_package).collect();

        tracing::debug!(count = packages.len(), "listed Zulu packages");

        Ok(packages)
	}
}

fn map_zulu_package(pkg: ZuluPackage) -> JavaPackage {
    JavaPackage {
        archive: PackageArchive::Zip,
        download_url: pkg.download_url,
        java_version: pkg.java_version,
        name: pkg.name,
        vendor: JavaVendor::Zulu,
    }
}

fn zulu_url(major: Option<u32>) -> JavaResult<Url> {
    let mut url = Url::parse("https://api.azul.com/metadata/v1/zulu/packages/")?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("os", ZULU_OS)
            .append_pair("arch", ZULU_ARCH)
            .append_pair("archive_type", "zip")
            .append_pair("java_package_type", "jre")
            .append_pair("javafx_bundled", "false")
            .append_pair("release_status", "ga")
            .append_pair("availability_types", "CA")
            .append_pair("certifications", "tck")
            .append_pair("latest", "true")
            .append_pair("page", "1")
            .append_pair("page_size", "100");
        if let Some(major) = major {
            q.append_pair("java_version", &major.to_string());
        }
    }
    Ok(url)
}

const ZULU_ARCH: &str = match HostTarget::CURRENT.arch {
    HostArch::X86 => "x86",
    HostArch::X86_64 => "x64",
    HostArch::Arm => "arm",
    HostArch::Aarch64 => "aarch64",
};

const ZULU_OS: &str = match HostTarget::CURRENT.os {
    HostOs::Windows => "windows",
    HostOs::MacOs => "macos",
    HostOs::Linux { musl: true } => "linux-musl",
    HostOs::Linux { musl: false } => "linux",
};