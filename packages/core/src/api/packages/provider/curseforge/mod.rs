use std::collections::HashMap;

use chrono::{DateTime, Utc};
use onelauncher_entity::loader::GameLoader;
use onelauncher_entity::package::{PackageType, Provider};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::serde_as;
use url::Url;

use crate::api::packages::PackageError;
use crate::api::packages::categories::ToProviderCategory;
use crate::api::packages::data::{
	ManagedPackage, ManagedPackageBody, ManagedUser, ManagedVersion, ManagedVersionDependency,
	ManagedVersionFile, PackageAuthor, PackageDependencyType, PackageGallery, PackageLinks,
	PackageReleaseType, PackageSide, PackageStatus, SearchQuery, SearchResult,
};
use crate::api::packages::provider::ProviderExt;
use crate::api::packages::provider::curseforge::categories::CurseForgeCategories;
use crate::error::LauncherResult;
use crate::store::Core;
use crate::utils::http;
use crate::utils::pagination::Paginated;

#[derive(Default)]
pub struct CurseForgeProviderImpl;

mod categories;

#[async_trait::async_trait]
impl ProviderExt for CurseForgeProviderImpl {
	async fn search(&self, filter: &SearchQuery) -> LauncherResult<Paginated<SearchResult>> {
		let results: CFPaginatedData<Vec<CFPackage>> = fetch_url_builder(
			"/mods/search",
			Some(|url: &mut Url| {
				let mut params = url.query_pairs_mut();

				// Always required parameters
				params.append_pair("gameId", &crate::constants::CURSEFORGE_GAME_ID.to_string());

				let package_type: CFPackageType = filter
					.filters
					.as_ref()
					.and_then(|f| f.package_type.clone())
					.unwrap_or_default()
					.into();
				params.append_pair("classId", &(package_type as u32).to_string());

				params.append_pair("pageSize", &filter.limit.unwrap_or(10).to_string());
				params.append_pair("index", &filter.offset.unwrap_or(0).to_string());
				params.append_pair("sortField", &(SearchSort::Popularity as u8).to_string());
				params.append_pair("sortOrder", "desc");

				// Optional: search query
				if let Some(query) = &filter.query {
					params.append_pair("searchFilter", query);
				}

				if let Some(filters) = &filter.filters {
					if let Some(game_versions) = &filters.game_versions {
						// Need to create a list e.g. ["1.16.5", "1.17.1"]
						let game_versions = format!(
							"[{}]",
							game_versions
								.iter()
								.map(|v| format!("\"{}\"", v))
								.collect::<Vec<String>>()
								.join(",")
						);

						params.append_pair("gameVersions", game_versions.as_str());
					}

					if let Some(loaders) = &filters.loaders {
						// Need to create a list e.g. [1, 2, 3]
						let loaders = format!(
							"[{}]",
							loaders
								.iter()
								.map(|l| (CFLoader::from(l.clone()) as u32).to_string())
								.collect::<Vec<String>>()
								.join(",")
						);

						params.append_pair("modLoaderTypes", loaders.as_str());
					}

					if let Some(categories) = &filters.categories {
						let categories = CurseForgeCategories::as_out(categories);

						if !categories.is_empty() {
							// Need to create a list e.g. [6, 12, 4471]
							let categories = format!(
								"[{}]",
								categories
									.iter()
									.map(u32::to_string)
									.collect::<Vec<String>>()
									.join(",")
							);

							params.append_pair("categoryIds", categories.as_str());
						}
					}
				}
			}),
		)
		.await?;

		Ok(Paginated {
			total: results.pagination.total_count,
			offset: results.pagination.index,
			limit: results.pagination.page_size,
			items: results.data.into_iter().map(SearchResult::from).collect(),
		})
	}

	async fn get(&self, slug: &str) -> LauncherResult<ManagedPackage> {
		Ok(fetch::<CFData<CFPackage>>(&format!("/mods/{slug}"))
			.await?
			.data
			.into())
	}

