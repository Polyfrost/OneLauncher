//! **OneLauncher Content Package**
//!
//! Utilities for searching and downloading content packages to OneLauncher.

use modrinth::{Facet, FacetOperation};
use serde::{Deserialize, Serialize};

use crate::data::{Loader, ManagedPackage, ManagedUser, ManagedVersion};
use crate::package::content::modrinth::FacetBuilder;
use crate::store::{Author, ProviderSearchResults};
use crate::Result;

mod modrinth;

/// Providers for content packages
#[cfg_attr(feature = "specta", derive(specta::Type))]
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

	pub async fn search(
		&self,
		query: Option<String>,
		limit: Option<u8>,
		game_versions: Option<Vec<String>>,
		categories: Option<Vec<String>>,
		loaders: Option<Vec<Loader>>,
		open_source: Option<bool>,
	) -> Result<ProviderSearchResults> {
		match self {
			Providers::Modrinth => modrinth::search(
				query,
				limit,
				Some(|mut builder: FacetBuilder| {
					if let Some(game_versions) = game_versions {
						for version in game_versions {
							builder.and(Facet("versions".to_string(), FacetOperation::Eq, version));
						}
					}

					if let Some(categories) = categories {
						for category in categories {
							builder.or(Facet(
								"categories".to_string(),
								FacetOperation::Eq,
								category,
							));
						}
					}

					if let Some(loaders) = loaders {
						for loader in loaders {
							builder.or(Facet(
								"categories".to_string(),
								FacetOperation::Eq,
								loader.to_string(),
							));
						}
					}

					if let Some(open_source) = open_source {
						builder.and(Facet(
							"open_source".to_string(),
							FacetOperation::Eq,
							open_source.to_string(),
						));
					}

					// builder.and(Facet("client_side".to_string(), FacetOperation::Eq, "")) // TODO: Possibly make this client_side = required

					builder.build()
				}),
			),
		}
		.await
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

	pub async fn get_multiple(&self, slug_or_ids: &[String]) -> Result<Vec<ManagedPackage>> {
		Ok(match self {
			Providers::Modrinth => modrinth::get_multiple(slug_or_ids),
		}
		.await?
		.into_iter()
		.map(|p| p.into())
		.collect())
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

	pub async fn get_authors(&self, author: &Author) -> Result<Vec<ManagedUser>> {
		match self {
			Providers::Modrinth => modrinth::get_authors(author),
		}
		.await
	}
}
