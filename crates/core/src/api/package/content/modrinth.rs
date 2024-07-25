use crate::package::from::MODRINTH_API_URL;
use crate::store::{ManagedVersionFile, PackageFile, PackageSide};
use crate::utils::http::fetch;
use crate::{State, Result};
use crate::data::{ManagedPackage, ManagedVersion, PackageType};

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthPackage {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(rename = "client_side")]
    pub client_side: PackageSide,
    #[serde(rename = "server_side")]
    pub server_side: PackageSide,
    // #[serde(default)]
    // pub body: String,
    // #[serde(default)]
    // pub status: String,
    // #[serde(default)]
    // #[serde(rename = "requested_status")]
    // pub requested_status: String,
    #[serde(default)]
    #[serde(rename = "additional_categories")]
    pub additional_categories: Vec<String>,
    // #[serde(rename = "issues_url")]
    // pub issues_url: String,
    // #[serde(rename = "source_url")]
    // pub source_url: String,
    // #[serde(rename = "wiki_url")]
    // pub wiki_url: String,
    // #[serde(rename = "discord_url")]
    // pub discord_url: String,
    // #[serde(rename = "donation_urls")]
    // pub donation_urls: Vec<DonationUrl>,
    // #[serde(rename = "project_type")]
    // pub project_type: String,
    pub downloads: u32,
    #[serde(rename = "icon_url")]
    #[serde(default)]
    pub icon_url: String,
    // pub color: i64,
    // #[serde(rename = "thread_id")]
    // pub thread_id: String,
    // #[serde(rename = "monetization_status")]
    // pub monetization_status: String,
    pub id: String,
    // pub team: String,
    // #[serde(rename = "body_url")]
    // pub body_url: Value,
    // #[serde(rename = "moderator_message")]
    // pub moderator_message: Value,
    pub published: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    // pub approved: DateTime<Utc>,
    // pub queued: String,
    pub followers: u32,
    // pub license: License,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(rename = "game_versions")]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    // pub gallery: Vec<Gallery>,
}

impl From<ModrinthPackage> for ManagedPackage {
    fn from(value: ModrinthPackage) -> ManagedPackage {
        ManagedPackage {
            id: value.id,
            title: value.title,
            description: value.description,
            main: value.slug,
            versions: value.versions,
            game_versions: value.game_versions,
            loaders: value.loaders,
            icon_url: Some(value.icon_url),
            created: value.published,
            updated: value.updated,
            client: value.client_side,
            server: value.server_side,
            downloads: value.downloads,
            followers: value.followers,
            categories: value.categories,
            optional_categories: Some(value.additional_categories),
            uid: None,
            package_type: PackageType::Mod,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DonationUrl {
    pub id: String,
    pub platform: String,
    pub url: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct License {
    pub id: String,
    pub name: String,
    pub url: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gallery {
    pub url: String,
    pub featured: bool,
    pub title: String,
    pub description: String,
    pub created: String,
    pub ordering: i64,
}

macro_rules! format_url {
    ($($arg:tt)*) => {{
        format!("{}{}", MODRINTH_API_URL, format!($($arg)*))
    }};
}

pub async fn list() -> Result<Vec<ModrinthPackage>> {
    Ok(serde_json::from_slice(
        &fetch(
            format_url!("/projects_random?count=10").as_str(),
            None,
            &State::get().await?.fetch_semaphore
        ).await?
    )?)
}

pub async fn get(id: &str) -> Result<ModrinthPackage> {
    Ok(serde_json::from_slice(
        &fetch(
            format_url!("/project/{}", id).as_str(),
            None,
            &State::get().await?.fetch_semaphore
        ).await?
    )?)
}

#[derive(Serialize, Deserialize)]
struct SearchResults {
    hits: Vec<ModrinthPackage>,
}

pub async fn search(
    query: &str,
) -> Result<Vec<ModrinthPackage>> {
    // TODO: Fix date_created and date_updated inconsistency (published and updated)
    let response: SearchResults = serde_json::from_slice(
		&fetch(
            format_url!("/search?query={}", query).as_str(), 
            None, 
            &State::get().await?.fetch_semaphore
        ).await?,
	)?;

    Ok(response.hits)
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthVersion {
    #[serde(rename = "game_versions")]
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub id: String,
    #[serde(rename = "project_id")]
    pub project_id: String,
    #[serde(rename = "author_id")]
    pub author_id: String,
    pub featured: bool,
    pub name: String,
    #[serde(rename = "version_number")]
    pub version_number: String,
    pub changelog: String,
    #[serde(rename = "changelog_url")]
    pub changelog_url: Option<String>,
    #[serde(rename = "date_published")]
    pub date_published: DateTime<Utc>,
    pub downloads: u32,
    #[serde(rename = "version_type")]
    pub version_type: String,
    pub status: String,
    #[serde(rename = "requested_status")]
    pub requested_status: Value,
    pub files: Vec<File>,
    pub dependencies: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u32,
    #[serde(rename = "file_type")]
    pub file_type: Option<PackageFile>,
}

impl From<File> for ManagedVersionFile {
    fn from(value: File) -> Self {
        ManagedVersionFile {
            url: value.url,
            file_name: value.filename,
            primary: value.primary,
            size: value.size,
            file_type: value.file_type,
            hashes: Default::default(), // TODO
        }
    }
}

impl From<ModrinthVersion> for ManagedVersion {
    fn from(value: ModrinthVersion) -> Self {
        ManagedVersion {
            id: value.id,
            package_id: value.project_id,
            author: value.author_id,
            name: value.name,
        
            featured: value.featured,
            version_id: value.version_number,
            changelog: value.changelog,
            changelog_url: value.changelog_url,
        
            published: value.date_published,
            downloads: value.downloads,
            version_type: value.version_type,
        
            files: value.files.into_iter().map(|f| f.into()).collect(),
            deps: vec![], // TODO [`ManagedDependency`]?
            game_versions: value.game_versions,
            loaders: value.loaders,
        }
    }
}

pub async fn get_versions(
    project_id: &str,
) -> Result<Vec<ModrinthVersion>> {
    Ok(serde_json::from_slice(
        &fetch(
            format_url!("/project/{}/version", project_id).as_str(),
            None,
            &State::get().await?.fetch_semaphore
        ).await?
    )?)
}

pub async fn get_version(
    version_id: &str,
) -> Result<ModrinthVersion> {
    Ok(serde_json::from_slice(
        &fetch(
            format_url!("/version/{}", version_id).as_str(),
            None,
            &State::get().await?.fetch_semaphore
        ).await?
    )?)
}