	async fn get_multiple(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedPackage>> {
		let body = serde_json::json!({
			"modIds": slugs.iter().filter_map(|s| s.parse::<u32>().ok()).collect::<Vec<u32>>(),
		});

		let results: CFData<Vec<CFPackage>> = fetch_advanced(
			Method::POST,
			"/mods",
			None::<fn(&mut Url)>,
			None,
			Some(body),
		)
		.await?;

		Ok(results.data.into_iter().map(ManagedPackage::from).collect())
	}

	async fn get_versions_by_hashes(
		&self,
		hashes: &[String],
	) -> LauncherResult<HashMap<String, ManagedVersion>> {
		let body = serde_json::json!({
			"fingerprints": hashes.into_iter().filter_map(|hash| hash.parse::<u32>().ok()).collect::<Vec<u32>>()
		});

		#[derive(Deserialize)]
		struct FingerprintMatch {
			// id: u32,
			file: CFModFile,
		}

		#[derive(Deserialize)]
		#[serde(rename_all = "camelCase")]
		struct Response {
			exact_matches: Vec<FingerprintMatch>,
			// exact_fingerprints: Vec<u32>
		}

		let results: CFData<Response> = fetch_advanced(
			Method::POST,
			&format!(
				"/mods/fingerprints/{}",
				crate::constants::CURSEFORGE_GAME_ID
			),
			None::<fn(&mut Url)>,
			None,
			Some(body),
		)
		.await?;

		Ok(results
			.data
			.exact_matches
			.into_iter()
			.map(|file| {
				(
					file.file.file_fingerprint.to_string(),
					ManagedVersion::from(file.file),
				)
			})
			.collect())
	}

	async fn get_version_by_hash(&self, hash: &str) -> LauncherResult<Option<ManagedVersion>> {
		let results = self.get_versions_by_hashes(&[hash.to_string()]).await?;
		Ok(results.values().next().cloned())
	}

	async fn get_users_from_author(
		&self,
		author: PackageAuthor,
	) -> LauncherResult<Vec<ManagedUser>> {
		match author {
			PackageAuthor::Users(users) => Ok(users),
			_ => Err(PackageError::UnsupportedAuthorType(author).into()),
		}
	}

	// async fn get_users(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedUser>> {
	// todo!("CurseForge get users not implemented yet")
	// }

	// async fn get_user(&self, slug: &str) -> LauncherResult<ManagedUser> {
	// 	todo!("CurseForge get user not implemented yet")
	// }

	async fn get_versions_paginated(
		&self,
		slug: &str,
		mc_version: Option<String>,
		loaders: Option<GameLoader>,
		offset: usize,
		limit: usize,
	) -> LauncherResult<Paginated<ManagedVersion>> {
		let results: CFPaginatedData<Vec<CFModFile>> = fetch_url_builder(
			&format!("/mods/{slug}/files"),
			Some(|url: &mut Url| {
				let mut params = url.query_pairs_mut();

				if let Some(mc_version) = mc_version {
					params.append_pair("gameVersion", &mc_version);
				}

				if let Some(loader) = loaders {
					params
						.append_pair("modLoaderType", &(CFLoader::from(loader) as u8).to_string());
				}

				let page = offset / limit;

				params.append_pair("pageSize", &limit.to_string());
				params.append_pair("index", &page.to_string());
			}),
		)
		.await?;

		Ok(Paginated {
			total: results.pagination.total_count,
			offset: results.pagination.index,
			limit: results.pagination.page_size,
			items: results.data.into_iter().map(ManagedVersion::from).collect(),
		})
	}

	async fn get_versions(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedVersion>> {
		let json_body = serde_json::json!({
			"fileIds": slugs.iter().filter_map(|v| v.parse::<u32>().ok()).collect::<Vec<u32>>()
		});

		let results: CFData<Vec<CFModFile>> = fetch_advanced(
			Method::POST,
			"/mods/files",
			None::<fn(&mut Url)>,
			None,
			Some(json_body),
		)
		.await?;

		Ok(results.data.into_iter().map(ManagedVersion::from).collect())
	}

	async fn get_body(&self, body: &ManagedPackageBody) -> LauncherResult<String> {
		match body {
			ManagedPackageBody::Raw(raw) => Ok(raw.clone()),
			ManagedPackageBody::Url(url) => {
				let body: String = fetch(url).await?;
				Ok(body)
			}
		}
	}
}

// MARK: fetch helpers

async fn fetch_advanced<T: DeserializeOwned, F: FnOnce(&mut Url)>(
	method: Method,
	url: &str,
	url_builder: Option<F>,
	headers: Option<HashMap<&str, &str>>,
	json_body: Option<serde_json::Value>,
) -> LauncherResult<T> {
	let core = Core::get();
	let key = core
		.curseforge_api_key
		.as_ref()
		.ok_or(PackageError::MissingApiKey(
			onelauncher_entity::package::Provider::CurseForge,
		))?;

	let mut headers = headers.unwrap_or(HashMap::<&str, &str>::new());
	headers.insert("x-api-key", key);

	let url = &mut Url::parse(format!("{}{}", crate::constants::CURSEFORGE_API_URL, url).as_str())?;

	if let Some(url_builder) = url_builder {
		url_builder(url);
	}

	http::fetch_json_advanced(method, url.as_str(), json_body, Some(headers), None, None).await
}

pub async fn fetch_url_builder<T: DeserializeOwned, F: FnOnce(&mut Url)>(
	url: &str,
	url_builder: Option<F>,
) -> LauncherResult<T> {
	fetch_advanced::<T, F>(Method::GET, url, url_builder, None, None).await
}

async fn fetch<T: DeserializeOwned>(url: &str) -> LauncherResult<T> {
	fetch_url_builder::<T, fn(&mut Url)>(url, None).await
}

// MARK: data structs
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
			CFPackageType::Mods => Self::Mod,
			CFPackageType::ModPack => Self::ModPack,
			CFPackageType::DataPack => Self::DataPack,
			CFPackageType::ResourcePack => Self::ResourcePack,
			CFPackageType::ShaderPack => Self::Shader,
		}
	}
}

