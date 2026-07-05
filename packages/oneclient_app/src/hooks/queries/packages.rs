
use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::packages::domain::GameLoader;
use oneclient_core::packages::types::{
    DEFAULT_PAGE_SIZE, Page, ProjectDetail, ProjectSummary, SearchFilters, SearchSort,
    VersionSummary,
};
use oneclient_core::packages::{CachedPackageMeta, ContentType, ProviderId};
use oneclient_core::{LauncherError, LauncherState};

pub const BROWSE_PAGE_SIZE: usize = DEFAULT_PAGE_SIZE;
pub const VERSIONS_PAGE_SIZE: usize = 20;

pub fn content_type_for_slug(slug: &str) -> ContentType {
    match slug {
        "shader" => ContentType::Shader,
        "texture" => ContentType::ResourcePack,
        _ => ContentType::Mod,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageSearchQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageSearchKeys {
    pub provider: ProviderId,
    pub content_type: ContentType,
    pub query: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<GameLoader>,
    pub categories: Vec<String>,
    pub sort: SearchSort,
    pub page: usize,
}

impl QueryCapability for PackageSearchQuery {
    type Ok = Page<ProjectSummary>;
    type Err = LauncherError;
    type Keys = PackageSearchKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get()?;
        let provider = state.services.packages.get(keys.provider)?;
        provider
            .search(
                &SearchFilters {
                    query: (!keys.query.trim().is_empty()).then(|| keys.query.trim().to_string()),
                    content_type: Some(keys.content_type),
                    game_versions: (!keys.game_versions.is_empty()).then(|| keys.game_versions.clone()),
                    loaders: (!keys.loaders.is_empty()).then(|| keys.loaders.clone()),
                    categories: (!keys.categories.is_empty()).then(|| keys.categories.clone()),
                    sort: Some(keys.sort),
                    offset: Some(keys.page * BROWSE_PAGE_SIZE),
                    limit: Some(BROWSE_PAGE_SIZE),
                },
                &state.services,
            )
            .await
    }
}

#[allow(clippy::too_many_arguments)]
pub fn use_package_search(
    provider: ProviderId,
    content_type: ContentType,
    query: String,
    game_versions: Vec<String>,
    loaders: Vec<GameLoader>,
    categories: Vec<String>,
    sort: SearchSort,
    page: usize,
) -> UseQuery<PackageSearchQuery> {
    use_query(Query::new(
        PackageSearchKeys {
            provider,
            content_type,
            query,
            game_versions,
            loaders,
            categories,
            sort,
            page,
        },
        PackageSearchQuery,
    ))
}

pub fn search_items(query: &UseQuery<PackageSearchQuery>) -> Vec<ProjectSummary> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(page), .. } => page.items.clone(),
        QueryStateData::Loading { res: Some(Ok(page)) } => page.items.clone(),
        _ => Vec::new(),
    }
}

pub fn search_total(query: &UseQuery<PackageSearchQuery>) -> usize {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(page), .. } => page.total,
        QueryStateData::Loading { res: Some(Ok(page)) } => page.total,
        _ => 0,
    }
}

pub fn search_pending(query: &UseQuery<PackageSearchQuery>) -> bool {
    matches!(&*query.read().state(), QueryStateData::Pending | QueryStateData::Loading { .. })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageProjectQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageProjectKeys {
    pub provider: ProviderId,
    pub project_id: String,
}

impl QueryCapability for PackageProjectQuery {
    type Ok = ProjectDetail;
    type Err = LauncherError;
    type Keys = PackageProjectKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get()?;
        let provider = state.services.packages.get(keys.provider)?;
        provider.get_project(&keys.project_id, &state.services).await
    }
}

pub fn use_package_project(
    provider: ProviderId,
    project_id: String,
) -> UseQuery<PackageProjectQuery> {
    use_query(Query::new(
        PackageProjectKeys {
            provider,
            project_id,
        },
        PackageProjectQuery,
    ))
}

