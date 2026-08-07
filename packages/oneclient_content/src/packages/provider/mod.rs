mod curseforge;
pub mod http;
mod modrinth;
mod registry;

pub use curseforge::CurseForgeProvider;
pub use modrinth::ModrinthProvider;
pub use registry::PackageProviderRegistry;

use oneclient_common::domain::ProviderId;
use crate::packages::file_identity::FileIdentity;
use super::types::{
    Page, ProjectDetail, ProjectSummary, SearchFilters, VersionDetail, VersionLookup,
    VersionSummary,
};
use crate::error::ContentResult;
use crate::ctx::ContentCtx;

#[async_trait::async_trait]
pub trait PackageProvider: Send + Sync {
    fn id(&self) -> ProviderId;

    async fn search(
        &self,
        filters: &SearchFilters,
        ctx: &ContentCtx,
    ) -> ContentResult<Page<ProjectSummary>>;

    async fn get_project(
        &self,
        project_id: &str,
        ctx: &ContentCtx,
    ) -> ContentResult<ProjectDetail>;

    /// Populates [`ProjectDetail::body`]
    /// Costs an extra request on providers that serve descriptions separately
    /// so install paths should stay on `get_project`
    async fn get_project_with_body(
        &self,
        project_id: &str,
        ctx: &ContentCtx,
    ) -> ContentResult<ProjectDetail> {
        self.get_project(project_id, ctx).await
    }

    async fn get_projects(
        &self,
        project_ids: &[String],
        ctx: &ContentCtx,
    ) -> ContentResult<Vec<ProjectDetail>>;

    async fn list_versions(
        &self,
        project_id: &str,
        game_version: Option<&str>,
        loader: Option<oneclient_common::domain::GameLoader>,
        offset: usize,
        limit: usize,
        ctx: &ContentCtx,
    ) -> ContentResult<Page<VersionSummary>>;

    async fn get_version(
        &self,
        project_id: &str,
        version_id: &str,
        ctx: &ContentCtx,
    ) -> ContentResult<VersionDetail>;

    async fn get_versions(
        &self,
        version_ids: &[String],
        ctx: &ContentCtx,
    ) -> ContentResult<Vec<VersionDetail>>;

    async fn lookup_versions(
        &self,
        identities: &[FileIdentity],
        ctx: &ContentCtx,
    ) -> ContentResult<VersionLookup>;

    async fn list_categories(
        &self,
        _content_type: oneclient_common::domain::ContentType,
        _ctx: &ContentCtx,
    ) -> ContentResult<Vec<String>> {
        Ok(Vec::new())
    }
}