impl From<PackageType> for CFPackageType {
	fn from(package_type: PackageType) -> Self {
		match package_type {
			PackageType::Mod => Self::Mods,
			PackageType::ModPack => Self::ModPack,
			PackageType::DataPack => Self::DataPack,
			PackageType::ResourcePack => Self::ResourcePack,
			PackageType::Shader => Self::ShaderPack,
		}
	}
}

/// Mapping of Curseforge supported loaders to their respective id
#[derive(Debug, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum CFLoader {
	#[default]
	Any = 0,
	Forge = 1,
	Cauldron = 2,
	LiteLoader = 3,
	Fabric = 4,
	Quilt = 5,
	NeoForge = 6,
}

impl From<String> for CFLoader {
	fn from(loader: String) -> Self {
		match loader.to_lowercase().as_str() {
			"forge" => CFLoader::Forge,
			"cauldron" => CFLoader::Cauldron,
			"liteloader" => CFLoader::LiteLoader,
			"fabric" => CFLoader::Fabric,
			"quilt" => CFLoader::Quilt,
			"neoforge" => CFLoader::NeoForge,
			_ => CFLoader::Any,
		}
	}
}

impl From<GameLoader> for CFLoader {
	fn from(loader: GameLoader) -> Self {
		match loader {
			GameLoader::Forge => CFLoader::Forge,
			GameLoader::Fabric | GameLoader::LegacyFabric => CFLoader::Fabric,
			GameLoader::Quilt => CFLoader::Quilt,
			GameLoader::NeoForge => CFLoader::NeoForge,
			_ => CFLoader::Any,
		}
	}
}

impl From<CFLoader> for GameLoader {
	fn from(loader: CFLoader) -> Self {
		match loader {
			CFLoader::Forge => Self::Forge,
			CFLoader::Fabric => Self::Fabric,
			CFLoader::Quilt => Self::Quilt,
			CFLoader::NeoForge => Self::NeoForge,
			_ => GameLoader::Vanilla,
		}
	}
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde_as]
#[serde(rename_all = "camelCase")]
pub struct CFPackage {
	pub id: usize,
	pub game_id: usize,
	#[serde(alias = "classId")]
	pub package_type: CFPackageType,
	pub name: String,
	pub slug: String,
	pub links: CFLinks,
	#[serde(default)]
	#[serde_as(as = "serde_with::VecSkipError<_>")]
	pub social_links: Vec<CFSocialLink>,
	pub summary: String,
	pub status: CFStatus,
	pub download_count: usize,
	#[serde(default)]
	pub is_featured: bool,
	pub categories: Vec<Category>,
	pub authors: Vec<CFAuthor>,
	pub logo: Option<CFModAsset>,
	pub screenshots: Vec<CFModAsset>,
	#[serde_as(as = "serde_with::VecSkipError<_>")]
	pub latest_files_indexes: Vec<CFLatestFileIndex>,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
	pub date_released: DateTime<Utc>,
	pub rating: Option<f32>,
}

