use std::collections::HashMap;

use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::Deserialize;
use url::Url;

use super::PackageProvider;
use super::http::fetch_json;
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::file_identity::FileIdentity;
use crate::packages::types::{
    DependencyKind, InstalledPackage, PackageBody, Page, ProjectDetail, ProjectMember,
    ProjectSummary, ReleaseType, SearchFilters, SearchSort, VersionDependency, VersionDetail,
    VersionFile, VersionLookup, VersionSummary,
};
use oneclient_common::constants::HANGAR_API_URL;
use oneclient_common::domain::{ContentType, GameLoader, ProviderId};

const PLATFORM: &str = "PAPER";
const CATEGORIES: &[&str] = &[
    "admin_tools",
    "chat",
    "dev_tools",
    "economy",
    "gameplay",
    "games",
    "protection",
    "role_playing",
    "world_management",
    "misc",
];

pub struct HangarProvider;

const MAX_PAGE: usize = 25;

fn api(path: &str) -> String {
    format!("{HANGAR_API_URL}{path}")
}

#[derive(Deserialize)]
struct Paged<T> {
    pagination: Pagination,
    result: Vec<T>,
}

#[derive(Deserialize)]
struct Pagination {
    count: usize,
    limit: usize,
    offset: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HangarProject {
    id: u64,
    name: String,
    namespace: Namespace,
    #[serde(default)]
    stats: ProjectStats,
    #[serde(default)]
    description: String,
    created_at: DateTime<Utc>,
    last_updated: DateTime<Utc>,
    #[serde(default)]
    avatar_url: Option<String>,
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    supported_platforms: HashMap<String, Vec<String>>,
    #[serde(default)]
    main_page_content: Option<String>,
}

#[derive(Deserialize)]
struct Namespace {
    owner: String,
    slug: String,
}

#[derive(Deserialize, Default)]
struct ProjectStats {
    #[serde(default)]
    downloads: u64,
}

#[derive(Deserialize, Default)]
struct Settings {
    #[serde(default)]
    links: Vec<LinkSection>,
    #[serde(default)]
    license: Option<License>,
}

#[derive(Deserialize)]
struct LinkSection {
    #[serde(default)]
    links: Vec<Link>,
}

#[derive(Deserialize)]
struct Link {
    name: String,
    url: String,
}

#[derive(Deserialize)]
struct License {
    #[serde(default)]
    name: Option<String>,
}

impl HangarProject {
    fn icon_url(&self) -> String {
        self.avatar_url.clone().unwrap_or_else(|| {
            format!(
                "https://hangarcdn.papermc.io/avatars/project/{}.webp",
                self.id
            )
        })
    }

    fn game_versions(&mut self) -> Vec<String> {
        self.supported_platforms
            .remove(PLATFORM)
            .unwrap_or_default()
    }

    fn into_summary(mut self) -> ProjectSummary {
        ProjectSummary {
            id: self.id.to_string(),
            icon_url: Some(self.icon_url()),
            game_versions: self.game_versions(),
            slug: self.namespace.slug,
            provider: ProviderId::Hangar,
            content_type: ContentType::Mod,
            name: self.name,
            summary: self.description,
            author: self.namespace.owner,
            banner_url: None,
            downloads: self.stats.downloads,
            created: self.created_at,
            updated: self.last_updated,
            loaders: Vec::new(),
        }
    }

