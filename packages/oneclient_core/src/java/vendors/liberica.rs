use reqwest::{Method, Request};
use serde::Deserialize;
use url::Url;

use crate::LauncherResult;
use crate::java::PackageArchive;
use crate::java::data::JavaPackage;
use crate::java::vendors::{JavaRuntimeProvider, JavaVendor};
use crate::state::LauncherServices;

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
}

#[async_trait::async_trait]
impl JavaRuntimeProvider for LibericaRuntimeProvider {
    fn vendor(&self) -> JavaVendor {
        JavaVendor::Liberica
    }

    async fn list_packages(
        &self,
        major: Option<u32>,
        services: &LauncherServices,
    ) -> LauncherResult<Vec<JavaPackage>> {
        let url = liberica_url(major)?;
        let releases = services
            .requester
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

                JavaPackage {
                    archive: PackageArchive::from_filename(&r.filename),
                    download_url: r.download_url,
                    java_version,
                    name: r.filename,
                    vendor: JavaVendor::Liberica,
                }
            })
            .collect();

        Ok(packages)
    }
}

fn liberica_url(major: Option<u32>) -> LauncherResult<Url> {
    let mut url = Url::parse("https://api.bell-sw.com/v1/liberica/releases")?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("os", LIBERICA_OS)
            .append_pair("arch", LIBERICA_ARCH)
            .append_pair("bundle-type", "jre")
            .append_pair("bitness", LIBERICA_BITNESS)
            .append_pair("package-type", LIBERICA_PACKAGE)
            .append_pair("installation-type", "archive")
            .append_pair(
                "fields",
                "downloadUrl,filename,featureVersion,updateVersion,buildVersion,version",
            )
            .append_pair("output", "json");
        if let Some(major) = major {
            q.append_pair("version-feature", &major.to_string());
        }
    }
    Ok(url)
}

const LIBERICA_ARCH: &str = cfg_select! {
    target_arch = "x86" => "x86",
    target_arch = "x86_64" => "x86",
    target_arch = "arm" => "arm",
    target_arch = "aarch64" => "arm",
};

const LIBERICA_BITNESS: &str = cfg_select! {
    target_arch = "x86" => "32",
    target_arch = "x86_64" => "64",
    target_arch = "arm" => "32",
    target_arch = "aarch64" => "64",
};

const LIBERICA_OS: &str = cfg_select! {
    target_os = "windows" => "windows",
    target_os = "macos" => "macos",
    target_os = "linux" => cfg_select! {
        target_env = "musl" => "linux-musl",
        _ => "linux"
    },
};

const LIBERICA_PACKAGE: &str = cfg_select! {
    target_os = "windows" => "zip",
    _ => "tar.gz",
};
