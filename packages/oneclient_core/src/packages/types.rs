use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::domain::{ContentType, GameLoader, ProviderId};

pub const DEFAULT_PAGE_SIZE: usize = 24;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
	pub query: Option<String>,
	pub offset: Option<usize>,
	pub limit: Option<usize>,
	pub content_type: Option<ContentType>,
	pub game_versions: Option<Vec<String>>,
	pub loaders: Option<Vec<GameLoader>>,
	pub categories: Option<Vec<String>>,
	pub sort: Option<SearchSort>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SearchSort {
	#[default]
	Relevance,
	Downloads,
	Newest,
	Updated,
}

impl SearchSort {
	pub fn modrinth_index(self) -> &'static str {
		match self {
			Self::Relevance => "relevance",
			Self::Downloads => "downloads",
			Self::Newest => "newest",
			Self::Updated => "updated",
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
	pub offset: usize,
	pub limit: usize,
	pub total: usize,
	pub items: Vec<T>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSummary {
	pub id: String,
	pub slug: String,
	pub provider: ProviderId,
	pub content_type: ContentType,
	pub name: String,
	pub summary: String,
	pub author: String,
	pub icon_url: Option<String>,
	pub downloads: u64,
	pub created: DateTime<Utc>,
	pub updated: DateTime<Utc>,
	pub loaders: Vec<GameLoader>,
	pub game_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PackageBody {
	Url(String),
	Raw(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMember {
	pub name: String,
	pub role: String,
	pub url: Option<String>,
	pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalleryImage {
	pub url: String,
	pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetail {
	pub id: String,
	pub slug: String,
	pub provider: ProviderId,
	pub content_type: ContentType,
	pub name: String,
	pub summary: String,
	pub author: String,
	pub members: Vec<ProjectMember>,
	pub gallery: Vec<GalleryImage>,
	pub body: PackageBody,
	pub license: Option<String>,
	pub links: Vec<(String, String)>,
	pub version_ids: Vec<String>,
	pub game_versions: Vec<String>,
	pub loaders: Vec<GameLoader>,
	pub icon_url: Option<String>,
	pub created: DateTime<Utc>,
	pub updated: DateTime<Utc>,
	pub downloads: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSummary {
	pub version_id: String,
	pub project_id: String,
	pub name: String,
	pub version_number: String,
	pub published: DateTime<Utc>,
	pub release_type: ReleaseType,
	pub game_versions: Vec<String>,
	pub loaders: Vec<GameLoader>,
	pub downloads: u64,
	pub file_size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseType {
	#[default]
	Release,
	Beta,
	Alpha,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDetail {
	pub version_id: String,
	pub project_id: String,
	pub name: String,
	pub version_number: String,
	pub changelog: Option<String>,
	pub game_versions: Vec<String>,
	pub loaders: Vec<GameLoader>,
	pub published: DateTime<Utc>,
	pub downloads: u64,
	pub files: Vec<VersionFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFile {
	pub sha1: String,
	pub url: String,
	pub file_name: String,
	pub primary: bool,
	pub size: u64,
	pub fingerprint: Option<String>,
}

impl VersionDetail {
	pub fn primary_file(&self) -> Option<&VersionFile> {
		self.files.iter().find(|f| f.primary).or(self.files.first())
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalFile {
	pub name: String,
	pub url: String,
	pub sha1: String,
	pub size: u64,
	pub content_type: ContentType,
}

#[derive(Debug, Clone)]
pub struct CachedArtifact {
	pub hash: String,
	pub content_type: ContentType,
	pub path: std::path::PathBuf,
	pub file_name: String,
	pub size_bytes: Option<u64>,
	pub release: Option<ProviderReleaseInfo>,
}

#[derive(Debug, Clone)]
pub struct ProviderReleaseInfo {
	pub provider: ProviderId,
	pub project_id: String,
	pub version_id: String,
	pub display_name: String,
	pub display_version: String,
	pub mc_versions: Vec<String>,
	pub loaders: Vec<GameLoader>,
}

pub type VersionLookup = HashMap<String, VersionDetail>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedArtifactInfo {
	pub hash: String,
	pub cluster_file_name: String,
	pub enabled: bool,
	pub content_type: ContentType,
	pub file_name: String,
	pub project_id: Option<String>,
	pub display_name: Option<String>,
	pub display_version: Option<String>,
	pub provider: Option<ProviderId>,
}
