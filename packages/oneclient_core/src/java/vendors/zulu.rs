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

	async fn list_packages_by_major(&self, major: u32, services: &LauncherServices) -> LauncherResult<Vec<JavaPackage>> {
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

fn zulu_url(major: u32) -> LauncherResult<Url> {
    Ok(Url::parse(&format!("https://api.azul.com/metadata/v1/zulu/packages/?java_version={major}&os={ZULU_OS}&arch={ZULU_ARCH}&archive_type=zip&java_package_type=jre&javafx_bundled=false&release_status=ga&availability_types=CA&certifications=tck&page=1&page_size=5"))?)
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