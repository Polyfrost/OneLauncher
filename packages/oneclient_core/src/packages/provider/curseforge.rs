use std::collections::HashMap;

use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::Deserialize;
use url::Url;

use super::PackageProvider;
use super::http::fetch_json_with_headers;
use crate::api_config::curseforge_headers;
use crate::LauncherResult;
use crate::constants::{CURSEFORGE_API_URL, CURSEFORGE_GAME_ID};
use crate::packages::domain::{ContentType, GameLoader, ProviderId};
use crate::packages::file_identity::FileIdentity;
use crate::packages::types::{
    GalleryImage, PackageBody, Page, ProjectDetail, ProjectMember, ProjectSummary, ReleaseType,
    SearchFilters, VersionDetail, VersionFile, VersionLookup, VersionSummary,
};
use crate::state::LauncherServices;

pub struct CurseForgeProvider;

#[inline(always)]
fn api_url(path: &str) -> String {
    format!("{CURSEFORGE_API_URL}{path}")
}

#[async_trait::async_trait]
impl PackageProvider for CurseForgeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::CurseForge
    }

    async fn search(
        &self,
        filters: &SearchFilters,
        services: &LauncherServices,
    ) -> LauncherResult<Page<ProjectSummary>> {
        let mut url = Url::parse(&api_url("/mods/search"))?;

        {
            let mut params = url.query_pairs_mut();
            params.append_pair("gameId", &CURSEFORGE_GAME_ID.to_string());
            let class_id = filters
                .content_type
                .map(cf_class_id)
                .unwrap_or(CfClass::Mod as u32);
            params.append_pair("classId", &class_id.to_string());
            params.append_pair(
                "pageSize",
                &filters
                    .limit
                    .unwrap_or(super::super::types::DEFAULT_PAGE_SIZE)
                    .to_string(),
            );
            params.append_pair("index", &filters.offset.unwrap_or(0).to_string());
            params.append_pair("sortField", "6");
            params.append_pair("sortOrder", "desc");
            if let Some(q) = &filters.query {
                params.append_pair("searchFilter", q);
            }
            if let Some(v) = filters.game_versions.as_ref().and_then(|v| v.first()) {
                params.append_pair("gameVersion", v);
            }
            if let Some(loader) = filters
                .loaders
                .as_ref()
                .and_then(|l| l.first())
                .and_then(|l| cf_loader_type(*l))
            {
                params.append_pair("modLoaderType", &loader.to_string());
            }
        }

        let response: CfPaged<Vec<CfMod>> = fetch_json_with_headers(
            &services.requester,
            Method::GET,
            url.as_str(),
            None,
            &curseforge_headers(),
        )
        .await?;

        Ok(Page {
            offset: response.pagination.index,
            limit: response.pagination.page_size,
            total: response.pagination.total_count,
            items: response
                .data
                .into_iter()
                .map(ProjectSummary::from)
                .collect(),
        })
    }

    async fn get_project(
        &self,
        project_id: &str,
        services: &LauncherServices,
    ) -> LauncherResult<ProjectDetail> {
        let response: CfData<CfMod> = fetch_json_with_headers(
            &services.requester,
            Method::GET,
            &api_url(&format!("/mods/{project_id}")),
            None,
            &curseforge_headers(),
        )
        .await?;
        Ok(response.data.into_detail())
    }

    async fn get_projects(
        &self,
        project_ids: &[String],
        services: &LauncherServices,
    ) -> LauncherResult<Vec<ProjectDetail>> {
        let mod_ids: Vec<u32> = project_ids.iter().filter_map(|s| s.parse().ok()).collect();
        let body = serde_json::json!({ "modIds": mod_ids });
        let response: CfData<Vec<CfMod>> = fetch_json_with_headers(
            &services.requester,
            Method::POST,
            &api_url("/mods"),
            Some(body),
            &curseforge_headers(),
        )
        .await?;
        Ok(response.data.into_iter().map(CfMod::into_detail).collect())
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
        let mut url = Url::parse(&api_url(&format!("/mods/{project_id}/files")))?;
        {
            let mut params = url.query_pairs_mut();
            if let Some(v) = game_version {
                params.append_pair("gameVersion", v);
            }
            if let Some(t) = loader.and_then(cf_loader_type) {
                params.append_pair("modLoaderType", &t.to_string());
            }
        }
        let response: CfPaged<Vec<CfFile>> = fetch_json_with_headers(
            &services.requester,
            Method::GET,
            url.as_str(),
            None,
            &curseforge_headers(),
        )
        .await?;

        let total = response.data.len();
        let items = response
            .data
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

    async fn list_categories(
        &self,
        content_type: ContentType,
        services: &LauncherServices,
    ) -> LauncherResult<Vec<String>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CfCategory {
            name: String,
        }
        let mut url = Url::parse(&api_url("/categories"))?;
        url.query_pairs_mut()
            .append_pair("gameId", &CURSEFORGE_GAME_ID.to_string())
            .append_pair("classId", &cf_class_id(content_type).to_string());
        let response: CfData<Vec<CfCategory>> = fetch_json_with_headers(
            &services.requester,
            Method::GET,
            url.as_str(),
            None,
            &curseforge_headers(),
        )
        .await?;
        Ok(response.data.into_iter().map(|c| c.name).collect())
    }

    async fn get_version(
        &self,
        _project_id: &str,
        version_id: &str,
        services: &LauncherServices,
    ) -> LauncherResult<VersionDetail> {
        let response: CfData<CfFile> = fetch_json_with_headers(
            &services.requester,
            Method::GET,
            &api_url(&format!("/mods/files/{version_id}")),
            None,
            &curseforge_headers(),
        )
        .await?;
        Ok(response.data.into())
    }

    async fn get_versions(
        &self,
        version_ids: &[String],
        services: &LauncherServices,
    ) -> LauncherResult<Vec<VersionDetail>> {
        let file_ids: Vec<u32> = version_ids.iter().filter_map(|s| s.parse().ok()).collect();
        let body = serde_json::json!({ "fileIds": file_ids });
        let response: CfData<Vec<CfFile>> = fetch_json_with_headers(
            &services.requester,
            Method::POST,
            &api_url("/mods/files"),
            Some(body),
            &curseforge_headers(),
        )
        .await?;
        Ok(response.data.into_iter().map(Into::into).collect())
    }

    async fn lookup_versions(
        &self,
        identities: &[FileIdentity],
        services: &LauncherServices,
    ) -> LauncherResult<VersionLookup> {
        let mut out = HashMap::new();

        let with_fingerprint: Vec<(&FileIdentity, u32)> = identities
            .iter()
            .filter_map(|id| id.cf_fingerprint.map(|fp| (id, fp)))
            .collect();

        if with_fingerprint.is_empty() {
            return Ok(out);
        }

        let nums: Vec<u32> = with_fingerprint.iter().map(|(_, fp)| *fp).collect();
        let body = serde_json::json!({ "fingerprints": nums });

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            exact_matches: Vec<FingerprintMatch>,
        }
        #[derive(Deserialize)]
        struct FingerprintMatch {
            file: CfFile,
        }

        let response: CfData<Response> = fetch_json_with_headers(
            &services.requester,
            Method::POST,
            &api_url(&format!("/mods/fingerprints/{CURSEFORGE_GAME_ID}")),
            Some(body),
            &curseforge_headers(),
        )
        .await?;

        for m in response.data.exact_matches {
            let version: VersionDetail = m.file.clone().into();
            let file_sha1 = version
                .files
                .first()
                .map(|f| crate::crypto::normalize_hash(&f.sha1));

            let fp = m.file.file_fingerprint;

            for (identity, expected_fp) in &with_fingerprint {
                if *expected_fp == fp || file_sha1.as_deref() == Some(identity.sha1.as_str()) {
                    out.insert(identity.sha1.clone(), version.clone());
                }
            }
        }

        Ok(out)
    }
}