impl From<CFPackage> for ManagedPackage {
	fn from(value: CFPackage) -> Self {
		let package_type: PackageType = value.package_type.into();

		let mut loaders: Vec<GameLoader> = Vec::new();
		let mut mc_versions: Vec<String> = Vec::new();
		let mut latest_version_ids: Vec<String> = Vec::new();

		for index in value.latest_files_indexes {
			mc_versions.push(index.game_version);
			latest_version_ids.push(index.file_id.to_string());

			if let Some(loader) = index
				.mod_loader
				.and_then(|loader| GameLoader::try_from(loader).ok())
			{
				loaders.push(loader);
			}
		}

		Self {
			provider: Provider::CurseForge,
			id: value.id.to_string(),
			slug: value.slug,
			name: value.name,
			short_desc: value.summary,
			body: ManagedPackageBody::Url(format!("/v1/mods/{}/description", value.id)),
			version_ids: latest_version_ids,
			mc_versions,
			loaders,
			icon_url: value.logo.map(|logo| logo.url),
			created: value.date_created,
			updated: value.date_modified,
			client: PackageSide::Unknown, // TODO: determine client side
			server: PackageSide::Unknown, // TODO: determine server side
			categories: CurseForgeCategories::to_list(
				&package_type,
				&value.categories.into_iter().map(|c| c.id).collect(),
			),
			package_type,
			license: None,
			author: PackageAuthor::Users(
				value
					.authors
					.into_iter()
					.map(|author| ManagedUser {
						id: author.id.to_string(),
						username: author.name,
						url: Some(author.url),
						is_organization_user: false,
						avatar_url: author.avatar_url,
						bio: None,
						role: None,
					})
					.collect(),
			),
			links: PackageLinks {
				source: value.links.source_url,
				issues: value.links.issues_url,
				wiki: value.links.wiki_url,
				donation: None, // we love curseforge

				discord: value
					.social_links
					.iter()
					.find(|l| l.link_type == CFSocialLinkType::Discord)
					.map(|l| l.url.clone()),
				website: value
					.social_links
					.iter()
					.find(|l| l.link_type == CFSocialLinkType::Website)
					.map(|l| l.url.clone()),

				..Default::default()
			},
			status: value.status.into(),
			downloads: value.download_count,
			gallery: value.screenshots.into_iter().map(Into::into).collect(),
		}
	}
}

impl From<CFPackage> for SearchResult {
	fn from(value: CFPackage) -> Self {
		let mc_versions: Vec<String> = value
			.latest_files_indexes
			.into_iter()
			.map(|index| index.game_version)
			.collect();

		let package_type: PackageType = value.package_type.into();

		Self {
			title: value.name,
			slug: value.slug,
			project_id: value.id.to_string(),
			author: value
				.authors
				.first()
				.and_then(|a| Some(a.name.clone()))
				.unwrap_or(String::from("Unknown")),
			description: value.summary,
			client_side: PackageSide::Unknown,
			server_side: PackageSide::Unknown,
			downloads: value.download_count,
			icon_url: value.logo.map_or(String::new(), |l| l.url),
			categories: CurseForgeCategories::to_list(
				&package_type,
				&value.categories.into_iter().map(|c| c.id).collect(),
			),
			package_type,
			mc_versions,
			date_created: value.date_created,
			date_modified: value.date_modified,
			gallery: value
				.screenshots
				.into_iter()
				.map(|asset| asset.url)
				.collect(),
			latest_version: "".to_string(), // TODO: figure out how to get this
			license: None,
		}
	}
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CFLatestFileIndex {
	pub game_version: String,
	pub file_id: usize,
	#[serde(rename = "filename")]
	pub file_name: String,
	// pub release_type: ModFileReleaseType,
	pub mod_loader: Option<CFLoader>,
}

#[derive(Default, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u64)]
pub enum CFStatus {
	#[default]
	Unknown = 0,
	New = 1,
	Approved = 4,
	Rejected = 5,
	Deleted = 9,
}

impl From<CFStatus> for PackageStatus {
	fn from(status: CFStatus) -> Self {
		match status {
			CFStatus::Unknown => Self::Active,
			CFStatus::New => Self::Active,
			CFStatus::Approved => Self::Active,
			CFStatus::Rejected => Self::Abandoned,
			CFStatus::Deleted => Self::Abandoned,
		}
	}
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CFLinks {
	pub website_url: Option<String>,
	pub wiki_url: Option<String>,
	pub issues_url: Option<String>,
	pub source_url: Option<String>,
}

// undocumented ??
#[derive(Default, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u64)]
pub enum CFSocialLinkType {
	None = 0,
	Mastodon = 1,
	Discord = 2,
	Website = 3,
	Facebook = 4,
	Twitter = 5,
	Instagram = 6,
	Patreon = 7,
	Twitch = 8,
	Reddit = 9,
	Youtube = 10,
	TikTok = 11,
	Pinterest = 12,
	Github = 13,
	BlueSky = 14,

