use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::Deserialize;
use url::Url;

use serde::de::DeserializeOwned;

use super::PackageProvider;
use super::http::fetch_json_with_headers;
use crate::LauncherResult;
use crate::api_config::modrinth_headers;
use crate::constants::MODRINTH_API_URL;
use crate::http::{RequestClient, RequestError};
use crate::packages::domain::{ContentType, GameLoader, ProviderId};
use crate::packages::file_identity::FileIdentity;
use crate::packages::types::{
    GalleryImage, PackageBody, Page, ProjectDetail, ProjectMember, ProjectSummary, ReleaseType,
    SearchFilters, VersionDetail, VersionFile, VersionLookup, VersionSummary,
};
use crate::state::LauncherServices;

pub struct ModrinthProvider;

fn v2(path: &str) -> String {
    format!("{MODRINTH_API_URL}/v2{path}")
}

async fn fetch_json<T: DeserializeOwned>(
    client: &RequestClient,
    method: Method,
    url: impl reqwest::IntoUrl,
    body: Option<serde_json::Value>,
) -> Result<T, RequestError> {
    fetch_json_with_headers(client, method, url, body, &modrinth_headers()).await
}

#[async_trait::async_trait]
impl PackageProvider for ModrinthProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Modrinth
    }

    async fn search(
        &self,
        filters: &SearchFilters,
        services: &LauncherServices,
    ) -> LauncherResult<Page<ProjectSummary>> {
        let mut url = Url::parse(&v2("/search"))?;
        {
            let limit = filters
                .limit
                .unwrap_or(super::super::types::DEFAULT_PAGE_SIZE);

            let offset = filters.offset.unwrap_or(0);
            let mut params = url.query_pairs_mut();
            params.append_pair("limit", &limit.to_string());
            params.append_pair("offset", &offset.to_string());
            params.append_pair("query", filters.query.as_deref().unwrap_or_default());

            let mut groups: Vec<Vec<String>> = Vec::new();
            if let Some(content_type) = filters.content_type {
                groups.push(vec![format!("project_type:{}", content_type.modrinth_type())]);
            }
            if let Some(versions) = &filters.game_versions {
                let group: Vec<String> = versions.iter().map(|v| format!("versions:{v}")).collect();
                if !group.is_empty() {
                    groups.push(group);
                }
            }
            if let Some(loaders) = &filters.loaders {
                let group: Vec<String> = loaders
                    .iter()
                    .map(|l| format!("categories:{}", l.modrinth_name()))
                    .collect();
                if !group.is_empty() {
                    groups.push(group);
                }
            }
            if let Some(categories) = &filters.categories {
                for category in categories {
                    groups.push(vec![format!("categories:{category}")]);
                }
            }
            if !groups.is_empty() {
                params.append_pair("facets", &serde_json::to_string(&groups).unwrap_or_default());
            }

            if let Some(sort) = filters.sort {
                params.append_pair("index", sort.modrinth_index());
            }
        }

        #[derive(Deserialize)]
        struct Response {
            hits: Vec<Hit>,
            offset: usize,
            limit: usize,
            total_hits: usize,
        }

        #[derive(Deserialize)]
        struct Hit {
            project_id: String,
            project_type: String,
            slug: String,
            author: String,
            title: String,
            description: String,
            #[serde(default)]
            categories: Vec<String>,
            #[serde(default, alias = "versions")]
            game_versions: Vec<String>,
            downloads: u64,
            icon_url: Option<String>,
            date_created: DateTime<Utc>,
            date_modified: DateTime<Utc>,
        }

        let response: Response =
            fetch_json(&services.requester, Method::GET, url.as_str(), None).await?;

        Ok(Page {
            offset: response.offset,
            limit: response.limit,
            total: response.total_hits,
            items: response
                .hits
                .into_iter()
                .map(|h| ProjectSummary {
                    id: h.project_id,
                    slug: h.slug,
                    provider: ProviderId::Modrinth,
                    content_type: parse_content_type(&h.project_type),
                    name: h.title,
                    summary: h.description,
                    author: h.author,
                    icon_url: h.icon_url,
                    downloads: h.downloads,
                    created: h.date_created,
                    updated: h.date_modified,
                    loaders: h
                        .categories
                        .iter()
                        .filter_map(|c| GameLoader::from_str(c).ok())
                        .collect(),
                    game_versions: h.game_versions,
                })
                .collect(),
        })
    }

    async fn get_project(
        &self,
        project_id: &str,
        services: &LauncherServices,
    ) -> LauncherResult<ProjectDetail> {
        let project_url = v2(&format!("/project/{project_id}"));
        let members_url = v2(&format!("/project/{project_id}/members"));
        let project_fut =
            fetch_json::<ModrinthProject>(&services.requester, Method::GET, &project_url, None);
        let members_fut =
            fetch_json::<Vec<ModrinthMember>>(&services.requester, Method::GET, &members_url, None);
        let (raw, members) = tokio::join!(project_fut, members_fut);

        let mut detail = raw?.into_detail();
        if let Ok(members) = members {
            apply_modrinth_members(&mut detail, members);
        }
        Ok(detail)
    }

    async fn list_categories(
        &self,
        content_type: ContentType,
        services: &LauncherServices,
    ) -> LauncherResult<Vec<String>> {
        #[derive(Deserialize)]
        struct Tag {
            name: String,
            project_type: String,
        }
        let tags: Vec<Tag> =
            fetch_json(&services.requester, Method::GET, &v2("/tag/category"), None).await?;
        let want = content_type.modrinth_type();
        Ok(tags
            .into_iter()
            .filter(|t| t.project_type == want)
            .map(|t| t.name)
            .collect())
    }

    async fn get_projects(
        &self,
        project_ids: &[String],
        services: &LauncherServices,
    ) -> LauncherResult<Vec<ProjectDetail>> {
        if project_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = project_ids
            .iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(",");
        
        let raw: Vec<ModrinthProject> = fetch_json(
            &services.requester,
            Method::GET,
            &v2(&format!("/projects?ids=[{ids}]")),
            None,
        )
        .await?;

        let team_ids: Vec<String> = raw
            .iter()
            .map(|p| p.team.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        
        let mut members_by_team = fetch_modrinth_team_members(&services.requester, &team_ids)
            .await
            .unwrap_or_default();

        Ok(raw
            .into_iter()
            .map(|project| {
                let team_id = project.team.clone();
                let mut detail = project.into_detail();
                if let Some(members) = members_by_team.remove(&team_id) {
                    apply_modrinth_members(&mut detail, members);
                }
                detail
            })
            .collect())
    }

    async fn list_versions(
        &self,
        project_id: &str,
        game_version: Option<&str>,
        loader: Option<GameLoader>,
        offset: usize,
        limit: usize,
        services: &LauncherServices,
    ) -> LauncherResult<Page<VersionSummary>> {
        let mut url = Url::parse(&v2(&format!("/project/{project_id}/version")))?;
        if let Some(v) = game_version {
            url.query_pairs_mut()
                .append_pair("game_versions", &format!("[\"{v}\"]"));
        }
        if let Some(loader) = loader {
            url.query_pairs_mut()
                .append_pair("loaders", &format!("[\"{}\"]", loader.modrinth_name()));
        }

        let versions: Vec<ModrinthVersion> =
            fetch_json(&services.requester, Method::GET, url.as_str(), None).await?;

        let total = versions.len();
        let items = versions
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(VersionSummary::from)
            .collect();

        Ok(Page {
            offset,
            limit,
            total,
            items,
        })
    }

    async fn get_version(
        &self,
        _project_id: &str,
        version_id: &str,
        services: &LauncherServices,
    ) -> LauncherResult<VersionDetail> {
        let raw: ModrinthVersion = fetch_json(
            &services.requester,
            Method::GET,
            &v2(&format!("/version/{version_id}")),
            None,
        )
        .await?;
        Ok(raw.into())
    }

    async fn get_versions(
        &self,
        version_ids: &[String],
        services: &LauncherServices,
    ) -> LauncherResult<Vec<VersionDetail>> {
        if version_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = version_ids
            .iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(",");
        let raw: Vec<ModrinthVersion> = fetch_json(
            &services.requester,
            Method::GET,
            &v2(&format!("/versions?ids=[{ids}]")),
            None,
        )
        .await?;
        Ok(raw.into_iter().map(Into::into).collect())
    }

    async fn lookup_versions(
        &self,
        identities: &[FileIdentity],
        services: &LauncherServices,
    ) -> LauncherResult<VersionLookup> {
        let sha1_hashes: Vec<String> = identities
            .iter()
            .map(|id| id.sha1.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        if sha1_hashes.is_empty() {
            return Ok(HashMap::new());
        }

        let body = serde_json::json!({
            "hashes": sha1_hashes,
            "algorithm": "sha1"
        });

        let fetched: HashMap<String, ModrinthVersion> = fetch_json(
            &services.requester,
            Method::POST,
            &v2("/version_files"),
            Some(body),
        )
        .await?;

        let mut out = HashMap::new();
        for identity in identities {
            if let Some(version) = fetched.get(&identity.sha1) {
                out.insert(identity.sha1.clone(), version.clone().into());
            }
        }

        Ok(out)
    }
}

async fn fetch_modrinth_team_members(
    client: &RequestClient,
    team_ids: &[String],
) -> Result<HashMap<String, Vec<ModrinthMember>>, RequestError> {
    if team_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids = team_ids
        .iter()
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(",");
    let teams: Vec<Vec<ModrinthMember>> = fetch_json(
        client,
        Method::GET,
        &v2(&format!("/teams?ids=[{ids}]")),
        None,
    )
    .await?;
    Ok(teams
        .into_iter()
        .filter_map(|members| {
            let team_id = members.first()?.team_id.clone();
            Some((team_id, members))
        })
        .collect())
}

#[derive(Deserialize)]
struct ModrinthProject {
    id: String,
    slug: String,
    team: String,
    project_type: String,
    title: String,
    description: String,
    body: Option<String>,
    body_url: Option<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    icon_url: Option<String>,
    published: DateTime<Utc>,
    updated: DateTime<Utc>,
    downloads: u64,
    license: Option<ModrinthLicense>,
    source_url: Option<String>,
    issues_url: Option<String>,
    wiki_url: Option<String>,
    discord_url: Option<String>,
    #[serde(default)]
    gallery: Vec<ModrinthGalleryItem>,
}

#[derive(Deserialize)]
struct ModrinthGalleryItem {
    url: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Deserialize)]
