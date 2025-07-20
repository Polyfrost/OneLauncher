use super::ProviderExt;
use crate::api::packages::data::{
	ManagedPackage, ManagedUser, ManagedVersion, ManagedVersionDependency, ManagedVersionFile, PackageAuthor, PackageDependencyType, PackageDonationUrl, PackageGallery, PackageLicense, PackageLinks, PackageReleaseType, PackageSide, PackageStatus, SearchQuery, SearchResult, DEFAULT_LIMIT
};
use crate::error::LauncherResult;
use crate::utils::http;
use crate::utils::pagination::Paginated;
use chrono::{DateTime, Utc};
use onelauncher_entity::loader::GameLoader;
use onelauncher_entity::package::{PackageType, Provider};
use reqwest::Method;
use serde::Deserialize;
use serde_with::serde_as;
use url::Url;

macro_rules! url {
	($($arg:tt)*) => {
		format!("{}/v2{}", crate::constants::MODRINTH_API_URL, format!($($arg)*)).as_str()
	};
}

macro_rules! url_v3 {
	($($arg:tt)*) => {
		format!("{}/v3{}", crate::constants::MODRINTH_API_URL, format!($($arg)*)).as_str()
	};
}

#[derive(Default)]
pub struct ModrinthProviderImpl;

impl ModrinthProviderImpl {
	#[must_use]
	pub const fn new() -> Self {
		Self
	}
}

#[async_trait::async_trait]
impl ProviderExt for ModrinthProviderImpl {
	async fn search(&self, query: &SearchQuery) -> LauncherResult<Paginated<SearchResult>> {
		let mut url = Url::parse(url!("/search"))?;

		{
			let mut params = url.query_pairs_mut();

			params.append_pair("limit", &query.limit.unwrap_or(DEFAULT_LIMIT).to_string());
			params.append_pair("offset", &query.offset.unwrap_or(0).to_string());
			params.append_pair("query", query.query.clone().unwrap_or_default().as_ref());

			if let Some(filters) = &query.filters {
				let mut builder = FacetBuilder::builder();

				if let Some(categories) = &filters.categories {
					for category in categories {
						builder.and(Facet("categories".to_string(), FacetOperation::Eq, category.to_string()));
					}
				}

				if let Some(mc_versions) = &filters.game_versions {
					for mc_version in mc_versions {
						builder.and(Facet("versions".to_string(), FacetOperation::Eq, mc_version.to_string()));
					}
				}

				if let Some(package_types) = &filters.package_types {
					for package_type in package_types {
						builder.and(Facet("project_type".to_string(), FacetOperation::Eq, package_type_to_string(package_type)));
					}
				}

				if let Some(loaders) = &filters.loaders {
					for loader in loaders {
						builder.and(Facet("categories".to_string(), FacetOperation::Eq, loader.to_string()));
					}
				}

				params.append_pair("facets", &format!("[{}]", &builder.build()));
			}

			if let Some(sort) = &query.sort {
				params.append_pair("index", &sort.to_string());
			}
		}

		let url = url.as_str();

		#[derive(Deserialize)]
		struct SearchResults {
			pub hits: Vec<SearchResult>,
			pub offset: usize,
			pub limit: usize,
			pub total_hits: usize,
		}

		let response = http::fetch_json::<SearchResults>(Method::GET, url, None, None).await?;

		Ok(Paginated {
			offset: response.offset,
			limit: response.limit,
			total: response.total_hits,
			items: response.hits,
		})
	}

	async fn get(&self, slug: &str) -> LauncherResult<ManagedPackage> {
		Ok(
			http::fetch_json::<ModrinthPackage>(Method::GET, url!("/project/{}", slug), None, None)
				.await?
				.into(),
		)
	}

