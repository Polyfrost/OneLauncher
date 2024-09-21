use std::collections::HashMap;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use url::Url;

use crate::{data::{Loader, ManagedPackage, PackageType}, store::{ProviderSearchResults, SearchResult}, utils::http, Result, State};

/// Mapping of Curseforge "classes" to their respective category ids
#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
#[repr(u32)]
pub enum CFPackageType {
	#[default]
	Mods = 6,
	ModPack = 4471,
	DataPack = 6945,
	ResourcePack = 12,
	ShaderPack = 6552,
}

impl From<CFPackageType> for PackageType {
	fn from(package_type: CFPackageType) -> Self {
		match package_type {
			CFPackageType::Mods => PackageType::Mod,
			CFPackageType::ModPack => PackageType::ModPack,
			CFPackageType::DataPack => PackageType::DataPack,
			CFPackageType::ResourcePack => PackageType::ResourcePack,
			CFPackageType::ShaderPack => PackageType::ShaderPack,
		}
	}
}

impl From<PackageType> for CFPackageType {
	fn from(package_type: PackageType) -> Self {
		match package_type {
			PackageType::Mod => CFPackageType::Mods,
			PackageType::ModPack => CFPackageType::ModPack,
			PackageType::DataPack => CFPackageType::DataPack,
			PackageType::ResourcePack => CFPackageType::ResourcePack,
			PackageType::ShaderPack => CFPackageType::ShaderPack,
		}
	}
}

/// Mapping of Curseforge supported loaders to their respective id
#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum CurseforgeLoader {
	#[default]
	Any = 0,
	Forge = 1,
	Fabric = 4,
	Quilt = 5,
	NeoForge = 6,
}

