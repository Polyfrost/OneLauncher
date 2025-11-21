use chrono::{DateTime, Utc};
use onelauncher_entity::loader::GameLoader;
use onelauncher_entity::package::{PackageType, Provider};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::api::packages::categories::PackageCategories;

// Divisible by 4 and 3
pub const DEFAULT_LIMIT: usize = 24;

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Filters {
	pub game_versions: Option<Vec<String>>,
	pub loaders: Option<Vec<GameLoader>>,
	pub categories: Option<PackageCategories>,
	pub package_type: Option<PackageType>,
}

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
	#[default]
	Relevance,
	Downloads,
	Newest,
	Updated,
}

impl std::fmt::Display for Sort {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Relevance => "relevance",
				Self::Downloads => "downloads",
				Self::Newest => "newest",
				Self::Updated => "updated",
			}
		)
	}
}

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
	pub query: Option<String>,
	pub offset: Option<usize>,
	pub limit: Option<usize>,
	pub sort: Option<Sort>,
	pub filters: Option<Filters>,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
	pub project_id: String,
	pub package_type: PackageType,
	pub slug: String,
	pub author: String,
	pub title: String,
	pub description: String,
	pub categories: PackageCategories,
	pub loaders: Vec<GameLoader>,
	pub mc_versions: Vec<String>,
	pub downloads: usize,
	pub icon_url: String,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
	pub latest_version: String,
	pub license: Option<String>,
	pub client_side: PackageSide,
	pub server_side: PackageSide,
	/// List of URLs to images
	pub gallery: Vec<String>,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalPackage {
	pub name: String,
	pub url: String,
	pub sha1: String,
	pub size: usize,
	pub package_type: PackageType,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageOverrides {
	pub icon: Option<String>,
	pub name: Option<String>,
	pub authors: Option<Vec<String>>,
	pub description: Option<String>,
}

#[onelauncher_macro::specta]
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedPackage {
	pub id: String,
	pub slug: String,
	pub provider: Provider,
	pub package_type: PackageType,
	pub name: String,
	pub short_desc: String,
	pub body: ManagedPackageBody,
	/// Won't have all versions for some providers, like CurseForge.
	/// Try making a request to get the versions if needed
	pub version_ids: Vec<String>,
	pub mc_versions: Vec<String>,
	#[serde_as(as = "serde_with::VecSkipError<_>")]
	pub loaders: Vec<GameLoader>,
	pub icon_url: Option<String>,
	pub created: DateTime<Utc>,
	pub updated: DateTime<Utc>,
	pub client: PackageSide,
	pub server: PackageSide,
	pub categories: PackageCategories,
	pub license: Option<PackageLicense>,
	pub author: PackageAuthor,
	pub links: PackageLinks,
	pub status: PackageStatus,
	pub downloads: usize,
	#[serde_as(as = "serde_with::VecSkipError<_>")]
	pub gallery: Vec<PackageGallery>,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManagedPackageBody {
	Url(String),
	Raw(String),
}

impl std::fmt::Display for ManagedPackageBody {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				ManagedPackageBody::Url(_) => "Url",
				ManagedPackageBody::Raw(_) => "Raw",
			}
		)
	}
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedUser {
	pub id: String,
	pub username: String,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub avatar_url: Option<String>,
	#[serde(default)]
	pub bio: Option<String>,
	#[serde(default = "default_is_org_user")]
	pub is_organization_user: bool,
	#[serde(default)]
	pub role: Option<String>,
}

const fn default_is_org_user() -> bool {
	false
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedVersion {
	pub version_id: String,
	pub project_id: String,
	pub display_name: String,
	pub display_version: String,
	pub changelog: Option<String>,
	pub dependencies: Vec<ManagedVersionDependency>,
	pub mc_versions: Vec<String>,
	pub release_type: PackageReleaseType,
	pub loaders: Vec<GameLoader>,
	pub published: DateTime<Utc>,
	pub downloads: usize,
	pub files: Vec<ManagedVersionFile>,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedVersionDependency {
	pub version_id: Option<String>,
	pub project_id: Option<String>,
	pub file_name: Option<String>,
	pub dependency_type: PackageDependencyType,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedVersionFile {
	pub sha1: String,
	pub url: String,
	pub file_name: String,
	pub primary: bool,
	pub size: usize,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageAuthor {
	Team {
		team_id: String,
		org_id: Option<String>,
	},
	Users(Vec<ManagedUser>),
}

impl std::fmt::Display for PackageAuthor {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Team { team_id, org_id } => {
					if let Some(org_id) = org_id {
						format!("Team({} - {})", team_id, org_id)
					} else {
						format!("Team({})", team_id)
					}
				}
				Self::Users(users) => format!("Users({})", users.len()),
			}
		)
	}
}

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageSide {
	#[default]
	Unknown,
	Required,
	Optional,
	Unsupported,
}

/// <https://spdx.org/licenses/>
#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageLicense {
	pub id: String,
	pub name: String,
	pub url: Option<String>,
}

/// <https://docs.curseforge.com/rest-api/#tocS_ModLinks>
#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageLinks {
	pub website: Option<String>,
	pub issues: Option<String>,
	pub source: Option<String>,
	pub wiki: Option<String>,
	pub donation: Option<Vec<PackageDonationUrl>>,
	pub discord: Option<String>,
}

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageDonationUrl {
	pub id: PackageDonationPlatform,
	pub url: String,
}

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageGallery {
	pub url: String,
	pub thumbnail_url: String,
	pub title: Option<String>,
	pub description: Option<String>,
	pub featured: Option<bool>,
}

/// <https://api.modrinth.com/v2/tag/donation_platform>
#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageDonationPlatform {
	Patreon,
	#[serde(alias = "bmac")]
	BuyMeACoffee,
	PayPal,
	GitHub,
	#[serde(alias = "ko-fi")]
	KoFi,
	#[default]
	Other,
}

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageStatus {
	#[default]
	#[serde(alias = "approved")]
	Active,
	#[serde(alias = "archived", alias = "inactive")]
	Abandoned,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageDependencyType {
	Required,
	Optional,
	Embedded,
	Incompatible,
}

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageReleaseType {
	#[default]
	Release,
	Beta,
	Alpha,
}
