use serde::de::DeserializeOwned;
use tokio::sync::{Mutex, RwLock};

use crate::LauncherResult;
use crate::state::LauncherServices;
use crate::versions::arts::{ArtsManifest, VersionArts};
use crate::versions::manifest::{
    ReleaseTarget, RemoteMigration, VersionMetadata, VersionsManifest, added_release_targets,
};
use oneclient_common::paths;
use oneclient_net::{EtagPolicy, fetch_cached};

pub struct VersionsManager {
    manifest: RwLock<VersionsManifest>,
    arts: RwLock<ArtsManifest>,
    syncing: Mutex<()>,
    added: std::sync::Mutex<Vec<ReleaseTarget>>,
}

impl VersionsManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            manifest: RwLock::new(VersionsManifest::default()),
            arts: RwLock::new(ArtsManifest::default()),
            syncing: Mutex::new(()),
            added: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Seeds from the last synced copy on disk so the UI renders stale metadata
    /// immediately instead of blocking on a request that usually 304s
    #[must_use]
    pub async fn from_cache() -> Self {
        let (manifest, arts) = tokio::join!(Self::cached_manifest(), Self::cached_arts());

        Self {
            manifest: RwLock::new(manifest.unwrap_or_default()),
            arts: RwLock::new(arts.unwrap_or_default()),
            syncing: Mutex::new(()),
            added: std::sync::Mutex::new(Vec::new()),
        }
    }

    async fn cached_arts() -> Option<ArtsManifest> {
        Self::cached_json("version-arts.json", "version arts are").await
    }

    async fn cached_manifest() -> Option<VersionsManifest> {
        Self::cached_json("versions.json", "versions manifest is").await
    }

    async fn cached_json<T: DeserializeOwned>(file_name: &str, what: &str) -> Option<T> {
        let path = paths::caches_dir().ok()?.join(file_name);
        let bytes = polyio::read(&path).await.ok()?;
        match serde_json::from_slice(&bytes) {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!("cached {what} unreadable: {err}");
                None
            }
        }
    }

    #[tracing::instrument(level = "debug", skip(self, services))]
    pub async fn sync(&self, services: &LauncherServices) -> LauncherResult<bool> {
        let _guard = self.syncing.lock().await;

        let (manifest, arts) =
            tokio::join!(Self::fetch_manifest(services), Self::fetch_arts(services));

        let mut changed = false;

        match arts {
            Ok(Some((arts, arts_changed))) => {
                *self.arts.write().await = arts;
                changed |= arts_changed;
            }
            Ok(None) => {
                tracing::debug!("skipping version arts sync; no remote or cached arts available");
            }
            Err(err) => tracing::warn!(
                "version arts sync failed, art falls back to the bundled background: {err}"
            ),
        }

        let Some((manifest, manifest_changed)) = manifest? else {
            tracing::debug!("skipping versions sync; no remote or cached manifest available");
            return Ok(changed);
        };
        let mut current = self.manifest.write().await;
        let added = added_release_targets(&current, &manifest);
        *current = manifest;
        drop(current);

        if !added.is_empty() {
            tracing::info!(added = ?added, "versions manifest gained new versions");
            let mut pending = self.added.lock().unwrap();
            for target in added {
                if !pending.contains(&target) {
                    pending.push(target);
                }
            }
        }
        Ok(changed || manifest_changed)
    }

    pub async fn metadata(&self, meta_url_base: &str) -> Vec<VersionMetadata> {
        self.manifest.read().await.metadata(meta_url_base)
    }

    pub async fn arts(&self, meta_url_base: &str) -> VersionArts {
        let arts = self.arts.read().await;
        VersionArts::new(&arts, meta_url_base)
    }

    pub async fn migrations(&self) -> Vec<RemoteMigration> {
        self.manifest.read().await.migrations.clone()
    }

    pub async fn shows_initial_migration(&self, target: &ReleaseTarget) -> bool {
        self.manifest.read().await.shows_initial_migration(target)
    }

    pub fn take_added_versions(&self) -> Vec<ReleaseTarget> {
        std::mem::take(&mut *self.added.lock().unwrap())
    }

    #[tracing::instrument(level = "debug", skip(services))]
    async fn fetch_manifest(
        services: &LauncherServices,
    ) -> LauncherResult<Option<(VersionsManifest, bool)>> {
        Self::fetch_json(services, "versions.json", "metadata.json").await
    }

    #[tracing::instrument(level = "debug", skip(services))]
    async fn fetch_arts(
        services: &LauncherServices,
    ) -> LauncherResult<Option<(ArtsManifest, bool)>> {
        Self::fetch_json(services, "version-arts.json", "arts.json").await
    }

    async fn fetch_json<T: DeserializeOwned>(
        services: &LauncherServices,
        file_name: &str,
        remote_name: &str,
    ) -> LauncherResult<Option<(T, bool)>> {
        let path = paths::caches_dir()?.join(file_name);
        let url = format!(
            "{}/oneclient/versions/{remote_name}",
            services.requester.config().meta_url_base
        );

        let Some(fetched) =
            fetch_cached(&services.requester, &url, &path, EtagPolicy::CommitNow).await?
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
