use std::path::Path;

use reqwest::header;
use reqwest::{Method, StatusCode};
use tokio::sync::RwLock;

use crate::api_config::meta_url_base;
use crate::paths;
use crate::state::LauncherServices;
use crate::versions::manifest::{RemoteMigration, VersionMetadata, VersionsManifest};
use crate::{LauncherError, LauncherResult};

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

    #[tracing::instrument(level = "debug", skip(self, services))]
    pub async fn sync(&self, services: &LauncherServices) -> LauncherResult<bool> {
        let Some((manifest, changed)) = Self::fetch_manifest(services).await? else {
            tracing::debug!("skipping versions sync; no remote or cached manifest available");
            return Ok(false);
        };
        *self.manifest.write().await = manifest;
        Ok(changed)
    }

    pub async fn metadata(&self) -> Vec<VersionMetadata> {
        self.manifest.read().await.metadata()
    }

    pub async fn migrations(&self) -> Vec<RemoteMigration> {
        self.manifest.read().await.migrations.clone()
    }

    #[tracing::instrument(level = "debug", skip(services))]
    async fn fetch_manifest(
        services: &LauncherServices,
    ) -> LauncherResult<Option<(VersionsManifest, bool)>> {
        let manifest_path = paths::caches_dir()?.join("versions.json");
        let etag_path = paths::caches_dir()?.join("versions.json.etag");

        let stored_etag = read_sidecar_etag(&etag_path).await;
        let url = format!("{}/oneclient/versions/metadata.json", meta_url_base());
        let url_parsed = url.parse().map_err(LauncherError::UrlError)?;

        let mut request = reqwest::Request::new(Method::GET, url_parsed);
        if let Some(etag) = &stored_etag {
            insert_if_none_match(&mut request, etag);
        }

        match services.requester.send(request).await {
            Ok(res) if res.status() == StatusCode::NOT_MODIFIED => {
                tracing::debug!("versions manifest cache hit (304)");
                read_cached_manifest(&manifest_path, false).await
            }
            Ok(res) if res.status().is_success() => {
                let server_etag = etag_from_response(&res);
                let bytes = res
                    .bytes()
                    .await
                    .map_err(|err| LauncherError::InvalidSettingsProfile {
                        reason: err.to_string(),
                    })?;
                let manifest: VersionsManifest = serde_json::from_slice(&bytes)?;

                if let Some(parent) = manifest_path.parent() {
                    polyio::create_dir_all(parent).await?;
                }
                polyio::write(&manifest_path, &bytes).await?;
                if let Some(etag) = server_etag {
                    let _ = polyio::write(&etag_path, etag.as_bytes()).await;
                }

                Ok(Some((manifest, true)))
            }
            Err(err) if manifest_path.exists() => {
                tracing::debug!(
                    "falling back to cached versions manifest after remote fetch failed: {err}"
                );
                read_cached_manifest(&manifest_path, false).await
            }
            Err(err) => {
                tracing::error!("failed to fetch versions manifest from remote: {err}");
                Ok(None)
            }
            Ok(res) => {
                tracing::warn!(
                    status = %res.status(),
                    "unexpected versions manifest response; using cache when available"
                );
                if manifest_path.exists() {
                    read_cached_manifest(&manifest_path, false).await
                } else {
                    Ok(None)
                }
            }
        }
    }
}

impl Default for VersionsManager {
    fn default() -> Self {
        Self::new()
    }
}

async fn read_cached_manifest(
    manifest_path: &Path,
    changed: bool,
) -> LauncherResult<Option<(VersionsManifest, bool)>> {
    if !manifest_path.exists() {
        return Ok(None);
    }
    let bytes = polyio::read(manifest_path).await?;
    let manifest: VersionsManifest = serde_json::from_slice(&bytes)?;
    Ok(Some((manifest, changed)))
}

fn insert_if_none_match(request: &mut reqwest::Request, etag: &str) {
    if let Ok(value) = header::HeaderValue::from_str(etag) {
        request.headers_mut().insert(header::IF_NONE_MATCH, value);
    }
}

fn etag_from_response(res: &reqwest::Response) -> Option<String> {
    res.headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

async fn read_sidecar_etag(path: &Path) -> Option<String> {
    polyio::read(path)
        .await
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .filter(|etag| !etag.is_empty())
}
