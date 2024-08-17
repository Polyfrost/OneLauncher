//! **OneLauncher Content Package**
//!
//! Utilities for searching and downloading content packages to OneLauncher.

use serde::{Deserialize, Serialize};

use crate::data::{ManagedPackage, ManagedVersion};
use crate::Result;

mod modrinth;

/// Providers for content packages
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum Providers {
	Modrinth,
}

impl std::fmt::Display for Providers {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.name())
	}
}

impl Providers {
	/// Get the name of the provider
	pub fn name(&self) -> &str {
		match self {
			Providers::Modrinth => "Modrinth",
		}
	}

	/// Get the URL of the provider
	pub fn url(&self) -> &str {
		match self {
			Providers::Modrinth => "https://modrinth.com",
		}
	}

	pub async fn list(&self) -> Result<Vec<ManagedPackage>> {
		Ok(match self {
			Providers::Modrinth => modrinth::list(),
		}
		.await?
		.into_iter()
		.map(|p| p.into())
		.collect())
	}

	pub async fn get(&self, slug_or_id: &str) -> Result<ManagedPackage> {
		Ok(match self {
			Providers::Modrinth => modrinth::get(slug_or_id),
		}
		.await?
		.into())
	}

	pub async fn get_versions(&self, project_id: &str) -> Result<Vec<ManagedVersion>> {
		Ok(match self {
			Providers::Modrinth => modrinth::get_versions(project_id),
		}
		.await?
		.into_iter()
		.map(|p| p.into())
		.collect())
	}

	pub async fn get_version(&self, version_id: &str) -> Result<ManagedVersion> {
		Ok(match self {
			Providers::Modrinth => modrinth::get_version(version_id),
		}
		.await?
		.into())
	}
}