	async fn get_multiple(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedPackage>> {
		Ok(http::fetch_json::<Vec<ModrinthPackage>>(
			Method::GET,
			url!(
				"/projects?ids=[{}]",
				slugs
					.iter()
					.map(|id| format!("\"{id}\""))
					.collect::<Vec<String>>()
					.join(",")
			),
			None,
			None,
		)
		.await?
		.into_iter()
		.map(Into::into)
		.collect())
	}

	async fn get_versions_by_hashes(
		&self,
		hashes: &[String],
	) -> LauncherResult<Vec<ManagedVersion>> {
		let body = serde_json::json!({
			"hashes": hashes,
			"algorithm": "sha1"
		});

		Ok(http::fetch_json::<Vec<ModrinthVersion>>(
			Method::POST,
			url!("/version_files"),
			Some(body),
			None,
		)
		.await?
		.into_iter()
		.map(Into::into)
		.collect())
	}

	async fn get_version_by_hash(&self, hash: &str) -> LauncherResult<ManagedVersion> {
		Ok(http::fetch_json::<ModrinthVersion>(
			Method::GET,
			url!("/version_file/{hash}"),
			None,
			None,
		)
		.await?
		.into())
	}

	// async fn get_org_projects(&self, slug: &str) -> LauncherResult<Vec<ManagedPackage>> {
	// 	Ok(http::fetch_json(Method::GET, url_v3!("/organizations/{slug}/projects"), None, None).await?)
	// }

	async fn get_users_from_author(&self, author: PackageAuthor) -> LauncherResult<Vec<ManagedUser>> {
		let (team_id, org_id) = match author {
			PackageAuthor::Team { team_id, org_id } => (team_id, org_id),
			PackageAuthor::Users(users) => {
				return Ok(users);
			}
		};

		let mut users = Vec::new();

		if let Some(org_id) = org_id {
			#[derive(Deserialize)]
			struct Organization {
				pub id: String,
				pub name: String,
				#[serde(default)]
				pub icon_url: Option<String>,
				pub description: String,
				// pub team_id: String,
				// pub members: Vec<TeamMember>,
			}

			let organization = http::fetch_json::<Organization>(Method::GET, url_v3!("/organization/{org_id}"), None, None).await?;

			let org_user = ManagedUser {
				id: organization.id.clone(),
				username: organization.name,
				avatar_url: organization.icon_url,
				bio: Some(organization.description),
				url: Some(format!("{}/organization/{}", Provider::Modrinth.website(), organization.id)),
				is_organization_user: true,
				role: None,
			};

			users.push(org_user);
		}

		let project_members = http::fetch_json::<Vec<TeamMember>>(
			Method::GET,
			url!("/team/{}/members", team_id),
			None,
			None,
		)
		.await?;

		let project_members = project_members
			.into_iter()
			.map(Into::<ManagedUser>::into)
			.collect::<Vec<_>>();

		users.extend(project_members);
		Ok(users)
	}

	async fn get_users(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedUser>> {
		Ok(http::fetch_json::<Vec<ModrinthUser>>(
			Method::GET,
			url!("/users?ids=[{}]", &serde_json::to_string(&slugs)?),
			None,
			None,
		)
		.await?
		.into_iter()
		.map(Into::into)
		.collect())
	}

	async fn get_user(&self, slug: &str) -> LauncherResult<ManagedUser> {
		Ok(
			http::fetch_json::<ModrinthUser>(Method::GET, url!("/user/{slug}"), None, None)
				.await?
				.into(),
		)
	}

	async fn get_versions_paginated(
		&self,
		slug: &str,
		mc_versions: Option<Vec<String>>,
		loaders: Option<Vec<GameLoader>>,
		offset: usize,
		limit: usize,
	) -> LauncherResult<Paginated<ManagedVersion>> {
		let mut url = Url::parse(url!("/project/{slug}/version"))?;

		if let Some(mc_versions) = mc_versions {
			url.query_pairs_mut()
				.append_pair("game_versions", &serde_json::to_string(&mc_versions)?);
		}

		if let Some(loaders) = loaders {
			url.query_pairs_mut()
				.append_pair("loaders", &serde_json::to_string(&loaders)?);
		}

		// url.query_pairs_mut()
		// 	.append_pair("offset", &offset.to_string())
		// 	.append_pair("limit", &limit.to_string());

		let response =
			http::fetch_json::<Vec<ModrinthVersion>>(Method::GET, url.as_str(), None, None).await?;

		let paginated = response
			.into_iter()
			.skip(offset)
			.take(limit)
			.collect::<Vec<ModrinthVersion>>();

		Ok(Paginated {
			offset,
			limit,
			total: paginated.len(),
			items: paginated.into_iter().map(Into::into).collect(),
		})
	}

	async fn get_versions(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedVersion>> {
		Ok(http::fetch_json::<Vec<ModrinthVersion>>(
			Method::GET,
			url!(
				"/versions?ids=[{}]",
				slugs
					.iter()
					.map(|v| format!("\"{v}\""))
					.collect::<Vec<String>>()
					.join(",")
			),
			None,
			None,
		)
		.await?
		.into_iter()
		.map(Into::into)
		.collect())
	}
}

fn package_type_to_string(package_type: &PackageType) -> String {
	match package_type {
		PackageType::Mod => "mod",
		PackageType::ResourcePack => "resourcepack",
		PackageType::Shader => "shader",
		PackageType::DataPack => "datapack",
		PackageType::ModPack => "modpack",
	}
	.to_string()
}

#[serde_as]
#[derive(Deserialize)]
struct ModrinthPackage {
	// pub slug: String,
	pub title: String,
	#[serde(default)]
	pub description: String,
	#[serde(default)]
	pub categories: Vec<String>,
	pub client_side: PackageSide,
	pub server_side: PackageSide,
	#[serde(default)]
	pub body: String,
	// #[serde(default)]
	// #[serde(alias = "display_categories")]
	// pub additional_categories: Vec<String>,
	#[serde(default)]
	pub issues_url: Option<String>,
	#[serde(default)]
	pub source_url: Option<String>,
	#[serde(default)]
	pub wiki_url: Option<String>,
	#[serde(default)]
	pub discord_url: Option<String>,
	#[serde(default)]
	pub donation_urls: Vec<PackageDonationUrl>,
	pub project_type: PackageType,
	pub downloads: usize,
	#[serde(default)]
	pub icon_url: Option<String>,
	#[serde(alias = "project_id")]
	pub id: String,
	pub team: String,
	#[serde(default)]
	pub organization: Option<String>,
	#[serde(alias = "date_created")]
	pub published: DateTime<Utc>,
	#[serde(alias = "date_modified")]
	pub updated: DateTime<Utc>,
	// pub followers: u32,
	#[serde(default)]
	pub versions: Vec<String>,
	#[serde(default)]
	pub game_versions: Vec<String>,
	#[serde_as(as = "serde_with::VecSkipError<_>")]
	pub loaders: Vec<GameLoader>,
	#[serde(default)]
	pub license: Option<PackageLicense>,
	// #[serde(default)]
	pub gallery: Vec<ModrinthGallery>,
	#[serde(default)]
	pub status: PackageStatus,
}

impl From<ModrinthPackage> for ManagedPackage {
	fn from(value: ModrinthPackage) -> Self {
		Self {
			provider: Provider::Modrinth,
			id: value.id,
			package_type: value.project_type,
			name: value.title,
			short_desc: value.description,
			body: value.body,
			version_ids: value.versions,
			mc_versions: value.game_versions,
			loaders: value.loaders,
			icon_url: value.icon_url,
			created: value.published,
			updated: value.updated,
			client: value.client_side,
			server: value.server_side,
			categories: value.categories,
			license: value.license,
			author: PackageAuthor::Team {
				team_id: value.team,
				org_id: value.organization,
			},
			links: PackageLinks {
				source: value.source_url,
				discord: value.discord_url,
				issues: value.issues_url,
				wiki: value.wiki_url,
				donation: if value.donation_urls.is_empty() {
					None
				} else {
					Some(value.donation_urls)
				},
				..Default::default()
			},
			status: value.status,
			downloads: value.downloads,
			gallery: value.gallery.into_iter().map(Into::into).collect()
		}
	}
}

#[derive(Deserialize)]
struct ModrinthVersion {
	pub name: String,
	pub version_number: String,
	pub changelog: Option<String>,
	pub dependencies: Vec<ModrinthDependency>,
	pub game_versions: Vec<String>,
	pub version_type: PackageReleaseType,
	pub loaders: Vec<GameLoader>,
	// pub featured: bool,
	pub id: String,
	pub project_id: String,
	pub date_published: DateTime<Utc>,
	pub downloads: usize,
	pub files: Vec<ModrinthFile>,
}

impl From<ModrinthVersion> for ManagedVersion {
	fn from(value: ModrinthVersion) -> Self {
		Self {
			version_id: value.id,
			project_id: value.project_id,
			display_name: value.name,
			display_version: value.version_number,
			changelog: value.changelog,
			dependencies: value.dependencies.into_iter().map(Into::into).collect(),
			mc_versions: value.game_versions,
			release_type: value.version_type,
			loaders: value.loaders,
			published: value.date_published,
			downloads: value.downloads,
			files: value.files.into_iter().map(Into::into).collect(),
		}
	}
}

#[derive(Deserialize)]
struct ModrinthDependency {
	pub version_id: Option<String>,
	pub project_id: Option<String>,
	pub file_name: Option<String>,
	pub dependency_type: PackageDependencyType,
}

impl From<ModrinthDependency> for ManagedVersionDependency {
	fn from(value: ModrinthDependency) -> Self {
		Self {
			version_id: value.version_id,
			project_id: value.project_id,
			file_name: value.file_name,
			dependency_type: value.dependency_type,
		}
	}
}

#[derive(Deserialize)]
struct ModrinthFile {
	pub hashes: ModrinthFileHashes,
	pub url: String,
	pub filename: String,
	pub primary: bool,
	pub size: usize,
}

impl From<ModrinthFile> for ManagedVersionFile {
	fn from(value: ModrinthFile) -> Self {
		Self {
			sha1: value.hashes.sha1,
			url: value.url,
			file_name: value.filename,
			primary: value.primary,
			size: value.size,
		}
	}
}

#[derive(Deserialize)]
struct ModrinthFileHashes {
	pub sha1: String,
	// pub sha512: String,
}

#[derive(Deserialize)]
struct TeamMember {
	#[serde(default)]
	pub role: Option<String>,
	pub user: ModrinthUser,
}

impl From<TeamMember> for ManagedUser {
	fn from(value: TeamMember) -> Self {
		let mut user: Self = value.user.into();
		user.role = value.role;
		user
	}
}

#[derive(Deserialize)]
struct ModrinthUser {
	pub id: String,
	pub username: String,
	pub avatar_url: Option<String>,
	pub bio: Option<String>,
}

impl From<ModrinthUser> for ManagedUser {
	fn from(value: ModrinthUser) -> Self {
		Self {
			id: value.id.clone(),
			username: value.username,
			url: Some(format!(
				"{}user/{}",
				Provider::Modrinth.website(),
				value.id
			)),
			avatar_url: value.avatar_url,
			bio: value.bio,
			is_organization_user: false,
			role: None,
		}
	}
}

#[derive(Deserialize)]
pub struct ModrinthGallery {
	#[serde(rename = "raw_url")]
    pub url: String,
	#[serde(rename = "url")]
    pub thumbnail_url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub featured: Option<bool>
}

impl From<ModrinthGallery> for PackageGallery {
  fn from(value: ModrinthGallery) -> Self {
    Self {
      url: value.url,
      thumbnail_url: value.thumbnail_url,
	  title: value.title,
	  description: value.description,
	  featured: value.featured
    }
  }
}

#[allow(dead_code)]
enum FacetOperation {
	NotEq,
	LargeEq,
	Large,
	SmallEq,
	Small,
	Eq,
}

impl std::fmt::Display for FacetOperation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::NotEq => "!=",
			Self::LargeEq => ">=",
			Self::Large => ">",
			Self::SmallEq => "<=",
			Self::Small => "<",
			Self::Eq => "=",
		})
	}
}

struct Facet(pub String, pub FacetOperation, pub String);

impl std::fmt::Display for Facet {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(format!("{}{}{}", self.0, self.1, self.2).as_str())
	}
}

struct FacetBuilder {
	pub facets: Vec<Vec<Facet>>,
}

impl FacetBuilder {
	pub const fn builder() -> Self {
		Self { facets: vec![] }
	}

	pub fn and(&mut self, facet: Facet) -> &Self {
		self.facets.push(vec![facet]);
		self
	}

	#[allow(dead_code)]
	pub fn or(&mut self, facet: Facet) -> &Self {
		let mut last_facet = self.facets.pop().unwrap_or_default();
		last_facet.push(facet);
		self.facets.push(last_facet);
		self
	}

	pub fn build(&self) -> String {
		let mut builder: Vec<String> = vec![];

		for facet in &self.facets {
			let mut stringified = String::new();
			stringified.push('[');
			for (i, f) in facet.iter().enumerate() {
				if i != 0 {
					stringified.push(',');
				}
				stringified.push_str(format!("\"{f}\"").as_str());
			}
			stringified.push(']');

			builder.push(stringified);
		}

		builder.join(",")
	}
}
