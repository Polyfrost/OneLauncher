use reqwest::{Method, Request};
use serde_json::Value;
use url::Url;

use crate::LauncherResult;
use crate::java::PackageArchive;
use crate::java::data::JavaPackage;
use crate::java::vendors::{JavaRuntimeProvider, JavaVendor};
use crate::state::LauncherServices;

pub struct CorrettoRuntimeProvider;

// { os: { arch: { "jdk": { major: { ext: ... } } } } }
const INDEX_URL: &str =
    "https://corretto.github.io/corretto-downloads/latest_links/indexmap_with_checksum.json";

#[async_trait::async_trait]
impl JavaRuntimeProvider for CorrettoRuntimeProvider {
    fn vendor(&self) -> JavaVendor {
        JavaVendor::Corretto
    }

    async fn list_packages(
        &self,
        major: Option<u32>,
        services: &LauncherServices,
    ) -> LauncherResult<Vec<JavaPackage>> {
        let index = services
            .requester
            .send_as::<Value>(Request::new(Method::GET, Url::parse(INDEX_URL)?))
            .await?;

        // Corretto ships JDK only
        let Some(by_major) = index
            .get(CORRETTO_OS)
            .and_then(|by_arch| by_arch.get(CORRETTO_ARCH))
            .and_then(|by_type| by_type.get("jdk"))
            .and_then(Value::as_object)
        else {
            return Ok(Vec::new());
        };

        let mut packages = Vec::new();
        for (major_str, by_ext) in by_major {
            let Ok(this_major) = major_str.parse::<u32>() else {
                continue;
            };
            if let Some(filter) = major
                && filter != this_major
            {
                continue;
            }

            let has_ext = by_ext
                .as_object()
                .is_some_and(|exts| exts.contains_key(CORRETTO_EXT.0));
            if !has_ext {
                continue;
            }

            let download_url = latest_url(this_major, CORRETTO_EXT.0);
            let name = format!(
                "amazon-corretto-{this_major}-{CORRETTO_ARCH}-{CORRETTO_OS}-jdk.{}",
                CORRETTO_EXT.0
            );

            packages.push(JavaPackage {
                archive: CORRETTO_EXT.1,
                download_url,
                java_version: vec![this_major],
                name,
                vendor: JavaVendor::Corretto,
            });
        }

        packages.sort_by_key(|p| std::cmp::Reverse(p.java_version.first().copied().unwrap_or(0)));
        Ok(packages)
    }
}

fn latest_url(major: u32, ext: &str) -> String {
    format!(
        "https://corretto.aws/downloads/latest/amazon-corretto-{major}-{CORRETTO_ARCH}-{CORRETTO_OS}-jdk.{ext}"
    )
}

const CORRETTO_ARCH: &str = cfg_select! {
    target_arch = "x86" => "x86",
    target_arch = "x86_64" => "x64",
    target_arch = "arm" => "arm",
    target_arch = "aarch64" => "aarch64",
};

const CORRETTO_OS: &str = cfg_select! {
    target_os = "windows" => "windows",
    target_os = "macos" => "macos",
    target_os = "linux" => "linux",
};

const CORRETTO_EXT: (&str, PackageArchive) = cfg_select! {
    target_os = "windows" => ("zip", PackageArchive::Zip),
    _ => ("tar.gz", PackageArchive::TarGz),
};