#[derive(Deserialize)]
struct CfData<T> {
    data: T,
}

#[derive(Deserialize)]
struct CfPaged<T> {
    data: T,
    pagination: CfPagination,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfPagination {
    index: usize,
    page_size: usize,
    total_count: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u32,
    name: String,
    slug: String,
    summary: String,
    download_count: u64,
    date_created: DateTime<Utc>,
    date_modified: DateTime<Utc>,
    #[serde(default)]
    logo: Option<CfLogo>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
    #[serde(default)]
    links: Option<CfLinks>,
    #[serde(default)]
    screenshots: Vec<CfScreenshot>,
    class_id: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfScreenshot {
    url: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLogo {
    #[serde(default)]
    thumbnail_url: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct CfAuthor {
    name: String,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLinks {
    #[serde(default)]
    website_url: Option<String>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    issues_url: Option<String>,
    #[serde(default)]
    wiki_url: Option<String>,
}

fn cf_links(links: &CfLinks) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (label, url) in [
        ("Website", &links.website_url),
        ("Source", &links.source_url),
        ("Issues", &links.issues_url),
        ("Wiki", &links.wiki_url),
    ] {
        if let Some(url) = url.as_ref().filter(|u| !u.is_empty()) {
            out.push((label.to_string(), url.clone()));
        }
    }
    out
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u32,
    mod_id: u32,
    display_name: String,
    file_name: String,
    release_type: u8,
    file_date: DateTime<Utc>,
    download_count: u64,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    mod_loaders: Vec<CfLoader>,
    #[serde(default)]
    hashes: Vec<CfHash>,
    file_fingerprint: u32,
    download_url: String,
    file_length: u64,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(from = "u8")]
enum CfLoader {
    Forge = 1,
    Cauldron = 2,
    #[allow(dead_code)]
    LiteLoader = 3,
    Fabric = 4,
    Quilt = 5,
    NeoForge = 6,
}

#[derive(Clone, Deserialize)]
struct CfHash {
    value: String,
    #[serde(rename = "algo")]
    algorithm: u8,
}

#[derive(Deserialize)]
#[serde(from = "u32")]
enum CfClass {
    Mod = 6,
    ResourcePack = 12,
    DataPack = 4472,
    Shader = 6552,
    Modpack = 4471,
}

impl From<CfMod> for ProjectSummary {
    fn from(m: CfMod) -> Self {
        Self {
            id: m.id.to_string(),
            slug: m.slug,
            provider: ProviderId::CurseForge,
            content_type: cf_class_to_type(m.class_id),
            name: m.name,
            summary: m.summary,
            author: m
                .authors
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            icon_url: m
                .logo
                .and_then(|l| l.thumbnail_url.or(l.url))
                .filter(|s| !s.is_empty()),
            downloads: m.download_count,
            created: m.date_created,
            updated: m.date_modified,
            loaders: Vec::new(),
            game_versions: Vec::new(),
        }
    }
}

impl CfMod {
    fn into_detail(self) -> ProjectDetail {
        ProjectDetail {
            id: self.id.to_string(),
            slug: self.slug,
            provider: ProviderId::CurseForge,
            content_type: cf_class_to_type(self.class_id),
            name: self.name.clone(),
            summary: self.summary,
            author: self
                .authors
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            members: self
                .authors
                .iter()
                .map(|a| ProjectMember {
                    name: a.name.clone(),
                    role: "Author".to_string(),
                    url: a.url.clone(),
                    avatar_url: None,
                })
                .collect(),
            gallery: self
                .screenshots
                .into_iter()
                .map(|s| GalleryImage {
                    url: s.url,
                    title: s.title.filter(|t| !t.is_empty()),
                })
                .collect(),
            license: None,
            links: self
                .links
                .as_ref()
                .map(cf_links)
                .unwrap_or_default(),
            body: PackageBody::Raw(String::new()),
            version_ids: Vec::new(),
            game_versions: Vec::new(),
            loaders: Vec::new(),
            icon_url: self
                .logo
                .and_then(|l| l.thumbnail_url.or(l.url))
                .filter(|s| !s.is_empty()),
            created: self.date_created,
            updated: self.date_modified,
            downloads: self.download_count,
        }
    }
}

impl From<CfFile> for VersionSummary {
    fn from(f: CfFile) -> Self {
        VersionSummary {
            version_id: f.id.to_string(),
            project_id: f.mod_id.to_string(),
            name: f.display_name,
            version_number: f.file_name,
            published: f.file_date,
            release_type: match f.release_type {
                2 => ReleaseType::Beta,
                3 => ReleaseType::Alpha,
                _ => ReleaseType::Release,
            },
            game_versions: f.game_versions,
            loaders: f
                .mod_loaders
                .iter()
                .filter_map(|l| cf_loader_to_game(*l))
                .collect(),
            downloads: f.download_count,
            file_size: f.file_length,
        }
    }
}

impl From<CfFile> for VersionDetail {
    fn from(f: CfFile) -> Self {
        let sha1 = f
            .hashes
            .iter()
            .find(|h| h.algorithm == 1)
            .map(|h| h.value.clone())
            .unwrap_or_default();

        VersionDetail {
            version_id: f.id.to_string(),
            project_id: f.mod_id.to_string(),
            name: f.display_name,
            version_number: f.file_name.clone(),
            changelog: None,
            game_versions: f.game_versions,
            loaders: f
                .mod_loaders
                .iter()
                .filter_map(|l| cf_loader_to_game(*l))
                .collect(),
            published: f.file_date,
            downloads: f.download_count,
            files: vec![VersionFile {
                sha1,
                url: f.download_url,
                file_name: f.file_name,
                primary: true,
                size: f.file_length,
                fingerprint: Some(f.file_fingerprint.to_string()),
            }],
        }
    }
}

fn cf_class_id(t: ContentType) -> u32 {
    match t {
        ContentType::Mod => 6,
        ContentType::ResourcePack => 12,
        ContentType::DataPack => 4472,
        ContentType::Shader => 6552,
        ContentType::Modpack => 4471,
        ContentType::World => 6,
    }
}

fn cf_class_to_type(class_id: u32) -> ContentType {
    match CfClass::from(class_id) {
        CfClass::ResourcePack => ContentType::ResourcePack,
        CfClass::DataPack => ContentType::DataPack,
        CfClass::Shader => ContentType::Shader,
        CfClass::Modpack => ContentType::Modpack,
        CfClass::Mod => ContentType::Mod,
    }
}

impl From<u8> for CfLoader {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Forge,
            2 => Self::Cauldron,
            3 => Self::LiteLoader,
            4 => Self::Fabric,
            5 => Self::Quilt,
            6 => Self::NeoForge,
            _ => Self::Cauldron,
        }
    }
}

impl From<u32> for CfClass {
    fn from(value: u32) -> Self {
        match value {
            6 => Self::Mod,
            12 => Self::ResourcePack,
            4472 => Self::DataPack,
            6552 => Self::Shader,
            4471 => Self::Modpack,
            _ => Self::Mod,
        }
    }
}

fn cf_loader_to_game(loader: CfLoader) -> Option<GameLoader> {
    match loader {
        CfLoader::Forge => Some(GameLoader::Forge),
        CfLoader::Fabric => Some(GameLoader::Fabric),
        CfLoader::Quilt => Some(GameLoader::Quilt),
        CfLoader::NeoForge => Some(GameLoader::NeoForge),
        _ => None,
    }
}

fn cf_loader_type(loader: GameLoader) -> Option<u8> {
    match loader {
        GameLoader::Forge => Some(1),
        GameLoader::Fabric => Some(4),
        GameLoader::Quilt => Some(5),
        GameLoader::NeoForge => Some(6),
        _ => None,
    }
}