struct ModrinthMember {
    #[serde(default)]
    team_id: String,
    role: String,
    user: ModrinthUser,
}

#[derive(Deserialize)]
struct ModrinthUser {
    username: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

impl ModrinthMember {
    fn into_member(self) -> ProjectMember {
        ProjectMember {
            url: Some(format!("https://modrinth.com/user/{}", self.user.username)),
            name: self.user.username,
            role: self.role,
            avatar_url: self.user.avatar_url,
        }
    }
}

fn apply_modrinth_members(detail: &mut ProjectDetail, members: Vec<ModrinthMember>) {
    detail.members = members.into_iter().map(ModrinthMember::into_member).collect();
    if detail.author.is_empty() {
        detail.author = detail
            .members
            .iter()
            .find(|m| m.role.eq_ignore_ascii_case("owner"))
            .or_else(|| detail.members.first())
            .map(|m| m.name.clone())
            .unwrap_or_default();
    }
}

#[derive(Deserialize)]
struct ModrinthLicense {
    #[serde(default)]
    name: Option<String>,
    id: Option<String>,
}

impl ModrinthProject {
    fn into_detail(self) -> ProjectDetail {
        let body = if let Some(raw) = self.body {
            PackageBody::Raw(raw)
        } else if let Some(url) = self.body_url {
            PackageBody::Url(url)
        } else {
            PackageBody::Raw(String::new())
        };

        let license = self
            .license
            .and_then(|l| l.name.or(l.id))
            .filter(|s| !s.is_empty());

        let mut links = Vec::new();
        for (label, url) in [
            ("Source", self.source_url),
            ("Issues", self.issues_url),
            ("Wiki", self.wiki_url),
            ("Discord", self.discord_url),
        ] {
            if let Some(url) = url.filter(|u| !u.is_empty()) {
                links.push((label.to_string(), url));
            }
        }

        ProjectDetail {
            id: self.id.clone(),
            slug: self.slug,
            provider: ProviderId::Modrinth,
            content_type: parse_content_type(&self.project_type),
            name: self.title,
            summary: self.description,
            author: String::new(),
            members: Vec::new(),
            gallery: self
                .gallery
                .into_iter()
                .map(|g| GalleryImage {
                    url: g.url,
                    title: g.title.filter(|t| !t.is_empty()),
                })
                .collect(),
            license,
            links,
            body,
            version_ids: Vec::new(),
            game_versions: self.game_versions,
            loaders: self
                .loaders
                .iter()
                .filter_map(|l| GameLoader::from_str(l).ok())
                .collect(),
            icon_url: self.icon_url,
            created: self.published,
            updated: self.updated,
            downloads: self.downloads,
        }
    }
}

#[derive(Clone, Deserialize)]
struct ModrinthVersion {
    id: String,
    project_id: String,
    name: String,
    version_number: String,
    changelog: Option<String>,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    date_published: DateTime<Utc>,
    downloads: u64,
    #[serde(default)]
    files: Vec<ModrinthFile>,
    #[serde(default)]
    version_type: String,
}

#[derive(Clone, Deserialize)]
struct ModrinthFile {
    hashes: ModrinthHashes,
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    size: u64,
}

#[derive(Clone, Deserialize)]
struct ModrinthHashes {
    sha1: String,
}

impl From<ModrinthVersion> for VersionDetail {
    fn from(v: ModrinthVersion) -> Self {
        Self {
            version_id: v.id,
            project_id: v.project_id,
            name: v.name.clone(),
            version_number: v.version_number,
            changelog: v.changelog,
            game_versions: v.game_versions,
            loaders: v
                .loaders
                .iter()
                .filter_map(|l| GameLoader::from_str(l).ok())
                .collect(),
            published: v.date_published,
            downloads: v.downloads,
            files: v.files.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ModrinthVersion> for VersionSummary {
    fn from(v: ModrinthVersion) -> Self {
        let file_size = v
            .files
            .iter()
            .find(|f| f.primary)
            .or_else(|| v.files.first())
            .map(|f| f.size)
            .unwrap_or(0);
        VersionSummary {
            version_id: v.id,
            project_id: v.project_id,
            name: v.name,
            version_number: v.version_number,
            published: v.date_published,
            release_type: parse_release_type(&v.version_type),
            game_versions: v.game_versions,
            loaders: v
                .loaders
                .iter()
                .filter_map(|l| GameLoader::from_str(l).ok())
                .collect(),
            downloads: v.downloads,
            file_size,
        }
    }
}

impl From<ModrinthFile> for VersionFile {
    fn from(f: ModrinthFile) -> Self {
        Self {
            sha1: f.hashes.sha1,
            url: f.url,
            file_name: f.filename,
            primary: f.primary,
            size: f.size,
            fingerprint: None,
        }
    }
}

fn parse_content_type(s: &str) -> ContentType {
    match s {
        "mod" => ContentType::Mod,
        "resourcepack" => ContentType::ResourcePack,
        "shader" => ContentType::Shader,
        "datapack" => ContentType::DataPack,
        "modpack" => ContentType::Modpack,
        _ => ContentType::Mod,
    }
}

fn parse_release_type(s: &str) -> ReleaseType {
    match s {
        "beta" => ReleaseType::Beta,
        "alpha" => ReleaseType::Alpha,
        _ => ReleaseType::Release,
    }
}
