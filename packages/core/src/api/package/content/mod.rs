//! **`OneLauncher` Content Package**
//!
//! Utilities for searching and downloading content packages to `OneLauncher`.

use modrinth::{Facet, FacetOperation};
use serde::{Deserialize, Serialize};

use crate::data::{Loader, ManagedPackage, ManagedUser, ManagedVersion, PackageType};
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
	#[must_use]
	pub const fn name(&self) -> &str {
		match self {
			Self::Modrinth => "Modrinth",
		}
	}

	/// Get the URL of the provider
	#[must_use]
	pub const fn url(&self) -> &str {
		match self {
			Self::Modrinth => "https://modrinth.com",
		}
	}

	#[allow(clippy::too_many_arguments)]
	pub async fn search(
		&self,
		query: Option<String>,
		limit: Option<u8>,
		offset: Option<u32>,
		game_versions: Option<Vec<String>>,
		categories: Option<Vec<String>>,
		loaders: Option<Vec<Loader>>,
		package_types: Option<Vec<PackageType>>,
		open_source: Option<bool>,
	) -> Result<ProviderSearchResults> {
		match self {
			Self::Modrinth => modrinth::search(
				query,
				limit,
				offset,
				Some(|mut builder: FacetBuilder| {
					if let Some(game_versions) = game_versions {
						for version in game_versions {
							builder.and(Facet("versions".to_string(), FacetOperation::Eq, version));
						}
					}

					if let Some(categories) = categories {
						for category in categories {
							builder.and(Facet(
								"categories".to_string(),
								FacetOperation::Eq,
								category,
							));
						}
					}

					if let Some(package_types) = package_types {
						for package_type in package_types {
							builder.and(Facet(
								"project_types".to_string(),
								FacetOperation::Eq,
								package_type.get_name().to_string(),
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
			Self::Modrinth => modrinth::list(),
		}
		.await?
		.into_iter()
		.map(Into::into)
		.collect())
	}

	pub async fn get(&self, slug_or_id: &str) -> Result<ManagedPackage> {
		Ok(match self {
			Self::Modrinth => modrinth::get(slug_or_id),
		}
		.await?
		.into())
	}

	pub async fn get_multiple(&self, slug_or_ids: &[String]) -> Result<Vec<ManagedPackage>> {
		Ok(match self {
			Self::Modrinth => modrinth::get_multiple(slug_or_ids),
		}
		.await?
		.into_iter()
		.map(Into::into)
		.collect())
	}

	pub async fn get_all_versions(
		&self,
		project_id: &str,
		game_versions: Option<Vec<String>>,
		loaders: Option<Vec<Loader>>,
	) -> Result<Vec<ManagedVersion>> {
		Ok(match self {
			Self::Modrinth => modrinth::get_all_versions(project_id, game_versions, loaders),
		}
		.await?
		.into_iter()
		.map(Into::into)
		.collect())
	}

	pub async fn get_versions(&self, versions: Vec<String>) -> Result<Vec<ManagedVersion>> {
		Ok(match self {
			Self::Modrinth => modrinth::get_versions(versions),
		}
		.await?
		.into_iter()
		.map(Into::into)
		.collect())
	}

	pub async fn get_version(&self, version_id: &str) -> Result<ManagedVersion> {
		Ok(match self {
			Self::Modrinth => modrinth::get_version(version_id),
		}
		.await?
		.into())
	}

	pub async fn get_authors(&self, author: &Author) -> Result<Vec<ManagedUser>> {
		match self {
			Self::Modrinth => modrinth::get_authors(author),
		}
		.await
	}
}
