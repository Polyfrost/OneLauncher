use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use oneclient_db::dao::bundle as bundle_dao;
use oneclient_db::dao::package_metadata as meta_dao;
use oneclient_db::models::{BundleRow, NewBundle};
use tokio::sync::{Mutex, RwLock};

use crate::bundles::error::BundleError;
use crate::bundles::manifest::{BundleManifest as RemoteBundleManifest, RemoteBundleRef};
use crate::bundles::polymrpack::{self, PolyMrpackMeta};
use crate::bundles::types::{BundleArchive, BundleFileKind};
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};
use oneclient_common::domain::{GameLoader, ProviderId};
use oneclient_common::paths;
use oneclient_net::RequestError;
use oneclient_net::{EtagPolicy, fetch_cached};

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

        let launcher_dir = paths::data_dir()?;
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
    catalog: RwLock<()>,
    catalog_revision: AtomicU64,
    syncing: Arc<Mutex<()>>,
}

struct StagedBundle {
    entry: RemoteBundleRef,
    loader: GameLoader,
    file_name: String,
    disk_path: PathBuf,
    download: Option<PathBuf>,
    meta: PolyMrpackMeta,
}

impl BundlesManager {
    pub fn new() -> Self {
        Self {
            manifest: RwLock::new(RemoteBundleManifest::default()),
            archive_cache: RwLock::new(HashMap::new()),
            catalog: RwLock::new(()),
            catalog_revision: AtomicU64::new(0),
            syncing: Arc::new(Mutex::new(())),
        }
    }

    pub fn catalog_revision(&self) -> u64 {
        self.catalog_revision.load(Ordering::Acquire)
    }

