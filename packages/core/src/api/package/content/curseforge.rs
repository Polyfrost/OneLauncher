use std::collections::HashMap;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_repr::{Deserialize_repr, Serialize_repr};
use url::Url;

use crate::data::{Loader, ManagedPackage, ManagedUser, ManagedVersion, PackageType};
use crate::store::{ManagedVersionFile, ManagedVersionReleaseType, ProviderSearchResults, SearchResult};
use crate::utils::{http, pagination::Pagination};
use crate::{Result, State};

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

impl From<String> for CurseforgeLoader {
	fn from(loader: String) -> Self {
		match loader.to_lowercase().as_str() {
			"forge" => CurseforgeLoader::Forge,
			"fabric" => CurseforgeLoader::Fabric,
			"quilt" => CurseforgeLoader::Quilt,
			"neoforge" => CurseforgeLoader::NeoForge,
			_ => CurseforgeLoader::Any,
		}
	}
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

impl Into<Loader> for CurseforgeLoader {
	fn into(self) -> Loader {
		match self {
			CurseforgeLoader::Forge => Loader::Forge,
			CurseforgeLoader::Fabric => Loader::Fabric,
			CurseforgeLoader::Quilt => Loader::Quilt,
			CurseforgeLoader::NeoForge => Loader::NeoForge,
			_ => Loader::Vanilla,
		}
	}
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
	#[serde(default)]
	pub is_featured: bool,
	// pub primary_category_id: u32,
	pub categories: Vec<Category>,
	pub authors: Vec<Author>,
	pub logo: Option<Logo>,
	// pub screenshots: Vec<Screenshot>,
	// pub main_file_id: u32,
	// pub latest_files: Vec<LatestFile>,
	// pub latest_files_indexes: Vec<LatestFilesIndex>,
	// pub latest_early_access_files_indexes: Vec<LatestEarlyAccessFilesIndex>,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
	pub date_released: DateTime<Utc>,
	// #[serde(default)]
	// pub allow_mod_distribution: bool,
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
			body: crate::store::PackageBody::Url(format!("/v1/mods/{}/description", package.id)),
			main: package.slug,
			versions: vec![],
			game_versions: vec![],
			loaders: vec![],
			icon_url: package.logo.and_then(|l| Some(l.url)),
			created: Some(package.date_created),
			updated: Some(package.date_modified),
			client: crate::store::PackageSide::Unknown,
			server: crate::store::PackageSide::Unknown,
			downloads: package.download_count,
			followers: package.rating.unwrap_or(0.0) as u32,
			categories: vec![], // TODO
			optional_categories: None,
			license: None,
			author: crate::store::Author::Users(
				package
					.authors
					.into_iter()
					.map(|a| ManagedUser {
						id: a.id.to_string(),
						username: a.name,
						url: Some(a.url),
						is_organization_user: false,
						avatar_url: None,
						bio: None,
						role: None,
					})
					.collect(),
			),
			is_archived: false,
		}
	}
}