    fn into_detail(mut self) -> ProjectDetail {
        let owner = self.namespace.owner.clone();
        ProjectDetail {
            id: self.id.to_string(),
            icon_url: Some(self.icon_url()),
            game_versions: self.game_versions(),
            slug: self.namespace.slug,
            provider: ProviderId::Hangar,
            content_type: ContentType::Mod,
            name: self.name,
            summary: self.description,
            members: vec![ProjectMember {
                url: Some(format!("{}{owner}", ProviderId::Hangar.website())),
                name: owner.clone(),
                role: "Owner".to_string(),
                avatar_url: None,
            }],
            author: owner,
            gallery: Vec::new(),
            body: PackageBody::raw(self.main_page_content.unwrap_or_default()),
            license: self.settings.license.and_then(|l| l.name),
            links: self
                .settings
                .links
                .into_iter()
                .flat_map(|section| section.links)
                .map(|link| (link.name, link.url))
                .collect(),
            version_ids: Vec::new(),
            loaders: Vec::new(),
            created: self.created_at,
            updated: self.last_updated,
            downloads: self.stats.downloads,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HangarVersion {
    id: u64,
    project_id: u64,
    name: String,
    #[serde(default)]
    description: Option<String>,
    created_at: DateTime<Utc>,
    #[serde(default)]
    stats: VersionStats,
    channel: Channel,
    #[serde(default)]
    downloads: HashMap<String, Download>,
    #[serde(default)]
    plugin_dependencies: HashMap<String, Vec<PluginDependency>>,
    #[serde(default)]
    platform_dependencies: HashMap<String, Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct VersionStats {
    #[serde(default)]
    total_downloads: u64,
}

#[derive(Deserialize)]
struct Channel {
    name: String,
    #[serde(default)]
    flags: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Download {
    #[serde(default)]
    file_info: Option<FileInfo>,
    #[serde(default)]
    download_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileInfo {
    name: String,
    size_bytes: u64,
    sha256_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginDependency {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    project_id: Option<u64>,
    #[serde(default)]
    required: bool,
}

fn release_type(channel: &Channel) -> ReleaseType {
    match (channel.name.to_ascii_lowercase().as_str(), &channel.flags) {
        ("alpha", _) => ReleaseType::Alpha,
        (_, Some(flags)) if !flags.iter().any(|f| f == "UNSTABLE") => ReleaseType::Release,
        ("release", None) => ReleaseType::Release,
        _ => ReleaseType::Beta,
    }
}

impl HangarVersion {
    fn file(&mut self) -> Option<VersionFile> {
        let download = self.downloads.remove(PLATFORM)?;
        let info = download.file_info?;
        Some(VersionFile {
            sha1: String::new(),
            url: download.download_url?,
            file_name: info.name,
            primary: true,
            size: info.size_bytes,
            fingerprint: None,
            sha256: Some(info.sha256_hash),
        })
    }

    fn dependencies(&mut self) -> Vec<VersionDependency> {
        self.plugin_dependencies
            .remove(PLATFORM)
            .unwrap_or_default()
            .into_iter()
            .map(|dep| VersionDependency {
                project_id: dep.project_id.map(|id| id.to_string()),
                version_id: None,
                kind: if dep.required {
                    DependencyKind::Required
                } else {
                    DependencyKind::Optional
                },
                name: dep.name,
            })
            .collect()
    }

    fn into_detail(mut self) -> VersionDetail {
        let file = self.file();
        VersionDetail {
            version_id: self.id.to_string(),
            project_id: self.project_id.to_string(),
            name: self.name.clone(),
            version_number: self.name.clone(),
            changelog: self.description.take(),
            dependencies: self.dependencies(),
            game_versions: self
                .platform_dependencies
                .remove(PLATFORM)
                .unwrap_or_default(),
            loaders: Vec::new(),
            published: self.created_at,
            downloads: self.stats.total_downloads,
            files: file.into_iter().collect(),
        }
    }

    fn into_summary(mut self) -> VersionSummary {
        let size = self
            .downloads
            .get(PLATFORM)
            .and_then(|d| d.file_info.as_ref())
            .map_or(0, |info| info.size_bytes);
        VersionSummary {
            version_id: self.id.to_string(),
            project_id: self.project_id.to_string(),
            name: self.name.clone(),
            version_number: self.name.clone(),
            release_type: release_type(&self.channel),
            dependencies: self.dependencies(),
            game_versions: self
                .platform_dependencies
                .remove(PLATFORM)
                .unwrap_or_default(),
            loaders: Vec::new(),
            published: self.created_at,
            downloads: self.stats.total_downloads,
            file_size: size,
        }
    }
}

fn sort_param(sort: SearchSort) -> Option<&'static str> {
    match sort {
        SearchSort::Relevance => None,
        SearchSort::Downloads => Some("-downloads"),
        SearchSort::Newest => Some("-newest"),
        SearchSort::Updated => Some("-updated"),
    }
}

async fn fetch_project(project_id: &str, ctx: &ContentCtx) -> ContentResult<HangarProject> {
    let url = api(&format!("/projects/{project_id}"));
    Ok(fetch_json(&ctx.net, Method::GET, url.as_str(), None).await?)
}

#[async_trait::async_trait]
impl PackageProvider for HangarProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Hangar
    }

    #[tracing::instrument(level = "debug", skip(self, filters, ctx))]
    async fn search(
        &self,
        filters: &SearchFilters,
        ctx: &ContentCtx,
    ) -> ContentResult<Page<ProjectSummary>> {
        let mut url = Url::parse(&api("/projects"))?;
        {
            let mut params = url.query_pairs_mut();
            let limit = filters
                .limit
                .unwrap_or(super::super::types::DEFAULT_PAGE_SIZE)
                .min(MAX_PAGE);
            params.append_pair("limit", &limit.to_string());
            params.append_pair("offset", &filters.offset.unwrap_or(0).to_string());
            params.append_pair("platform", PLATFORM);
            let query = filters.normalized_query();
            if !query.is_empty() {
                params.append_pair("query", &query);
            }
            if let Some(version) = filters.game_versions.as_ref().and_then(|v| v.first()) {
                params.append_pair("version", version);
            }
            for category in filters.categories.iter().flatten() {
                params.append_pair("category", category);
            }
            if let Some(sort) = filters.sort.and_then(sort_param) {
                params.append_pair("sort", sort);
            }
        }

        let page: Paged<HangarProject> =
            fetch_json(&ctx.net, Method::GET, url.as_str(), None).await?;
        Ok(Page {
            offset: page.pagination.offset,
            limit: page.pagination.limit,
            total: page.pagination.count,
            items: page
                .result
                .into_iter()
                .map(HangarProject::into_summary)
                .collect(),
        })
    }

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    async fn get_project(
        &self,
        project_id: &str,
        ctx: &ContentCtx,
    ) -> ContentResult<ProjectDetail> {
        Ok(fetch_project(project_id, ctx).await?.into_detail())
    }

    async fn list_categories(
        &self,
        _content_type: ContentType,
        _ctx: &ContentCtx,
    ) -> ContentResult<Vec<String>> {
        Ok(CATEGORIES.iter().map(|c| c.to_string()).collect())
    }

    #[tracing::instrument(level = "debug", skip(self, project_ids, ctx))]
    async fn get_projects(
        &self,
        project_ids: &[String],
        ctx: &ContentCtx,
    ) -> ContentResult<Vec<ProjectDetail>> {
        let fetched =
            futures_util::future::join_all(project_ids.iter().map(|id| fetch_project(id, ctx)))
                .await;
        Ok(fetched
            .into_iter()
            .filter_map(Result::ok)
            .map(HangarProject::into_detail)
            .collect())
    }

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    async fn list_versions(
        &self,
        project_id: &str,
        game_version: Option<&str>,
        _loader: Option<GameLoader>,
        offset: usize,
        limit: usize,
        ctx: &ContentCtx,
    ) -> ContentResult<Page<VersionSummary>> {
        let mut url = Url::parse(&api(&format!("/projects/{project_id}/versions")))?;
        {
            let mut params = url.query_pairs_mut();
            params.append_pair("limit", &limit.min(MAX_PAGE).to_string());
            params.append_pair("offset", &offset.to_string());
            params.append_pair("platform", PLATFORM);
            if let Some(version) = game_version {
                params.append_pair("platformVersion", version);
            }
        }
        let page: Paged<HangarVersion> =
            fetch_json(&ctx.net, Method::GET, url.as_str(), None).await?;
        Ok(Page {
            offset: page.pagination.offset,
            limit: page.pagination.limit,
            total: page.pagination.count,
            items: page
                .result
                .into_iter()
                .map(HangarVersion::into_summary)
                .collect(),
        })
    }

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    async fn get_version(
        &self,
        project_id: &str,
        version_id: &str,
        ctx: &ContentCtx,
    ) -> ContentResult<VersionDetail> {
        let url = api(&format!("/projects/{project_id}/versions/{version_id}"));
        let version: HangarVersion = fetch_json(&ctx.net, Method::GET, url.as_str(), None).await?;
        Ok(version.into_detail())
    }

    #[tracing::instrument(level = "debug", skip(self, version_ids, ctx))]
    async fn get_versions(
        &self,
        version_ids: &[String],
        ctx: &ContentCtx,
    ) -> ContentResult<Vec<VersionDetail>> {
        let fetched = futures_util::future::join_all(version_ids.iter().map(|id| async move {
            let url = api(&format!("/versions/{id}"));
            fetch_json::<HangarVersion>(&ctx.net, Method::GET, url.as_str(), None).await
        }))
        .await;
        Ok(fetched
            .into_iter()
            .filter_map(Result::ok)
            .map(HangarVersion::into_detail)
            .collect())
    }

    async fn lookup_versions(
        &self,
        _identities: &[FileIdentity],
        _ctx: &ContentCtx,
    ) -> ContentResult<VersionLookup> {
        Ok(HashMap::new())
    }

    async fn latest_for_game_version(
        &self,
        _packages: &[InstalledPackage],
        _mc_version: &str,
        _loader: GameLoader,
        _ctx: &ContentCtx,
    ) -> ContentResult<HashMap<String, VersionDetail>> {
        Ok(HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERSION: &str = r#"{
        "createdAt": "2026-11-07T09:38:30.877163Z",
        "id": 33780,
        "projectId": 12,
        "name": "5.12.3-SNAPSHOT+669",
        "description": "notes",
        "stats": {"totalDownloads": 7, "platformDownloads": {}},
        "channel": {"name": "Snapshot"},
        "downloads": {
            "PAPER": {
                "fileInfo": {"name": "ViaBackwards.jar", "sizeBytes": 10, "sha256Hash": "ab"},
                "externalUrl": null,
                "downloadUrl": "https://hangarcdn.papermc.io/x/ViaBackwards.jar"
            },
            "VELOCITY": {"fileInfo": null, "externalUrl": "https://example.com", "downloadUrl": null}
        },
        "pluginDependencies": {"PAPER": [
            {"name": "ViaRewind", "projectId": 112, "required": false},
            {"name": "ViaVersion", "projectId": 31, "required": true},
            {"name": "Vault", "projectId": null, "required": true, "externalUrl": "https://x"}
        ]},
        "platformDependencies": {"PAPER": ["1.21.1"], "VELOCITY": ["3.4"]}
    }"#;

    #[test]
    fn paper_version_maps_to_a_sha256_file_and_hangar_dependencies() {
        let version: HangarVersion = serde_json::from_str(VERSION).unwrap();
        let detail = version.into_detail();
        let file = detail.primary_file().unwrap();
        assert!(file.sha1.is_empty());
        assert_eq!(file.sha256.as_deref(), Some("ab"));
        assert_eq!(file.file_name, "ViaBackwards.jar");
        assert_eq!(detail.version_id, "33780");
        assert_eq!(detail.project_id, "12");
        assert_eq!(detail.game_versions, ["1.21.1"]);
        assert_eq!(
            detail.dependencies,
            [
                VersionDependency {
                    project_id: Some("112".into()),
                    version_id: None,
                    kind: DependencyKind::Optional,
                    name: Some("ViaRewind".into()),
                },
                VersionDependency {
                    project_id: Some("31".into()),
                    version_id: None,
                    kind: DependencyKind::Required,
                    name: Some("ViaVersion".into()),
                },
                VersionDependency {
                    project_id: None,
                    version_id: None,
                    kind: DependencyKind::Required,
                    name: Some("Vault".into()),
                },
            ]
        );
    }

    #[test]
    fn channel_flags_decide_the_release_type() {
        let channel = |name: &str, flags: Option<&[&str]>| Channel {
            name: name.into(),
            flags: flags.map(|f| f.iter().map(|s| s.to_string()).collect()),
        };
        assert!(matches!(release_type(&channel("Snapshot", Some(&["PINNED"][..]))), ReleaseType::Release));
        assert!(matches!(release_type(&channel("Release", Some(&["UNSTABLE"][..]))), ReleaseType::Beta));
        assert!(matches!(release_type(&channel("Alpha", Some(&[][..]))), ReleaseType::Alpha));
        assert!(matches!(release_type(&channel("Release", None)), ReleaseType::Release));
        assert!(matches!(release_type(&channel("Snapshot", None)), ReleaseType::Beta));
    }
}
