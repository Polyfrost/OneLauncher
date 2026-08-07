use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::Utc;
use oneclient_db::dao::bundle as bundle_dao;
use oneclient_db::models::{BundleRow, NewBundle};
use tokio::sync::RwLock;

use crate::bundles::error::BundleError;
use crate::bundles::manifest::{BundleManifest as RemoteBundleManifest, RemoteBundleRef};
use crate::bundles::types::BundleArchive;
use crate::bundles::polymrpack;
use oneclient_net::RequestError;
use oneclient_common::domain::GameLoader;
use oneclient_common::paths;
use oneclient_net::{EtagPolicy, fetch_cached};
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};

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
    fn try_from_row(row: BundleRow) -> ContentResult<Self> {
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
    pub(crate) archive_cache: RwLock<HashMap<PathBuf, crate::bundles::types::BundleManifest>>,
}

impl BundlesManager {
    pub fn new() -> Self {
        Self {
            manifest: RwLock::new(RemoteBundleManifest::default()),
            archive_cache: RwLock::new(HashMap::new()),
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn sync(&self, ctx: &ContentCtx) -> ContentResult<bool> {
        let Some(fetched) = Self::fetch_manifest(ctx).await? else {
            tracing::debug!(
                "skipping bundle sync because no remote or cached manifest is available"
            );
            return Ok(false);
        };
        let FetchedManifest {
            manifest,
            changed: manifest_changed,
            etag,
        } = fetched;
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

        bundle_dao::hide_bundles_not_in(&ctx.db, &remote_paths).await?;

        let bundles_root = paths::bundles_dir()?;
        polyio::create_dir_all(&bundles_root.join("bundles")).await?;

        let mut synced_all = true;
        for entry in manifest.remote_paths() {
            if let Err(err) = self.sync_bundle(&entry, ctx, &bundles_root).await {
                synced_all = false;
                tracing::warn!(
                    remote_path = %entry.remote_path,
                    error = %err,
                    "failed to sync bundle"
                );
            }
        }

        if synced_all {
            if let Some(etag) = etag {
                let manifest_path = paths::bundles_dir()?.join("metadata.json");
                oneclient_net::commit_etag(&manifest_path, &etag).await;
            }
        } else {
            tracing::warn!(
                "bundle catalog incomplete; not caching the manifest etag so the next \
                 sync retries the missing bundles"
            );
        }

        Ok(true)
    }

    pub async fn cache_archive_manifest(
        &self,
        path: std::path::PathBuf,
        manifest: crate::bundles::types::BundleManifest,
    ) {
        self.archive_cache.write().await.insert(path, manifest);
    }

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    pub async fn archives_for(
        &self,
        ctx: &ContentCtx,
        mc_version: &str,
        loader: GameLoader,
    ) -> ContentResult<Vec<BundleArchive>> {
        let mut archives = Vec::new();
        for bundle in self.list_for(ctx, mc_version, loader).await? {
            let manifest = self.manifest_for_archive(&bundle.path).await?;
            archives.push(BundleArchive { bundle, manifest });
        }
        Ok(archives)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn manifest_for_archive(
        &self,
        path: &Path,
    ) -> ContentResult<crate::bundles::types::BundleManifest> {
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

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    pub async fn list_for(
        &self,
        ctx: &ContentCtx,
        mc_version: &str,
        loader: GameLoader,
    ) -> ContentResult<Vec<Bundle>> {
        let rows =
            bundle_dao::list_visible_for_version_loader(&ctx.db, mc_version, loader as i64)
                .await?;

        rows.into_iter()
            .map(Bundle::try_from_row)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(level = "debug", skip(self, entry, ctx, bundles_root), fields(remote_path = %entry.remote_path))]
    async fn sync_bundle(
        &self,
        entry: &RemoteBundleRef,
        ctx: &ContentCtx,
        bundles_root: &Path,
    ) -> ContentResult<()> {
        let loader = GameLoader::from_str(&entry.loader)
            .map_err(|_| BundleError::UnknownLoader(entry.loader.clone()))?;

        let file_name = entry
            .remote_path
            .split('/')
            .next_back()
            .ok_or_else(|| BundleError::InvalidPath(entry.remote_path.clone()))?
            .to_string();

        let disk_path = bundles_root.join("bundles").join(&file_name);
        let remote_url = format!("{}{}", ctx.net.config().meta_url_base, entry.remote_path);

        download_bundle_if_needed(ctx, &remote_url, &disk_path, &entry.sha1).await?;

        let meta = polymrpack::read_meta_from_archive(&disk_path).await?;

        let relative_disk_path = disk_path
            .strip_prefix(paths::launcher_dir()?)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| disk_path.to_string_lossy().into_owned());

        let synced_at = Utc::now().to_rfc3339();

        bundle_dao::upsert_bundle(
            &ctx.db,
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

    /// Defers the ETag commit
    /// bundles download after the catalog so an early ETag would 304 past
    /// bundles that never arrived
    /// `sync` commits it once all land
    #[tracing::instrument(level = "debug", skip(ctx))]
    async fn fetch_manifest(
        ctx: &ContentCtx,
    ) -> ContentResult<Option<FetchedManifest>> {
        let manifest_path = paths::bundles_dir()?.join("metadata.json");
        let url = format!(
            "{}/oneclient/bundles/metadata.json",
            ctx.net.config().meta_url_base
        );

        let Some(fetched) =
            fetch_cached(&ctx.net, &url, &manifest_path, EtagPolicy::Defer).await?
        else {
            return Ok(None);
        };

        Ok(Some(FetchedManifest {
            manifest: fetched.json()?,
            changed: fetched.changed,
            etag: fetched.etag,
        }))
    }
}

impl Default for BundlesManager {
    fn default() -> Self {
        Self::new()
    }
}

struct FetchedManifest {
    manifest: RemoteBundleManifest,
    changed: bool,
    etag: Option<String>,
}


#[tracing::instrument(level = "debug", skip(ctx))]
async fn download_bundle_if_needed(
    ctx: &ContentCtx,
    url: &str,
    disk_path: &Path,
    expected_sha1: &str,
) -> ContentResult<()> {
    if oneclient_net::matches_on_disk(disk_path, expected_sha1).await {
        tracing::debug!("bundle cache hit via SHA1: {url}");
        return Ok(());
    }

    tracing::debug!("downloading bundle from remote: {url}");
    let expected = polyio::Checksum::sha1(expected_sha1);
    oneclient_net::download_verified(
        &ctx.net,
        &ctx.events,
        url,
        disk_path,
        Some(&expected),
        0,
        None,
    )
    .await
    .map_err(map_request_error)
}


fn map_request_error(err: RequestError) -> ContentError {
    match err {
        RequestError::DeserializeError {
            source,
            type_name,
            url,
            status,
            snippet,
        } => ContentError::InvalidData {
            reason: format!(
                "failed to parse {type_name} from {url} (HTTP {status}): {source}; body: {snippet}"
            ),
        },
        other => ContentError::InvalidData {
            reason: other.to_string(),
        },
    }
}