impl From<Loader> for CurseforgeLoader {
	fn from(loader: Loader) -> Self {
		match loader {
			Loader::Forge => CurseforgeLoader::Forge,
			Loader::Fabric => CurseforgeLoader::Fabric,
			Loader::Quilt => CurseforgeLoader::Quilt,
			Loader::NeoForge => CurseforgeLoader::NeoForge,
			_ => CurseforgeLoader::Any,
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Pagination {
	pub index: u32,
	pub page_size: u32,
	pub result_count: u32,
	pub total_count: u32,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgePackage {
    pub id: u32,
    pub game_id: u32,
	#[serde(alias = "classId")]
	pub package_type: CFPackageType,
    pub name: String,
    pub slug: String,
    pub links: Links,
    pub summary: String,
    // pub status: u32,
    pub download_count: u64,
    pub is_featured: bool,
    // pub primary_category_id: u32,
    pub categories: Vec<Category>,
    pub authors: Vec<Author>,
    pub logo: Logo,
    // pub screenshots: Vec<Screenshot>,
    // pub main_file_id: u32,
    // pub latest_files: Vec<LatestFile>,
    // pub latest_files_indexes: Vec<LatestFilesIndex>,
    // pub latest_early_access_files_indexes: Vec<LatestEarlyAccessFilesIndex>,
    pub date_created: DateTime<Utc>,
    pub date_modified: DateTime<Utc>,
    pub date_released: DateTime<Utc>,
	#[serde(default)]
    pub allow_mod_distribution: bool,
    // pub game_popularity_rank: u32,
    // pub is_available: bool,
    pub thumbs_up_count: u32,
    pub rating: Option<f32>,
}

impl From<CurseforgePackage> for ManagedPackage {
	fn from(package: CurseforgePackage) -> Self {
		ManagedPackage {
			id: package.id.to_string(),
			title: package.name,
			provider: super::Providers::Curseforge,
			package_type: package.package_type.into(),
			description: package.summary,
			body: "".to_string(), // TODO: Description is a new HTTP request
			main: todo!(),
			versions: todo!(),
			game_versions: todo!(),
			loaders: todo!(),
			icon_url: todo!(),
			created: todo!(),
			updated: todo!(),
			client: todo!(),
			server: todo!(),
			downloads: todo!(),
			followers: todo!(),
			categories: todo!(),
			optional_categories: todo!(),
			license: todo!(),
			author: todo!(),
			is_archived: todo!(),
		}
	}
}

impl Into<SearchResult> for CurseforgePackage {
	fn into(self) -> SearchResult {
		SearchResult {
			title: self.name,
			slug: self.slug,
			project_id: self.id.to_string(),
			author: self.authors.first().and_then(|a| Some(a.name.clone())).unwrap_or(String::from("Unknown")),
			description: self.summary,
			client_side: crate::store::PackageSide::Unknown,
			server_side: crate::store::PackageSide::Unknown,
			project_type: self.package_type.into(),
			downloads: self.download_count,
			icon_url: self.logo.url,
			categories: vec![], // TODO
			display_categories: vec![],
			versions: vec![],
			follows: self.thumbs_up_count,
			date_created: self.date_created,
			date_modified: self.date_modified,
		}
	}
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    pub website_url: Option<String>,
    pub wiki_url: Option<String>,
    pub issues_url: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: u32,
    pub game_id: u32,
    pub name: String,
    pub slug: String,
    pub url: String,
    pub icon_url: String,
    pub date_modified: String,
    // pub parent_category_id: u32,
    // pub display_index: u32,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub id: u32,
    pub name: String,
    pub url: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Logo {
    pub id: u32,
    pub mod_id: u32,
    pub title: String,
    pub description: String,
    pub thumbnail_url: String,
    pub url: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub id: u32,
    pub mod_id: u32,
    pub title: String,
    pub description: String,
    pub thumbnail_url: String,
    pub url: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestFile {
    pub id: u32,
    pub game_id: u32,
    pub mod_id: u32,
    pub is_available: bool,
    pub display_name: String,
    pub file_name: String,
    // pub release_type: u32,
    // pub file_status: u32,
    // pub hashes: Vec<Hash>,
    // pub file_date: String,
    // pub file_length: u32,
    pub download_count: u32,
    // pub file_size_on_disk: u32,
    pub download_url: Option<String>,
    pub game_versions: Vec<String>,
    pub sortable_game_versions: Vec<SortableGameVersion>,
    pub dependencies: Vec<Dependency>,
    // pub expose_as_alternative: bool,
    // pub parent_project_file_id: u32,
    // pub alternate_file_id: u32,
    // pub is_server_pack: bool,
    // pub server_pack_file_id: u32,
    // pub is_early_access_content: bool,
    // pub early_access_end_date: String,
    // pub file_fingerprint: u32,
    // pub modules: Vec<Module>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hash {
    pub value: String,
    pub algo: u32,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortableGameVersion {
    pub game_version_name: String,
    pub game_version_padded: String,
    pub game_version: String,
    pub game_version_release_date: String,
    pub game_version_type_id: u32,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub mod_id: u32,
    pub relation_type: u32,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub name: String,
    pub fingerprint: u32,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestFilesIndex {
    pub game_version: String,
    pub file_id: u32,
    pub filename: String,
    pub release_type: u32,
    pub game_version_type_id: u32,
	#[serde(default)]
    pub mod_loader: CurseforgeLoader,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestEarlyAccessFilesIndex {
    pub game_version: String,
    pub file_id: u32,
    pub filename: String,
    pub release_type: u32,
    pub game_version_type_id: u32,
	#[serde(default)]
    pub mod_loader: CurseforgeLoader,
}

// https://docs.curseforge.com/rest-api/?shell#tocS_ModsSearchSortField
#[allow(dead_code)]
enum SearchSort {
    Featured = 1,
	Popularity = 2,
	LastUpdated = 3,
	Name = 4,
	Author = 5,
	TotalDownloads = 6,
	Rating = 12,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct CFData<T> {
	data: T,
}

pub async fn fetch_url_builder<F: FnOnce(&mut Url)>(url: &str, url_builder: Option<F>) -> Result<Bytes> {
	// TODO: Get the API key from settings, fallback to constant, and error if missing
	let key = crate::constants::CURSEFORGE_API_KEY.ok_or(anyhow::anyhow!("missing curseforge api key"))?;

	let mut headers = HashMap::<&str, &str>::new();
	headers.insert("x-api-key", key);

	let url = &mut Url::parse(format!(
		"{}{}",
		crate::constants::CURSEFORGE_API_URL,
		url,
	).as_str())?;

	if let Some(url_builder) = url_builder {
		url_builder(url);
	}

	http::fetch_advanced(
		Method::GET,
		url.as_str(),
		None,
		None,
		Some(headers),
		None,
		&State::get().await?.fetch_semaphore,
	)
	.await
}

async fn fetch(url: &str) -> Result<Bytes> {
	fetch_url_builder(url, None::<fn(&mut Url)>).await
}

#[tracing::instrument(skip_all)]
pub async fn search(
	query: Option<String>,
	limit: Option<u8>,
	offset: Option<u32>,
	game_versions: Option<Vec<String>>,
	_categories: Option<Vec<String>>,
	loaders: Option<Vec<Loader>>,
	package_types: Option<Vec<PackageType>>,
) -> Result<ProviderSearchResults> {
	#[derive(Deserialize)]
	struct CFSearchResults {
		pagination: Pagination,
		data: Vec<CurseforgePackage>
	}

	let results: CFSearchResults = serde_json::from_slice(
		&fetch_url_builder(
			"/v1/mods/search",
			Some(Box::new(move |url: &mut Url| {
				let params = &mut url.query_pairs_mut();

				params.append_pair("gameId", crate::constants::CURSEFORGE_GAME_ID.to_string().as_str());
				params.append_pair("pageSize", limit.unwrap_or(10).to_string().as_str());
				params.append_pair("index", offset.unwrap_or(0).to_string().as_str());
				params.append_pair("sortField", (SearchSort::Popularity as u8).to_string().as_str());
				params.append_pair("sortOrder", "desc");

				if let Some(query) = query {
					params.append_pair("searchFilter", query.as_str());
				}

				if let Some(game_versions) = game_versions {
					// Need to create a list e.g. ["1.16.5", "1.17.1"]
					params.append_pair("gameVersions", format!("[{}]", game_versions.into_iter().map(|v| format!("\"{}\"", v)).collect::<Vec<String>>().join(",")).as_str());
				}

				if let Some(loaders) = loaders {
					params.append_pair("modLoaderTypes", format!("[{}]", loaders.into_iter().map(|l| (CurseforgeLoader::from(l) as u32).to_string()).collect::<Vec<String>>().join(",")).as_str());
				}

				if let Some(package_types) = package_types {
					if package_types.len() == 0 {
						return;
					}

					if package_types.len() > 1 {
						tracing::warn!("curseforge provider does not support multiple package types");
					}

					let package_type = package_types.first().unwrap();

					params.append_pair("classId", (CFPackageType::from(*package_type) as u32).to_string().as_str());
				}
			}))
		).await?)?;

	Ok(ProviderSearchResults {
		total: results.pagination.total_count,
		results: results.data.into_iter().map(Into::into).collect::<Vec<SearchResult>>(),
		provider: crate::package::content::Providers::Curseforge,
	})
}

pub async fn get(id: u32) -> Result<CurseforgePackage> {
	Ok(serde_json::from_slice::<CFData<_>>(
		&fetch(format!("/v1/mods/{}", id).as_str())
		.await?,
	)?.data)
}