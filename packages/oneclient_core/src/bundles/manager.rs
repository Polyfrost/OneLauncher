use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::Utc;
use oneclient_db::dao::bundle as bundle_dao;
use oneclient_db::models::{BundleRow, NewBundle};
use reqwest::Method;
use reqwest::header;
use reqwest::StatusCode;
use tokio::sync::RwLock;

use crate::bundles::error::BundleError;
use crate::bundles::manifest::{BundleManifest as RemoteBundleManifest, RemoteBundleRef};
use crate::bundles::types::BundleArchive;
use crate::bundles::polymrpack;
use crate::api_config::meta_url_base;
use crate::crypto;
use crate::http::{RequestError, ResponseExt, ResponseOptions};
use crate::packages::domain::GameLoader;
use crate::paths;
use crate::state::LauncherServices;
use crate::{LauncherError, LauncherResult};

#[derive(Debug, Clone)]
pub struct Bundle {
    pub remote_path: String,
    pub mc_version: String,
    pub loader: GameLoader,
    pub file_name: String,
    pub name: String,
    pub version_id: String,
    pub category: String,
    pub loader_version: String,
    pub path: PathBuf,
    pub hidden: bool,
}

impl Bundle {
    fn try_from_row(row: BundleRow) -> LauncherResult<Self> {
        let loader = GameLoader::from_repr(row.mc_loader as u8)
            .ok_or(BundleError::InvalidLoader(row.mc_loader))?;

        let launcher_dir = paths::launcher_dir()?;
        let path = launcher_dir.join(&row.disk_path);

        Ok(Self {
            remote_path: row.remote_path,
            mc_version: row.mc_version,
            loader,
            file_name: row.file_name,
            name: row.name.unwrap_or_default(),
            version_id: row.version_id.unwrap_or_default(),
            category: row.category.unwrap_or_default(),
            loader_version: row.loader_version.unwrap_or_default(),
            path,
            hidden: row.hidden != 0,
        })
    }
}

pub struct BundlesManager {
    manifest: RwLock<RemoteBundleManifest>,
    archive_cache: RwLock<HashMap<PathBuf, crate::bundles::types::BundleManifest>>,
}

