mod curseforge;
pub mod http;
mod modrinth;
mod registry;

pub use curseforge::CurseForgeProvider;
pub use modrinth::ModrinthProvider;
pub use registry::PackageProviderRegistry;

use super::domain::ProviderId;
use crate::packages::file_identity::FileIdentity;
use super::types::{
    Page, ProjectDetail, ProjectSummary, SearchFilters, VersionDetail, VersionLookup,
    VersionSummary,
};
use crate::LauncherResult;
use crate::state::LauncherServices;

#[async_trait::async_trait]
pub trait PackageProvider: Send + Sync {
    fn id(&self) -> ProviderId;

    async fn search(
        &self,
        filters: &SearchFilters,
        services: &LauncherServices,
    ) -> LauncherResult<Page<ProjectSummary>>;

    async fn get_project(
        &self,
        project_id: &str,
        services: &LauncherServices,
    ) -> LauncherResult<ProjectDetail>;

    async fn get_projects(
        &self,
        project_ids: &[String],
        services: &LauncherServices,
    ) -> LauncherResult<Vec<ProjectDetail>>;

    async fn list_versions(
        &self,
        project_id: &str,
        game_version: Option<&str>,
        loader: Option<super::domain::GameLoader>,
        offset: usize,
        limit: usize,
        services: &LauncherServices,
    ) -> LauncherResult<Page<VersionSummary>>;

    async fn get_version(
        &self,
        project_id: &str,
        version_id: &str,
        services: &LauncherServices,
    ) -> LauncherResult<VersionDetail>;

    async fn get_versions(
        &self,
        version_ids: &[String],
        services: &LauncherServices,
    ) -> LauncherResult<Vec<VersionDetail>>;

    async fn lookup_versions(
        &self,
        identities: &[FileIdentity],
        services: &LauncherServices,
    ) -> LauncherResult<VersionLookup>;

    async fn list_categories(
        &self,
        _content_type: super::domain::ContentType,
        _services: &LauncherServices,
    ) -> LauncherResult<Vec<String>> {
        Ok(Vec::new())
    }
}