	#[default]
	#[serde(other)]
	Unknown,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CFSocialLink {
	#[serde(rename = "type")]
	pub link_type: CFSocialLinkType,
	pub url: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
	pub id: u32,
	// pub game_id: u32,
	// pub name: String,
	pub slug: String,
	// pub url: String,
	// pub icon_url: String,
	// pub date_modified: String,
	// pub parent_category_id: u32,
	// pub display_index: u32,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CFAuthor {
	pub id: u32,
	pub name: String,
	pub url: String,
	pub avatar_url: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CFModAsset {
	pub id: u32,
	pub mod_id: u32,
	pub title: String,
	pub description: String,
	pub thumbnail_url: String,
	pub url: String,
}

impl From<CFModAsset> for PackageGallery {
	fn from(screenshot: CFModAsset) -> Self {
		Self {
			url: screenshot.url,
			thumbnail_url: screenshot.thumbnail_url,
			title: Some(screenshot.title),
			description: Some(screenshot.description),
			featured: None,
		}
	}
}

#[derive(Default, Serialize_repr, Deserialize_repr, Clone, Debug)]
#[repr(u8)]
pub enum ModFileReleaseType {
	#[default]
	Release = 1,
	Beta = 2,
	Alpha = 3,
}

impl From<ModFileReleaseType> for PackageReleaseType {
	fn from(value: ModFileReleaseType) -> Self {
		match value {
			ModFileReleaseType::Release => Self::Release,
			ModFileReleaseType::Beta => Self::Beta,
			ModFileReleaseType::Alpha => Self::Alpha,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CFModFile {
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
	pub file_length: usize,
	pub download_count: usize,
	pub file_size_on_disk: Option<usize>,
	pub download_url: String,
	pub game_versions: Vec<String>,
	pub sortable_game_versions: Vec<SortableGameVersion>,
	pub dependencies: Vec<CFDependency>,
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

impl From<CFModFile> for ManagedVersion {
	fn from(value: CFModFile) -> ManagedVersion {
		let mut hashes = HashMap::new();

		for hash in value.hashes {
			hashes.insert(hash.algo.name(), hash.value);
		}

		let mut mc_versions = Vec::new();
		let mut loaders = Vec::new();

		for version in value.sortable_game_versions {
			if version.game_version.is_empty() {
				// This is most likely a loader
				loaders.push(CFLoader::from(version.game_version_name));
			} else {
				mc_versions.push(version.game_version);
			}
		}

		let mut files = Vec::new();
		files.push(ManagedVersionFile {
			url: value.download_url,
			file_name: value.file_name.clone(),
			primary: true,
			size: value.file_size_on_disk.unwrap_or(value.file_length),
			sha1: hashes.get("sha1").cloned().unwrap_or_default(),
		});

		ManagedVersion {
			project_id: value.mod_id.to_string(),
			version_id: value.id.to_string(),
			display_name: value.display_name.clone(),
			loaders: loaders.into_iter().map(Into::into).collect(),
			mc_versions,
			changelog: None,
			display_version: value.file_name,
			release_type: value.release_type.into(),
			downloads: value.download_count,
			published: value.file_date,
			dependencies: value.dependencies.into_iter().map(Into::into).collect(),
			files,
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
		}
		.to_string()
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
pub struct CFDependency {
	pub mod_id: usize,
	pub relation_type: CFRelationType,
}

impl From<CFDependency> for ManagedVersionDependency {
	fn from(value: CFDependency) -> Self {
		Self {
			project_id: Some(value.mod_id.to_string()),
			dependency_type: value.relation_type.into(),
			file_name: None,
			version_id: None,
		}
	}
}

#[derive(Default, Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum CFRelationType {
	Embedded = 1,
	#[default]
	Optional = 2,
	Required = 3,
	Tool = 4,
	Incompatible = 5,
	Include = 6,
}

impl From<CFRelationType> for PackageDependencyType {
	fn from(value: CFRelationType) -> Self {
		match value {
			CFRelationType::Embedded => Self::Embedded,
			CFRelationType::Optional => Self::Optional,
			CFRelationType::Required => Self::Required,
			CFRelationType::Tool => Self::Optional,
			CFRelationType::Incompatible => Self::Incompatible,
			CFRelationType::Include => Self::Embedded,
		}
	}
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

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
	pub index: usize,
	pub page_size: usize,
	pub total_count: usize,
}
