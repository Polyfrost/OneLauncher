use std::{path::PathBuf, str::FromStr};

use oneclient_events::{EventBus, GroupedProgressSession};
use oneclient_net::RequestClient;

use crate::data::JavaPackage;
use crate::error::JavaResult;
use crate::platform::{HostArch, HostTarget};

mod adoptium;
mod corretto;
mod graalvm;
mod liberica;
mod microsoft;
mod mojang;
mod zulu;

pub use adoptium::AdoptiumRuntimeProvider;
pub use corretto::CorrettoRuntimeProvider;
pub use graalvm::GraalVmRuntimeProvider;
pub(crate) use graalvm::INSTALL_PREFIX as GRAALVM_INSTALL_PREFIX;
pub use liberica::LibericaRuntimeProvider;
pub use microsoft::MicrosoftRuntimeProvider;
pub(crate) use mojang::INSTALL_PREFIX as MOJANG_INSTALL_PREFIX;
pub use mojang::MojangRuntimeProvider;
use serde::{Deserialize, Deserializer, Serialize};
pub use zulu::ZuluRuntimeProvider;

pub fn runtime_providers() -> Vec<Box<dyn JavaRuntimeProvider>> {
    let mut providers: Vec<Box<dyn JavaRuntimeProvider>> = vec![
        Box::new(MicrosoftRuntimeProvider),
        Box::new(ZuluRuntimeProvider),
        Box::new(AdoptiumRuntimeProvider),
        Box::new(CorrettoRuntimeProvider),
        Box::new(LibericaRuntimeProvider),
        Box::new(GraalVmRuntimeProvider),
        Box::new(MojangRuntimeProvider),
    ];
    if crate::platform::is_legacy_macos() {
        providers.sort_by_key(|provider| provider.vendor() != JavaVendor::Mojang);
    }
    providers
}

// For making sure microsoft jdk provider is first
#[must_use]
pub fn default_vendor() -> JavaVendor {
    runtime_providers()
        .first()
        .expect("the provider list is never empty")
        .vendor()
}

#[async_trait::async_trait]
pub trait JavaRuntimeProvider: Send + Sync {
    fn vendor(&self) -> JavaVendor;

    async fn list_arch_packages(
        &self,
        major: Option<u32>,
        arch: HostArch,
        net: &RequestClient,
    ) -> JavaResult<Vec<JavaPackage>>;

    #[tracing::instrument(level = "debug", skip(self, net))]
    async fn list_packages(
        &self,
        major: Option<u32>,
        net: &RequestClient,
    ) -> JavaResult<Vec<JavaPackage>> {
        let mut packages = Vec::new();
        for arch in HostTarget::runtime_arches() {
            if major.is_some() && !packages.is_empty() {
                break;
            }
            let found = self.list_arch_packages(major, arch, net).await?;
            add_uncovered_majors(&mut packages, found);
        }
        Ok(packages)
    }

    #[tracing::instrument(level = "debug", skip(self, net))]
    async fn latest_package_by_major(
        &self,
        major: u32,
        arches: &[HostArch],
        net: &RequestClient,
    ) -> JavaResult<Option<JavaPackage>> {
        for &arch in arches {
            let found = self
                .list_arch_packages(Some(major), arch, net)
                .await?
                .into_iter()
                .find(|p| p.java_version.first() == Some(&major));
            if found.is_some() {
                return Ok(found);
            }
        }
        Ok(None)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn install_package(
        &self,
        package: &JavaPackage,
        net: &RequestClient,
        events: &EventBus,
        progress: Option<&GroupedProgressSession>,
    ) -> JavaResult<PathBuf> {
        crate::install::install_package(package, net, events, progress).await
    }
}

/// Vendors list builds in arbitrary order; always show the newest major first.
pub(super) fn sort_packages_by_major_desc(packages: &mut [JavaPackage]) {
    packages.sort_by_key(|package| {
        std::cmp::Reverse(package.java_version.first().copied().unwrap_or(0))
    });
}

fn add_uncovered_majors(packages: &mut Vec<JavaPackage>, found: Vec<JavaPackage>) {
    let covered: Vec<u32> = packages
        .iter()
        .filter_map(|p| p.java_version.first().copied())
        .collect();
    packages.extend(found.into_iter().filter(|p| {
        p.java_version
            .first()
            .is_none_or(|major| !covered.contains(major))
    }));
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum JavaVendor {
    Zulu,
    Corretto,
    Oracle,
    GraalVm,
    Microsoft,
    Adoptium,
    Liberica,
    Mojang,
    OpenJDK,
    Other(String),
}

impl std::fmt::Display for JavaVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JavaVendor::Zulu => f.write_str("Zulu"),
            JavaVendor::Adoptium => f.write_str("Temurin"),
            JavaVendor::Liberica => f.write_str("Liberica"),
            JavaVendor::Corretto => f.write_str("Corretto"),
            JavaVendor::Microsoft => f.write_str("Microsoft"),
            JavaVendor::Mojang => f.write_str("Mojang"),
            JavaVendor::Oracle => f.write_str("Oracle"),
            JavaVendor::GraalVm => f.write_str("GraalVM"),
            JavaVendor::OpenJDK => f.write_str("OpenJDK"),
            JavaVendor::Other(other) => f.write_str(other),
        }
    }
}

impl FromStr for JavaVendor {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = s.to_lowercase();

        Ok(
            if v.contains("adoptium") || v.contains("adoptopenjdk") || v.contains("temurin") {
                Self::Adoptium
            } else if v.contains("liberica") || v.contains("bellsoft") {
                Self::Liberica
            } else if v.contains("mojang") {
                Self::Mojang
            } else if v.contains("microsoft") {
                Self::Microsoft
            } else if v.contains("graalvm") && !v.contains("community") {
                Self::GraalVm
            } else if v.contains("openjdk") {
                Self::OpenJDK
            } else if v.contains("oracle") {
                Self::Oracle
            } else if v.contains("amazon") || v.contains("corretto") {
                Self::Corretto
            } else if v.contains("azul") || v.contains("zulu") {
                Self::Zulu
            } else {
                Self::Other(s.to_string())
            },
        )
    }
}

impl<'de> Deserialize<'de> for JavaVendor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_str(&s).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::PackageArchive;

    fn package(major: u32, arch: &str) -> JavaPackage {
        JavaPackage {
            download_url: String::new(),
            name: format!("jdk-{major}-{arch}"),
            java_version: vec![major],
            vendor: JavaVendor::Zulu,
            archive: PackageArchive::Zip,
            checksum: None,
            size: None,
        }
    }

    #[test]
    fn emulated_builds_only_fill_majors_without_a_native_one() {
        let mut packages = vec![package(21, "aarch64"), package(17, "aarch64")];
        add_uncovered_majors(
            &mut packages,
            vec![package(21, "x64"), package(17, "x64"), package(8, "x64")],
        );

        let names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["jdk-21-aarch64", "jdk-17-aarch64", "jdk-8-x64"]);
    }
}
