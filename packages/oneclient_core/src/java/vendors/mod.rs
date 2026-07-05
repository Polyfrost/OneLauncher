use std::{path::PathBuf, str::FromStr};

use crate::java::JavaPackage;
use crate::state::LauncherServices;
use crate::LauncherResult;

mod adoptium;
mod zulu;

pub use adoptium::AdoptiumRuntimeProvider;
use serde::{Deserialize, Deserializer, Serialize};
pub use zulu::ZuluRuntimeProvider;

pub fn runtime_providers() -> Vec<Box<dyn JavaRuntimeProvider>> {
    vec![
        Box::new(ZuluRuntimeProvider),
        Box::new(AdoptiumRuntimeProvider),
    ]
}

#[async_trait::async_trait]
pub trait JavaRuntimeProvider: Send + Sync {
	fn vendor(&self) -> JavaVendor;

	async fn list_packages_by_major(
		&self,
		major: u32,
		services: &LauncherServices,
	) -> LauncherResult<Vec<JavaPackage>>;

    async fn latest_package_by_major(
        &self,
        major: u32,
        services: &LauncherServices
    ) -> LauncherResult<Option<JavaPackage>> {
        let packages = self.list_packages_by_major(major, services).await?;
        Ok(packages.first().cloned())
    }

	async fn install_package(
		&self,
		package: &JavaPackage,
		services: &LauncherServices,
	) -> LauncherResult<PathBuf> {
        super::install_package(package, services).await
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum JavaVendor {
	Zulu,
	Corretto,
	Oracle,
	Microsoft,
	Adoptium,
	OpenJDK,
	Other(String),
}

impl std::fmt::Display for JavaVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JavaVendor::Zulu => f.write_str("Zulu"),
            JavaVendor::Adoptium => f.write_str("Temurin"),
            JavaVendor::Corretto => f.write_str("Corretto"),
            JavaVendor::Microsoft => f.write_str("Microsoft"),
            JavaVendor::Oracle => f.write_str("Oracle"),
            JavaVendor::OpenJDK => f.write_str("OpenJDK"),
            JavaVendor::Other(other) => f.write_str(other),
        }
    }
}

impl FromStr for JavaVendor {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let v = s.to_lowercase();

		Ok(if v.contains("adoptium") || v.contains("adoptopenjdk") {
			Self::Adoptium
		} else if v.contains("microsoft") {
			Self::Microsoft
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
		})
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