    #[tracing::instrument(skip_all)]
    pub async fn sync(self: &Arc<Self>, ctx: &ContentCtx) -> ContentResult<bool> {
        let syncing = Arc::clone(&self.syncing).lock_owned().await;

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

        let entries: Vec<RemoteBundleRef> = manifest
            .remote_paths()
            .into_iter()
            .filter(|entry| {
                let known = GameLoader::from_str(&entry.loader).is_ok();
                if !known {
                    tracing::warn!(
                        remote_path = %entry.remote_path,
                        loader = %entry.loader,
                        "skipping a bundle for an unknown loader"
                    );
                }
                known
            })
            .collect();
        let remote_paths: Vec<String> = entries
            .iter()
            .map(|entry| entry.remote_path.clone())
            .collect();

        if remote_paths.is_empty() {
            tracing::error!("bundle catalog listed no bundles; keeping the previous catalog");
            return Ok(false);
        }

        let bundles_root = paths::bundles_dir()?;
        polyio::create_dir_all(&bundles_root.join("bundles")).await?;

        let mut staged = Vec::new();
        let mut staged_all = true;
        for entry in &entries {
            let archive = archive_file_name(entry, &entries);
            match Self::stage_bundle(entry, &archive, ctx, &bundles_root).await {
                Ok(bundle) => staged.push(bundle),
                Err(err) => {
                    staged_all = false;
                    tracing::warn!(
                        remote_path = %entry.remote_path,
                        error = %err,
                        "failed to sync bundle"
                    );
                }
            }
        }

        if !staged_all {
            tracing::warn!(
                "bundle catalog incomplete; keeping the previous catalog and not caching \
                 the manifest etag so the next sync retries"
            );
            return Ok(false);
        }

        let bundles = Arc::clone(self);
        let publishing = ctx.clone();
        let published_all = tokio::spawn(async move {
            let _syncing = syncing;
            bundles.publish(staged, &remote_paths, &publishing).await
        })
        .await
        .map_err(|err| ContentError::InvalidData {
            reason: format!("bundle catalog publish stopped: {err}"),
        })??;

        if published_all {
            if let Some(etag) = etag {
                let manifest_path = paths::bundles_dir()?.join("metadata.json");
                oneclient_net::commit_etag(&manifest_path, &etag).await;
            }
        } else {
            tracing::warn!(
                "bundle catalog partly published; not caching the manifest etag so the \
                 next sync retries the missing bundles"
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
        let _catalog = self.catalog.read().await;
        let mut archives = Vec::new();
        for bundle in self.list_for(ctx, mc_version, loader).await? {
            let manifest = self.manifest_for_archive(ctx, &bundle.path).await?;
            archives.push(BundleArchive { bundle, manifest });
        }
        Ok(archives)
    }

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    async fn manifest_for_archive(
        &self,
        ctx: &ContentCtx,
        path: &Path,
    ) -> ContentResult<crate::bundles::types::BundleManifest> {
        if let Some(manifest) = self.archive_cache.read().await.get(path) {
            return Ok(manifest.clone());
        }
        let manifest = polymrpack::read_manifest_from_archive(path).await?;
        record_external_metadata(&manifest, ctx).await;
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
            bundle_dao::list_visible_for_version_loader(&ctx.db, mc_version, loader as i64).await?;

        rows.into_iter()
            .map(Bundle::try_from_row)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn publish(
        &self,
        staged: Vec<StagedBundle>,
        remote_paths: &[String],
        ctx: &ContentCtx,
    ) -> ContentResult<bool> {
        let _catalog = self.catalog.write().await;
        bundle_dao::hide_bundles_not_in(&ctx.db, remote_paths).await?;

        let mut published_all = true;
        for bundle in staged {
            if let Err(err) = Self::publish_bundle(&bundle, ctx).await {
                published_all = false;
                tracing::warn!(
                    remote_path = %bundle.entry.remote_path,
                    error = %err,
                    "failed to publish bundle"
                );
            }
        }

        self.archive_cache.write().await.clear();
        self.catalog_revision.fetch_add(1, Ordering::Release);
        Ok(published_all)
    }

    #[tracing::instrument(level = "debug", skip(entry, ctx, bundles_root), fields(remote_path = %entry.remote_path))]
    async fn stage_bundle(
        entry: &RemoteBundleRef,
        archive: &str,
        ctx: &ContentCtx,
        bundles_root: &Path,
    ) -> ContentResult<StagedBundle> {
        let loader = GameLoader::from_str(&entry.loader)
            .map_err(|_| BundleError::UnknownLoader(entry.loader.clone()))?;

        let file_name = remote_file_name(&entry.remote_path).to_string();
        let disk_path = bundles_root.join("bundles").join(archive);
        let remote_url = format!("{}{}", ctx.net.config().meta_url_base, entry.remote_path);

        let download = if oneclient_net::matches_on_disk(&disk_path, &entry.sha1).await {
            None
        } else {
            let path = disk_path.with_file_name(format!("{archive}.staged"));
            download_bundle_if_needed(ctx, &remote_url, &path, &entry.sha1).await?;
            Some(path)
        };

        let meta =
            polymrpack::read_meta_from_archive(download.as_ref().unwrap_or(&disk_path)).await?;

        Ok(StagedBundle {
            entry: entry.clone(),
            loader,
            file_name,
            disk_path,
            download,
            meta,
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(remote_path = %bundle.entry.remote_path))]
    async fn publish_bundle(bundle: &StagedBundle, ctx: &ContentCtx) -> ContentResult<()> {
        let StagedBundle {
            entry,
            loader,
            file_name,
            disk_path,
            download,
            meta,
        } = bundle;

        if let Some(download) = download {
            polyio::rename(download, disk_path).await?;
        }

        let relative_disk_path = disk_path
            .strip_prefix(paths::data_dir()?)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| disk_path.to_string_lossy().into_owned());

        let synced_at = Utc::now().to_rfc3339();

        bundle_dao::upsert_bundle(
            &ctx.db,
            NewBundle {
                remote_path: &entry.remote_path,
                mc_version: &entry.mc_version,
                mc_loader: *loader as i64,
                file_name,
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
    async fn fetch_manifest(ctx: &ContentCtx) -> ContentResult<Option<FetchedManifest>> {
        let manifest_path = paths::bundles_dir()?.join("metadata.json");
        let url = format!(
            "{}/oneclient/bundles/metadata.json",
            ctx.net.config().meta_url_base
        );

        let Some(fetched) = fetch_cached(&ctx.net, &url, &manifest_path, EtagPolicy::Defer).await?
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

fn remote_file_name(remote_path: &str) -> &str {
    remote_path.rsplit('/').next().unwrap_or(remote_path)
}

fn archive_file_name(entry: &RemoteBundleRef, entries: &[RemoteBundleRef]) -> String {
    let name = remote_file_name(&entry.remote_path);
    let sharing = entries
        .iter()
        .filter(|other| remote_file_name(&other.remote_path) == name)
        .count();

    if sharing > 1 {
        let path = polyio::sha1_bytes(entry.remote_path.as_bytes());
        format!("{path}-{name}")
    } else {
        name.to_string()
    }
}

#[tracing::instrument(level = "debug", skip_all, fields(bundle = %manifest.name))]
async fn record_external_metadata(
    manifest: &crate::bundles::types::BundleManifest,
    ctx: &ContentCtx,
) {
    for bundle_file in &manifest.files {
        let BundleFileKind::External {
            file,
            meta: Some(meta),
            ..
        } = &bundle_file.kind
        else {
            continue;
        };

        if let Err(err) = meta_dao::upsert_package_metadata(
            &ctx.db,
            ProviderId::Local as i64,
            &file.sha1,
            meta.name.as_deref().unwrap_or(&file.name),
            meta.description.as_deref().unwrap_or_default(),
            &meta.authors.join(", "),
            meta.icon_url.as_deref(),
        )
        .await
        {
            tracing::warn!(file = %file.name, error = %err, "failed to record bundle file metadata");
        }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use oneclient_events::EventBus;
    use oneclient_net::{NetConfig, RequestClient};

    use super::*;
    use crate::packages::provider::PackageProviderRegistry;

    #[test]
    fn entries_sharing_a_name_get_their_own_archive() {
        let entry = |remote_path: &str| RemoteBundleRef {
            mc_version: "1.21.1".to_string(),
            loader: "fabric".to_string(),
            remote_path: remote_path.to_string(),
            sha1: String::new(),
        };
        let entries = [
            entry("/a/hud.mrpack"),
            entry("/b/hud.mrpack"),
            entry("/a/qol.mrpack"),
        ];

        let first = archive_file_name(&entries[0], &entries);
        let second = archive_file_name(&entries[1], &entries);

        assert_ne!(first, second);
        assert!(first.ends_with("-hud.mrpack"));
        assert_eq!(archive_file_name(&entries[2], &entries), "qol.mrpack");
    }

    #[tokio::test]
    async fn a_catalog_read_waits_out_a_publish() {
        let dir = polyio::tempdir().await.unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let ctx = ContentCtx::new(
            oneclient_db::connect(dir.dir_path().join("test.db"))
                .await
                .unwrap(),
            RequestClient::new(NetConfig::default()).unwrap(),
            EventBus::new(tx),
            PackageProviderRegistry::new(),
        );

        let bundles = BundlesManager::new();
        let publishing = bundles.catalog.write().await;
        let read = bundles.archives_for(&ctx, "1.21.1", GameLoader::Fabric);
        tokio::pin!(read);

        let budget = Duration::from_millis(50);
        assert!(tokio::time::timeout(budget, &mut read).await.is_err());

        drop(publishing);
        assert!(read.await.unwrap().is_empty());
    }
}
