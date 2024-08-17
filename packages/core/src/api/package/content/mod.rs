//! **OneLauncher Content Package**
//!
//! Utilities for searching and downloading content packages to OneLauncher.

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::data::{ManagedPackage, ManagedVersion};
use crate::Result;

mod modrinth;

/// Providers for content packages
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum Providers {
	// Curseforge,
	Modrinth,
}

impl Providers {
	/// Get the name of the provider
	pub fn name(&self) -> &str {
		match self {
			// Providers::Curseforge => "Curseforge",
			Providers::Modrinth => "Modrinth",
		}
	}

	/// Get the URL of the provider
	pub fn url(&self) -> &str {
		match self {
			// Providers::Curseforge => "https://curseforge.com/",
			Providers::Modrinth => "https://modrinth.com",
		}
	}

	pub async fn list(&self) -> Result<Vec<ManagedPackage>> {
		Ok(match self {
			// Providers::Curseforge => curseforge::list(),
			Providers::Modrinth => modrinth::list(),
		}
		.await?
		.into_iter()
		.map(|p| p.into())
		.collect())
	}

	pub async fn get(&self, slug_or_id: &str) -> Result<ManagedPackage> {
		Ok(match self {
			// Providers::Curseforge => curseforge::get(slug),
			Providers::Modrinth => modrinth::get(slug_or_id),
		}
		.await?
		.into())
	}

	pub async fn get_versions(&self, project_id: &str) -> Result<Vec<ManagedVersion>> {
		Ok(match self {
			// Providers::Curseforge => curseforge::get_versions(slug),
			Providers::Modrinth => modrinth::get_versions(project_id),
		}
		.await?
		.into_iter()
		.map(|p| p.into())
		.collect())
	}

	pub async fn get_version(&self, version_id: &str) -> Result<ManagedVersion> {
		Ok(match self {
			// Providers::Curseforge => curseforge::get_version(slug, version),
			Providers::Modrinth => modrinth::get_version(version_id),
		}
		.await?
		.into())
	}

	pub async fn get_version_for_game_version(
		&self,
		id: &str,
		game_version: &str,
	) -> Result<ManagedVersion> {
		let versions = self.get_versions(id).await?;
		Ok(versions
			.into_iter()
			.find(|v| v.game_versions.contains(&game_version.to_string()))
			.ok_or(anyhow!("no game version found"))?) // TODO: error handling
	}

	// pub async fn search(&self, query: &str) -> Result<Vec<ManagedPackage>> {
	//     match self {
	//         // Providers::Curseforge => curseforge::search(query),
	//         Providers::Modrinth => modrinth::search(query),
	//     }.await
	// }
}