impl Into<SearchResult> for CurseforgePackage {
	fn into(self) -> SearchResult {
		SearchResult {
			title: self.name,
			slug: self.slug,
			project_id: self.id.to_string(),
			author: self
				.authors
				.first()
				.and_then(|a| Some(a.name.clone()))
				.unwrap_or(String::from("Unknown")),
			description: self.summary,
			client_side: crate::store::PackageSide::Unknown,
			server_side: crate::store::PackageSide::Unknown,
			project_type: self.package_type.into(),
			downloads: self.download_count,
			icon_url: self.logo.map_or(String::new(), |l| l.url),
			categories: vec![], // TODO
			versions: vec![],
			follows: self.thumbs_up_count,
			date_created: Some(self.date_created),
			date_modified: Some(self.date_modified),
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

#[derive(Default, Serialize_repr, Deserialize_repr, Clone, Debug)]
#[repr(u8)]
pub enum ModFileReleaseType {
	#[default]
	Release = 1,
	Beta = 2,
	Alpha = 3,
}

impl Into<ManagedVersionReleaseType> for ModFileReleaseType {
	fn into(self) -> ManagedVersionReleaseType {
		match self {
			ModFileReleaseType::Release => ManagedVersionReleaseType::Release,
			ModFileReleaseType::Beta => ManagedVersionReleaseType::Beta,
			ModFileReleaseType::Alpha => ManagedVersionReleaseType::Alpha,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
	pub id: u32,
	pub game_id: u32,
	pub mod_id: u32,
	#[serde(default)]
	pub is_available: bool,
	pub display_name: String,
	pub file_name: String,
	pub release_type: ModFileReleaseType,
	// pub file_status: u32,
	pub hashes: Vec<Hash>,
	pub file_date: DateTime<Utc>,
	pub file_length: u64,
	pub download_count: u32,
	pub file_size_on_disk: Option<u64>,
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
	pub file_fingerprint: u32,
	// pub modules: Vec<Module>,
}

impl Into<ManagedVersion> for ModFile {
	fn into(self) -> ManagedVersion {
		let mut hashes = HashMap::new();

		for hash in self.hashes {
			hashes.insert(hash.algo.name(), hash.value);
		}

		let mut game_versions = Vec::new();
		let mut loader = CurseforgeLoader::Any;

		for version in self.sortable_game_versions {
			if version.game_version.is_empty() {
				// This is most likely a loader
				loader = CurseforgeLoader::from(version.game_version_name);
				continue;
			}

			game_versions.push(version.game_version);
		}

		let mut files = Vec::new();
		if let Some(download_url) = self.download_url {
			files.push(ManagedVersionFile {
				url: download_url,
				file_name: self.file_name,
				primary: true,
				size: self.file_size_on_disk.unwrap_or(self.file_length),
				file_type: Some(crate::store::PackageFile::RequiredPack),
				hashes,
			});
		}

		ManagedVersion {
			package_id: self.mod_id.to_string(),
			id: self.id.to_string(),
			name: self.display_name.clone(),
			author: String::new(),
			loaders: vec![Into::<Loader>::into(loader)],
			changelog: String::new(),
			changelog_url: None,
			deps: vec![],
			downloads: self.download_count,
			featured: false,
			is_available: self.is_available && files.len() > 0,
			files,
			game_versions: game_versions,
			published: self.file_date,
			version_display: self.display_name,
			version_type: self.release_type.into(),
		}
	}
}

#[derive(Debug, serde_repr::Serialize_repr, serde_repr::Deserialize_repr)]
#[repr(u8)]
pub enum HashAlgorithm {
	Sha1 = 1,
	MD5 = 2,
}

impl HashAlgorithm {
	pub fn name(&self) -> String {
		match self {
			Self::Sha1 => "sha1",
			Self::MD5 => "md5",
		}.to_string()
	}
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hash {
	pub value: String,
	pub algo: HashAlgorithm,
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
pub struct ModFilesIndex {
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

#[derive(Default, Debug, Serialize, Deserialize)]
struct CFPaginatedData<T> {
	pagination: Pagination,
	data: T,
}

pub async fn fetch_advanced<F: FnOnce(&mut Url)>(
	method: Method,
	url: &str,
	url_builder: Option<F>,
	headers: Option<HashMap<&str, &str>>,
	json_body: Option<serde_json::Value>,
) -> Result<Bytes> {
	// TODO: Get the API key from settings, fallback to constant, and error if missing
	let key = crate::constants::CURSEFORGE_API_KEY;

	let mut headers = headers.unwrap_or(HashMap::<&str, &str>::new());
	headers.insert("x-api-key", key);

	let url =
		&mut Url::parse(format!("{}{}", crate::constants::CURSEFORGE_API_URL, url).as_str())?;

	if let Some(url_builder) = url_builder {
		url_builder(url);
	}

	http::fetch_advanced(
		method,
		url.as_str(),
		None,
		json_body,
		Some(headers),
		None,
		&State::get().await?.fetch_semaphore,
	)
	.await
}

pub async fn fetch_url_builder<F: FnOnce(&mut Url)>(
	url: &str,
	url_builder: Option<F>,
) -> Result<Bytes> {
	fetch_advanced(Method::GET, url, url_builder, None, None).await
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
	type CFSearchResults = CFPaginatedData<Vec<CurseforgePackage>>;

	let results: CFSearchResults = serde_json::from_slice(
		&fetch_url_builder(
			"/v1/mods/search",
			Some(Box::new(move |url: &mut Url| {
				let params = &mut url.query_pairs_mut();

				params.append_pair(
					"gameId",
					crate::constants::CURSEFORGE_GAME_ID.to_string().as_str(),
				);
				params.append_pair("pageSize", limit.unwrap_or(10).to_string().as_str());
				params.append_pair("index", offset.unwrap_or(0).to_string().as_str());
				params.append_pair(
					"sortField",
					(SearchSort::Popularity as u8).to_string().as_str(),
				);
				params.append_pair("sortOrder", "desc");

				if let Some(query) = query {
					params.append_pair("searchFilter", query.as_str());
				}

				if let Some(game_versions) = game_versions {
					// Need to create a list e.g. ["1.16.5", "1.17.1"]
					params.append_pair(
						"gameVersions",
						format!(
							"[{}]",
							game_versions
								.into_iter()
								.map(|v| format!("\"{}\"", v))
								.collect::<Vec<String>>()
								.join(",")
						)
						.as_str(),
					);
				}

				if let Some(loaders) = loaders {
					params.append_pair(
						"modLoaderTypes",
						format!(
							"[{}]",
							loaders
								.into_iter()
								.map(|l| (CurseforgeLoader::from(l) as u32).to_string())
								.collect::<Vec<String>>()
								.join(",")
						)
						.as_str(),
					);
				}

				if let Some(package_types) = package_types {
					if package_types.len() == 0 {
						return;
					}

					if package_types.len() > 1 {
						tracing::warn!(
							"curseforge provider does not support multiple package types"
						);
					}

					let package_type = package_types.first().unwrap();

					params.append_pair(
						"classId",
						(CFPackageType::from(*package_type) as u32)
							.to_string()
							.as_str(),
					);
				}
			})),
		)
		.await?,
	)?;

	Ok(ProviderSearchResults {
		total: results.pagination.total_count,
		results: results
			.data
			.into_iter()
			.map(Into::into)
			.collect::<Vec<SearchResult>>(),
		provider: crate::package::content::Providers::Curseforge,
	})
}

pub async fn get(id: u32) -> Result<CurseforgePackage> {
	Ok(
		serde_json::from_slice::<CFData<_>>(&fetch(format!("/v1/mods/{}", id).as_str()).await?)?
			.data,
	)
}

pub async fn get_multiple(ids: &[u32]) -> Result<Vec<CurseforgePackage>> {
	let mut map = serde_json::Map::new();
	map.insert("modIds".to_string(), ids.to_vec().into());

	let json_body = serde_json::Value::Object(map);

	Ok(
		serde_json::from_slice::<CFData<_>>(
			&fetch_advanced(
				Method::POST,
				"/v1/mods",
				None::<fn(&mut Url)>,
				None,
				Some(json_body),
			).await?)?
			.data,
	)
}

pub async fn get_package_body(url: String) -> Result<String> {
	Ok(serde_json::from_slice::<CFData<_>>(&fetch(&url).await?)?.data)
}

pub async fn get_all_versions(
	project_id: &str,
	game_versions: Option<Vec<String>>,
	loaders: Option<Vec<Loader>>,
	page: Option<u32>,
	page_size: Option<u16>,
) -> Result<(Vec<ModFile>, Pagination)> {
	let data: CFPaginatedData<Vec<ModFile>> = serde_json::from_slice::<CFPaginatedData<_>>(
		&fetch_url_builder(
			format!("/v1/mods/{project_id}/files").as_str(),
			Some(Box::new(move |url: &mut Url| {
				if let Some(game_versions) = game_versions {
					if let Some(version) = game_versions.first() {
						url.query_pairs_mut().append_pair("gameVersion", version);
					}
				}

				if let Some(loader) = loaders {
					if let Some(loader) = loader.first() {
						url.query_pairs_mut().append_pair(
							"modLoaderType",
							(CurseforgeLoader::from(*loader) as u8).to_string().as_str(),
						);
					}
				}

				let page = page.unwrap_or(0);
				url.query_pairs_mut().append_pair("index", page.to_string().as_str());

				let page_size = page_size.unwrap_or(10);
				url.query_pairs_mut().append_pair("pageSize", page_size.to_string().as_str());
			})),
		)
		.await?,
	)?;

	Ok((data.data, data.pagination))
}

pub async fn get_versions(
	versions: Vec<String>,
) -> Result<Vec<ModFile>> {
	let json_body = json!({
		"fileIds": versions.iter().map(|v| v.parse::<u32>().unwrap()).collect::<Vec<u32>>()
	});

	Ok(serde_json::from_slice::<CFData<_>>(
		&fetch_advanced(
			Method::POST,
			format!("/v1/mods/files").as_str(),
			None::<fn(&mut Url)>,
			None,
			Some(json_body)
		)
		.await?,
	)?
	.data)
}

pub async fn get_versions_by_hashes(
	hashes: Vec<String>,
) -> Result<HashMap<String, ModFile>> {
	let body = json!({
		"fingerprints": hashes.into_iter().map(|hash| hash.parse::<u32>().unwrap_or_default()).collect::<Vec<u32>>()
	});

	#[derive(Debug, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	struct Response {
		exact_matches: Vec<FingerprintMatch>,
		exact_fingerprints: Vec<u32>
	}

	#[derive(Debug, Serialize, Deserialize)]
	struct FingerprintMatch {
		id: u32,
		file: ModFile
	}

	let files = serde_json::from_slice::<CFData<Response>>(
		&fetch_advanced(
			Method::POST,
			format!("/v1/fingerprints/{}", crate::constants::CURSEFORGE_GAME_ID).as_str(),
			None::<fn(&mut Url)>,
			None,
			Some(body),
		)
		.await?,
	)?;

	Ok(files.data.exact_matches.into_iter().map(|m| (m.file.file_fingerprint.to_string(), m.file)).collect())
}


