use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_common::domain::GameLoader;
use oneclient_content::packages::types::{
    DEFAULT_PAGE_SIZE, Page, ProjectDetail, ProjectSummary, SearchFilters, SearchSort,
    VersionSummary,
};
use oneclient_content::packages::{CachedPackageMeta, ContentType, ProviderId};
use oneclient_core::LauncherError;

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

    #[tracing::instrument(name = "package_search", level = "debug", skip(self, keys), fields(provider = ?keys.provider, page = keys.page))]
    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let provider = state.services.packages.get(keys.provider)?;
        Ok(provider
            .search(
                &SearchFilters {
                    query: (!keys.query.trim().is_empty()).then(|| keys.query.trim().to_string()),
                    content_type: Some(keys.content_type),
                    game_versions: (!keys.game_versions.is_empty())
                        .then(|| keys.game_versions.clone()),
                    loaders: (!keys.loaders.is_empty()).then(|| keys.loaders.clone()),
                    categories: (!keys.categories.is_empty()).then(|| keys.categories.clone()),
                    sort: Some(keys.sort),
                    offset: Some(keys.page * BROWSE_PAGE_SIZE),
                    limit: Some(BROWSE_PAGE_SIZE),
                },
                &state.services.content(),
            )
            .await?)
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
    super::state::settled_or_loading(query)
        .map(|page| page.items)
        .unwrap_or_default()
}

pub fn search_total(query: &UseQuery<PackageSearchQuery>) -> usize {
    super::state::settled_or_loading(query)
        .map(|page| page.total)
        .unwrap_or(0)
}

pub fn search_pending(query: &UseQuery<PackageSearchQuery>) -> bool {
    super::state::query_is_busy(query)
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
        let state = crate::launcher::state()?;
        let provider = state.services.packages.get(keys.provider)?;
        Ok(provider
            .get_project_with_body(&keys.project_id, &state.services.content())
            .await?)
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
    super::state::settled_or_loading(query)
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

    #[tracing::instrument(name = "package_meta_batch", level = "debug", skip(self, keys), fields(provider = ?keys.provider, ids = keys.project_ids.len()))]
    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        Ok(oneclient_content::packages::fetch_package_meta(
            &state.services.content(),
            keys.provider,
            &keys.project_ids,
        )
        .await?)
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
    super::state::settled_or_loading(query).unwrap_or_default()
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
        let state = crate::launcher::state()?;
        let provider = state.services.packages.get(keys.provider)?;
        Ok(provider
            .list_versions(
                &keys.project_id,
                keys.game_version.as_deref(),
                keys.loader,
                keys.page * VERSIONS_PAGE_SIZE,
                VERSIONS_PAGE_SIZE,
                &state.services.content(),
            )
            .await?)
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
    super::state::settled_or_loading(query)
        .map(|page| page.items)
        .unwrap_or_default()
}

pub fn versions_total(query: &UseQuery<PackageVersionsQuery>) -> usize {
    super::state::settled_or_loading(query)
        .map(|page| page.total)
        .unwrap_or(0)
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

    #[tracing::instrument(name = "package_categories", level = "debug", skip(self, keys), fields(provider = ?keys.provider))]
    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let provider = state.services.packages.get(keys.provider)?;
        Ok(provider
            .list_categories(keys.content_type, &state.services.content())
            .await?)
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
    super::state::settled_or_loading(query).unwrap_or_default()
}
