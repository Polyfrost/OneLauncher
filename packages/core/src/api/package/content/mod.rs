//! **`OneLauncher` Content Package**
//!
//! Utilities for searching and downloading content packages to `OneLauncher`.

use std::collections::HashMap;

use modrinth::{Facet, FacetOperation};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::data::{Loader, ManagedPackage, ManagedUser, ManagedVersion, PackageType};
use crate::package::content::modrinth::FacetBuilder;
use crate::store::{Author, PackageBody, ProviderSearchResults};
use crate::utils::http::fetch_json;
use crate::utils::pagination::Pagination;
use crate::{Result, State};

mod curseforge;
mod modrinth;
mod skyclient;

/// Providers for content packages
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum Providers {
	Modrinth,
	Curseforge,
	SkyClient,
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
			Self::Curseforge => "Curseforge",
			Self::SkyClient => "SkyClient",
		}
	}

	/// Get the URL of the provider
	#[must_use]
	pub const fn url(&self) -> &str {
		match self {
			Self::Modrinth => "https://modrinth.com",
			Self::Curseforge => "https://curseforge.com",
			Self::SkyClient => "https://skyclient.co",
		}
	}

	pub const fn get_providers() -> &'static [Providers] {
		&[Self::Modrinth, Self::Curseforge, Self::SkyClient]
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
			Self::Modrinth => {
				modrinth::search(
					query,
					limit,
					offset,
					Some(|mut builder| {
						build_facets(
							&mut builder,
							categories,
							game_versions,
							loaders,
							package_types,
							open_source,
						)
					}),
				)
				.await
			}
			Self::Curseforge => {
				curseforge::search(
					query,
					limit,
					offset,
					game_versions,
					categories,
					loaders,
					package_types,
				)
				.await
			}
			Self::SkyClient => {
				skyclient::search(
					query,
					limit,
					offset,
					game_versions,
					loaders
				).await
			},
		}
	}

	pub async fn get(&self, slug_or_id: &str) -> Result<ManagedPackage> {
		Ok(match self {
			Self::Modrinth => modrinth::get(slug_or_id).await?.into(),
			Self::Curseforge => curseforge::get(slug_or_id.parse::<u32>().map_err(|err| anyhow::anyhow!(err))?).await?.into(),
			Self::SkyClient => skyclient::get(slug_or_id).await?.into(),
		})
	}

	pub async fn get_multiple(&self, slug_or_ids: &[String]) -> Result<Vec<ManagedPackage>> {
		if slug_or_ids.len() <= 0 {
			return Ok(vec![]);
		}

		Ok(match self {
			Self::Modrinth => modrinth::get_multiple(slug_or_ids)
					.await?
					.into_iter()
					.map(Into::into)
					.collect(),
			Self::Curseforge => {
				let parsed_ids = slug_or_ids
					.iter()
					.filter_map(|id| id.parse::<u32>().ok())
					.collect::<Vec<u32>>();

				if parsed_ids.len() <= 0 {
					return Ok(vec![]);
				}

				let parsed_ids = parsed_ids.as_slice();

				curseforge::get_multiple(parsed_ids)
					.await?
					.into_iter()
					.map(Into::into)
					.collect()
			},
			Self::SkyClient => skyclient::get_multiple(slug_or_ids)
				.await?
				.into_iter()
				.map(Into::into)
				.collect(),
		})
	}

	pub async fn get_all_versions(
		&self,
		project_id: &str,
		game_versions: Option<Vec<String>>,
		loaders: Option<Vec<Loader>>,
		page: Option<u32>,
		page_size: Option<u16>,
	) -> Result<(Vec<ManagedVersion>, Pagination)> {
		Ok(match self {
			Self::Modrinth => {
				let data = modrinth::get_all_versions(project_id, game_versions, loaders, page, page_size).await?;
				(data.0.into_iter().map(Into::into).collect(), data.1)
			}

			Self::Curseforge => {
				let data = curseforge::get_all_versions(project_id, game_versions, loaders, page, page_size).await?;
				(data.0.into_iter().map(Into::into).collect(), data.1)
			}

			Self::SkyClient => todo!(),
		})
	}

	pub async fn get_versions(&self, versions: Vec<String>) -> Result<Vec<ManagedVersion>> {
		Ok(match self {
			Self::Modrinth => modrinth::get_versions(versions)
				.await?
				.into_iter()
				.map(Into::into)
				.collect(),
			Self::Curseforge => curseforge::get_versions(versions)
				.await?
				.into_iter()
				.map(Into::into)
				.collect(),
			Self::SkyClient => todo!(),
		})
	}

	pub async fn get_authors(&self, author: &Author) -> Result<Vec<ManagedUser>> {
		Ok(match author {
			Author::Team { .. } => match self {
				Self::Modrinth => modrinth::get_authors(author).await?,
				_ => return Err(anyhow::anyhow!("{} does not support teams", self).into()),
			},
			Author::Users(users) => users.to_owned(),
		})
	}

	pub async fn get_package_body(&self, body: &PackageBody) -> Result<String> {
		Ok(match body {
			PackageBody::Url(url) => match self {
				Self::Curseforge => curseforge::get_package_body(url.to_owned()).await?,
				_ => return Err(anyhow::anyhow!("{} does not support direct URLs", self).into()),
			},
			PackageBody::Markdown(markdown) => markdown.to_owned(),
		})
	}

	pub async fn get_versions_by_hashes(&self, hashes: Vec<String>) -> Result<HashMap<String, ManagedVersion>> {
		Ok(match self {
			Self::Modrinth => modrinth::get_versions_by_hashes(hashes)
				.await?
				.into_iter()
				.map(|(hash, version)| (hash, version.into()))
				.collect(),
			Self::Curseforge => curseforge::get_versions_by_hashes(hashes)
				.await?
				.into_iter()
				.map(|(hash, version)| (hash, version.into()))
				.collect(),
			Self::SkyClient => todo!(),
		})
	}
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Serialize, Deserialize)]
pub struct FeaturedPackage {
	pub package_type: PackageType,
	pub provider: Providers,
	pub id: String,
	pub title: String,
	pub description: String,
	pub thumbnail: String,
	pub oneconfig: bool,
}

pub async fn get_featured_packages() -> Result<Vec<FeaturedPackage>> {
	let state = State::get().await?;
	fetch_json(
		Method::GET,
		crate::constants::FEATURED_PACKAGES_URL,
		None,
		None,
		&state.fetch_semaphore
	).await
}

fn build_facets(
	builder: &mut FacetBuilder,
	categories: Option<Vec<String>>,
	game_versions: Option<Vec<String>>,
	loaders: Option<Vec<Loader>>,
	package_types: Option<Vec<PackageType>>,
	open_source: Option<bool>,
) -> String {
	if let Some(categories) = categories {
		for category in categories {
			builder.and(Facet(
				"categories".to_string(),
				FacetOperation::Eq,
				category,
			));
		}
	}

	if let Some(game_versions) = game_versions {
		for version in game_versions {
			builder.and(Facet("versions".to_string(), FacetOperation::Eq, version));
		}
	}

	if let Some(package_types) = package_types {
		if package_types.contains(&PackageType::Mod)
			|| package_types.contains(&PackageType::ModPack)
		{
			if let Some(loaders) = loaders {
				for loader in loaders {
					builder.and(Facet(
						"categories".to_string(),
						FacetOperation::Eq,
						loader.to_string(),
					));
				}
			}
		}

		for package_type in package_types {
			builder.and(Facet(
				"project_types".to_string(),
				FacetOperation::Eq,
				package_type.get_name().to_string(),
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
}
