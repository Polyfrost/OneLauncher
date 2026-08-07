
use tokio::sync::RwLock;

use oneclient_common::paths;
use oneclient_net::{EtagPolicy, fetch_cached};
use crate::state::LauncherServices;
use crate::versions::manifest::{RemoteMigration, VersionMetadata, VersionsManifest};
use crate::LauncherResult;

pub struct VersionsManager {
    manifest: RwLock<VersionsManifest>,
}

impl VersionsManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            manifest: RwLock::new(VersionsManifest::default()),
        }
    }

    /// Seeds from the last synced copy on disk so the UI renders stale metadata
    /// immediately instead of blocking on a request that usually 304s
    #[must_use]
    pub async fn from_cache() -> Self {
        let manifest = Self::cached_manifest()
            .await
            .unwrap_or_else(VersionsManifest::default);

        Self {
            manifest: RwLock::new(manifest),
        }
    }

    async fn cached_manifest() -> Option<VersionsManifest> {
        let path = paths::caches_dir().ok()?.join("versions.json");
        let bytes = polyio::read(&path).await.ok()?;
        match serde_json::from_slice(&bytes) {
            Ok(manifest) => Some(manifest),
            Err(err) => {
                tracing::warn!("cached versions manifest is unreadable: {err}");
                None
            }
        }
    }

    #[tracing::instrument(level = "debug", skip(self, services))]
    pub async fn sync(&self, services: &LauncherServices) -> LauncherResult<bool> {
        let Some((manifest, changed)) = Self::fetch_manifest(services).await? else {
            tracing::debug!("skipping versions sync; no remote or cached manifest available");
            return Ok(false);
        };
        *self.manifest.write().await = manifest;
        Ok(changed)
    }

    pub async fn metadata(&self, meta_url_base: &str) -> Vec<VersionMetadata> {
        self.manifest.read().await.metadata(meta_url_base)
    }

    pub async fn migrations(&self) -> Vec<RemoteMigration> {
        self.manifest.read().await.migrations.clone()
    }

    #[tracing::instrument(level = "debug", skip(services))]
    async fn fetch_manifest(
        services: &LauncherServices,
    ) -> LauncherResult<Option<(VersionsManifest, bool)>> {
        let manifest_path = paths::caches_dir()?.join("versions.json");
        let url = format!(
            "{}/oneclient/versions/metadata.json",
            services.requester.config().meta_url_base
        );

        let Some(fetched) =
            fetch_cached(&services.requester, &url, &manifest_path, EtagPolicy::CommitNow).await?
        else {
            return Ok(None);
        };

        Ok(Some((fetched.json()?, fetched.changed)))
    }
}

impl Default for VersionsManager {
    fn default() -> Self {
        Self::new()
    }
}