impl BundlesManager {
    pub fn new() -> Self {
        Self {
            manifest: RwLock::new(RemoteBundleManifest::default()),
            archive_cache: RwLock::new(HashMap::new()),
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn sync(&self, services: &LauncherServices) -> LauncherResult<bool> {
        let Some((manifest, manifest_changed)) = Self::fetch_manifest(services).await? else {
            tracing::debug!(
                "skipping bundle sync because no remote or cached manifest is available"
            );
            return Ok(false);
        };
        *self.manifest.write().await = manifest.clone();
        self.archive_cache.write().await.clear();

        if !manifest_changed {
            tracing::debug!("bundles manifest unchanged; skipping per-bundle remote checks");
            return Ok(false);
        }

        let remote_paths: Vec<String> = manifest
            .remote_paths()
            .into_iter()
            .map(|entry| entry.remote_path)
            .collect();

        bundle_dao::hide_bundles_not_in(&services.db, &remote_paths).await?;

        let bundles_root = paths::bundles_dir()?;
        polyio::create_dir_all(&bundles_root.join("bundles")).await?;

        for entry in manifest.remote_paths() {
            if let Err(err) = self.sync_bundle(&entry, services, &bundles_root).await {
                tracing::warn!(
                    remote_path = %entry.remote_path,
                    error = %err,
                    "failed to sync bundle"
                );
            }
        }

        Ok(true)
    }

    #[tracing::instrument(level = "debug", skip(self, services))]
    pub async fn archives_for(
        &self,
        services: &LauncherServices,
        mc_version: &str,
        loader: GameLoader,
    ) -> LauncherResult<Vec<BundleArchive>> {
        let mut archives = Vec::new();
        for bundle in self.list_for(services, mc_version, loader).await? {
            let manifest = self.manifest_for_archive(&bundle.path).await?;
            archives.push(BundleArchive { bundle, manifest });
        }
        Ok(archives)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn manifest_for_archive(
        &self,
        path: &Path,
    ) -> LauncherResult<crate::bundles::types::BundleManifest> {
        if let Some(manifest) = self.archive_cache.read().await.get(path) {
            return Ok(manifest.clone());
        }
        let manifest = polymrpack::read_manifest_from_archive(path).await?;
        self.archive_cache
            .write()
            .await
            .insert(path.to_path_buf(), manifest.clone());
        Ok(manifest)
    }

    #[tracing::instrument(level = "debug", skip(self, services))]
    pub async fn list_for(
        &self,
        services: &LauncherServices,
        mc_version: &str,
        loader: GameLoader,
    ) -> LauncherResult<Vec<Bundle>> {
        let rows =
            bundle_dao::list_visible_for_version_loader(&services.db, mc_version, loader as i64)
                .await?;

        rows.into_iter()
            .map(Bundle::try_from_row)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(level = "debug", skip(self, entry, services, bundles_root), fields(remote_path = %entry.remote_path))]
    async fn sync_bundle(
        &self,
        entry: &RemoteBundleRef,
        services: &LauncherServices,
        bundles_root: &Path,
    ) -> LauncherResult<()> {
        let loader = GameLoader::from_str(&entry.loader)
            .map_err(|_| BundleError::UnknownLoader(entry.loader.clone()))?;

        let file_name = entry
            .remote_path
            .split('/')
            .next_back()
            .ok_or_else(|| BundleError::InvalidPath(entry.remote_path.clone()))?
            .to_string();

        let disk_path = bundles_root.join("bundles").join(&file_name);
        let remote_url = format!("{}{}", meta_url_base(), entry.remote_path);

        download_bundle_if_needed(services, &remote_url, &disk_path, &entry.sha1).await?;

        let meta = polymrpack::read_meta_from_archive(&disk_path).await?;

        let relative_disk_path = disk_path
            .strip_prefix(paths::launcher_dir()?)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| disk_path.to_string_lossy().into_owned());

        let synced_at = Utc::now().to_rfc3339();

        bundle_dao::upsert_bundle(
            &services.db,
            NewBundle {
                remote_path: &entry.remote_path,
                mc_version: &entry.mc_version,
                mc_loader: loader as i64,
                file_name: &file_name,
                name: Some(&meta.name),
                version_id: Some(&meta.version_id),
                category: Some(&meta.category),
                loader_version: Some(&meta.loader_version),
                disk_path: &relative_disk_path,
                hidden: false,
                etag: Some(&entry.sha1),
                synced_at: Some(&synced_at),
            },
        )
        .await?;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(services))]
    async fn fetch_manifest(
        services: &LauncherServices,
    ) -> LauncherResult<Option<(RemoteBundleManifest, bool)>> {
        let manifest_path = paths::bundles_dir()?.join("metadata.json");
        let etag_path = paths::bundles_dir()?.join("metadata.json.etag");

        let stored_etag = read_sidecar_etag(&etag_path).await;
        let url = format!("{}/oneclient/bundles/metadata.json", meta_url_base());
        let url_parsed = url.parse().map_err(LauncherError::UrlError)?;

        let mut request = reqwest::Request::new(Method::GET, url_parsed);
        if let Some(etag) = &stored_etag {
            insert_if_none_match(&mut request, etag);
        }

        match services.requester.send(request).await {
            Ok(res) if res.status() == StatusCode::NOT_MODIFIED => {
                tracing::debug!("bundles manifest cache hit (304)");
                return read_cached_manifest(&manifest_path, false).await;
            }
            Ok(res) if res.status().is_success() => {
                let server_etag = etag_from_response(&res);
                let bytes = res.bytes().await.map_err(|err| LauncherError::InvalidSettingsProfile {
                    reason: err.to_string(),
                })?;
                let manifest: RemoteBundleManifest = serde_json::from_slice(&bytes)?;

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
                    "falling back to cached bundles manifest after remote fetch failed: {err}"
                );
                return read_cached_manifest(&manifest_path, false).await;
            }
            Err(err) => {
                tracing::error!("failed to fetch bundles manifest from remote: {err}");
                Ok(None)
            }
            Ok(res) => {
                tracing::warn!(
                    status = %res.status(),
                    "unexpected bundles manifest response; using cache when available"
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

impl Default for BundlesManager {
    fn default() -> Self {
        Self::new()
    }
}

async fn read_cached_manifest(
    manifest_path: &Path,
    changed: bool,
) -> LauncherResult<Option<(RemoteBundleManifest, bool)>> {
    if !manifest_path.exists() {
        return Ok(None);
    }
    let bytes = polyio::read(manifest_path).await?;
    let manifest: RemoteBundleManifest = serde_json::from_slice(&bytes)?;
    Ok(Some((manifest, changed)))
}

#[tracing::instrument(level = "debug", skip(services))]
async fn download_bundle_if_needed(
    services: &LauncherServices,
    url: &str,
    disk_path: &Path,
    expected_sha1: &str,
) -> LauncherResult<()> {
    let expected_sha1 = crypto::normalize_hash(expected_sha1);

    if disk_path.exists() {
        let local_sha1 = crypto::sha1_file(disk_path).await?;
        if crypto::normalize_hash(&local_sha1) == expected_sha1 {
            tracing::debug!("bundle cache hit via SHA1: {url}");
            return Ok(());
        }
        tracing::debug!("bundle SHA1 mismatch, re-downloading: {url}");
    } else {
        tracing::debug!("downloading bundle from remote: {url}");
    }

    let get = reqwest::Request::new(
        Method::GET,
        url.parse().map_err(LauncherError::UrlError)?,
    );

    let res = services
        .requester
        .send(get)
        .await
        .map_err(map_request_error)?;

    if !res.status().is_success() {
        return Err(BundleError::MissingFile(format!(
            "{url} (HTTP {})",
            res.status()
        ))
        .into());
    }

    if let Some(parent) = disk_path.parent() {
        polyio::create_dir_all(parent).await?;
    }

    let stream = res
        .stream(ResponseOptions::default(), &services.notifier)
        .await
        .map_err(map_request_error)?;
    let stream = std::pin::pin!(stream);
    polyio::write_stream(disk_path, stream)
        .await
        .map_err(map_request_error)?;

    let local_sha1 = crypto::sha1_file(disk_path).await?;
    if crypto::normalize_hash(&local_sha1) != expected_sha1 {
        return Err(BundleError::MissingFile(format!(
            "{url} (SHA1 mismatch: expected {expected_sha1}, got {local_sha1})"
        ))
        .into());
    }

    Ok(())
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

fn map_request_error(err: RequestError) -> LauncherError {
    match err {
        RequestError::DeserializeError {
            source,
            type_name,
            url,
            status,
            snippet,
        } => LauncherError::InvalidSettingsProfile {
            reason: format!(
                "failed to parse {type_name} from {url} (HTTP {status}): {source} — body: {snippet}"
            ),
        },
        other => LauncherError::InvalidSettingsProfile {
            reason: other.to_string(),
        },
    }
}
