use reqwest::{Method, Request};
use serde::Deserialize;
use url::Url;

use crate::java::PackageArchive;
use crate::java::data::JavaPackage;
use crate::java::vendors::{JavaRuntimeProvider, JavaVendor};
use crate::state::LauncherServices;
use crate::LauncherResult;

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

	async fn list_packages(&self, major: Option<u32>, services: &LauncherServices) -> LauncherResult<Vec<JavaPackage>> {
        let url = zulu_url(major)?;
        let packages = services.requester.send_as::<Vec<ZuluPackage>>(Request::new(Method::GET, url)).await?;
        let packages = packages.into_iter().map(map_zulu_package).collect();

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

fn zulu_url(major: Option<u32>) -> LauncherResult<Url> {
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

const ZULU_ARCH: &str = cfg_select! {
    target_arch = "x86" => "x86",
    target_arch = "x86_64" => "x64",
    target_arch = "arm" => "arm",
    target_arch = "aarch64" => "aarch64",
};

const ZULU_OS: &str = cfg_select! {
    target_os = "windows" => "windows",
    target_os = "macos" => "macos",
    target_os = "linux" => cfg_select! {
        target_env = "musl" => "linux-musl",
        _ => "linux"
    },
};