pub fn project_detail(query: &UseQuery<PackageProjectQuery>) -> Option<ProjectDetail> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(detail), .. } => Some(detail.clone()),
        QueryStateData::Loading { res: Some(Ok(detail)) } => Some(detail.clone()),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageMetaBatchQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageMetaBatchKeys {
    pub provider: ProviderId,
    pub project_ids: Vec<String>,
}

impl QueryCapability for PackageMetaBatchQuery {
    type Ok = std::collections::HashMap<String, CachedPackageMeta>;
    type Err = LauncherError;
    type Keys = PackageMetaBatchKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get()?;
        oneclient_core::packages::fetch_package_meta(
            &state.services,
            keys.provider,
            &keys.project_ids,
        )
        .await
    }
}

pub fn use_package_meta_batch(
    provider: ProviderId,
    mut project_ids: Vec<String>,
) -> UseQuery<PackageMetaBatchQuery> {
    project_ids.sort();
    project_ids.dedup();
    use_query(Query::new(
        PackageMetaBatchKeys {
            provider,
            project_ids,
        },
        PackageMetaBatchQuery,
    ))
}

pub fn package_meta_batch(
    query: &UseQuery<PackageMetaBatchQuery>,
) -> std::collections::HashMap<String, CachedPackageMeta> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(map), .. } => map.clone(),
        QueryStateData::Loading { res: Some(Ok(map)) } => map.clone(),
        _ => std::collections::HashMap::new(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageVersionsQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageVersionsKeys {
    pub provider: ProviderId,
    pub project_id: String,
    pub game_version: Option<String>,
    pub loader: Option<GameLoader>,
    pub page: usize,
}

impl QueryCapability for PackageVersionsQuery {
    type Ok = Page<VersionSummary>;
    type Err = LauncherError;
    type Keys = PackageVersionsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get()?;
        let provider = state.services.packages.get(keys.provider)?;
        provider
            .list_versions(
                &keys.project_id,
                keys.game_version.as_deref(),
                keys.loader,
                keys.page * VERSIONS_PAGE_SIZE,
                VERSIONS_PAGE_SIZE,
                &state.services,
            )
            .await
    }
}

pub fn use_package_versions(
    provider: ProviderId,
    project_id: String,
    game_version: Option<String>,
    loader: Option<GameLoader>,
    page: usize,
) -> UseQuery<PackageVersionsQuery> {
    use_query(Query::new(
        PackageVersionsKeys {
            provider,
            project_id,
            game_version,
            loader,
            page,
        },
        PackageVersionsQuery,
    ))
}

pub fn version_list(query: &UseQuery<PackageVersionsQuery>) -> Vec<VersionSummary> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(page), .. } => page.items.clone(),
        QueryStateData::Loading { res: Some(Ok(page)) } => page.items.clone(),
        _ => Vec::new(),
    }
}

pub fn versions_total(query: &UseQuery<PackageVersionsQuery>) -> usize {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(page), .. } => page.total,
        QueryStateData::Loading { res: Some(Ok(page)) } => page.total,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageCategoriesQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageCategoriesKeys {
    pub provider: ProviderId,
    pub content_type: ContentType,
}

impl QueryCapability for PackageCategoriesQuery {
    type Ok = Vec<String>;
    type Err = LauncherError;
    type Keys = PackageCategoriesKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get()?;
        let provider = state.services.packages.get(keys.provider)?;
        provider
            .list_categories(keys.content_type, &state.services)
            .await
    }
}

pub fn use_package_categories(
    provider: ProviderId,
    content_type: ContentType,
) -> UseQuery<PackageCategoriesQuery> {
    use_query(Query::new(
        PackageCategoriesKeys {
            provider,
            content_type,
        },
        PackageCategoriesQuery,
    ))
}

pub fn category_list(query: &UseQuery<PackageCategoriesQuery>) -> Vec<String> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. } => list.clone(),
        QueryStateData::Loading { res: Some(Ok(list)) } => list.clone(),
        _ => Vec::new(),
    }
}
