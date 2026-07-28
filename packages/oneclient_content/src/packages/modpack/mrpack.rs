use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_lite::AsyncReadExt;
use oneclient_db::models::ClusterRow;
use serde::Deserialize;
use uuid::Uuid;

use oneclient_common::domain::ContentType;
use crate::packages::error::PackageError;
use crate::packages::store::{self, PackageStore};
use crate::packages::types::ExternalFile;
use crate::ctx::ContentCtx;
use crate::error::ContentResult;

pub struct MrpackInstaller;

#[derive(Deserialize)]
#[allow(dead_code)]
struct MrpackManifest {
    #[serde(default)]
    files: Vec<MrpackFileEntry>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MrpackFileEntry {
    path: String,
    hashes: MrpackHashes,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    file_size: u64,
}

#[derive(Deserialize)]
struct MrpackHashes {
    sha1: String,
}

impl MrpackInstaller {
    #[tracing::instrument(skip(ctx))]
    pub async fn install_archive(
        archive_path: PathBuf,
        cluster_id: i64,
        ctx: &ContentCtx,
    ) -> ContentResult<()> {
        tracing::info!("installing mrpack archive to cluster");
        let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
        let cluster_root = oneclient_common::paths::clusters_dir()?.join(&cluster.folder_name);

        let bytes = Arc::new(polyio::read(archive_path).await?);

        let mut manifest_bytes: Option<Vec<u8>> = None;

        polyio::read_zip_entries_bytes(bytes.to_vec(), async |_, entry, reader| {
            let entry_name = entry.filename().as_str().map_err(io_err)?;

            if entry_name == "modrinth.index.json" {
                let mut buf = Vec::new();
                reader.read_to_end(&mut buf).await.map_err(io_err)?;
                manifest_bytes = Some(buf);
            }
            Ok(())
        })
        .await?;

        let manifest_bytes = manifest_bytes.ok_or(PackageError::UnsupportedModpackFormat)?;
        let manifest: MrpackManifest = serde_json::from_slice(&manifest_bytes)?;

        let mut failed = 0u64;
        let total = manifest.files.len() as u64;
        let progress_id = Uuid::new_v4();

        for (index, entry) in manifest.files.into_iter().enumerate() {
            ctx.events.progress(
                progress_id,
                "Installing Modpack Files",
                index as u64,
                total,
            );

            let content_type = content_type_from_path(&entry.path);
            let hash = entry.hashes.sha1.to_ascii_lowercase();
            let file_name = Path::new(&entry.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&entry.path)
                .to_string();

            let path_str = entry.path.clone();

            if let Err(err) = install_mrpack_file(
                entry,
                content_type,
                hash,
                file_name,
                &cluster,
                ctx,
            )
            .await
            {
                failed += 1;
                tracing::warn!(path = %path_str, error = %err, "modpack file install failed");
            }
        }

        ctx.events
            .progress(progress_id, "Installing Modpack Files", total, total);

        polyio::read_zip_entries_bytes(bytes.to_vec(), async |_, entry, reader| {
            let name = entry.filename().as_str().map_err(io_err)?;

            let Some(rest) = name.strip_prefix("overrides/") else {
                return Ok(());
            };
            if rest.is_empty() || name.ends_with('/') {
                return Ok(());
            }

            let dest = cluster_root.join(rest);
            if let Some(parent) = dest.parent() {
                polyio::create_dir_all(parent).await.map_err(io_err)?;
            }

            let mut file_bytes = Vec::new();
            reader.read_to_end(&mut file_bytes).await.map_err(io_err)?;
            polyio::write(&dest, &file_bytes).await.map_err(io_err)?;
            Ok(())
        })
        .await?;

        if failed > 0 {
            tracing::error!(failed, total, "modpack install completed with failures");
            return Err(PackageError::PartialModpackInstall { failed, total }.into());
        }

        tracing::info!(total, "modpack install complete");
        Ok(())
    }
}

#[tracing::instrument(level = "debug", skip(entry, cluster, ctx))]
async fn install_mrpack_file(
    entry: MrpackFileEntry,
    content_type: ContentType,
    hash: String,
    file_name: String,
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    if let Some(row) =
        oneclient_db::dao::artifact::get_artifact_by_hash(&ctx.db, &hash).await?
    {
        let path = store::artifact_absolute_path(&row.path)?;
        if path.exists() {
            PackageStore::link_artifact(&row, cluster, Some(&file_name), ctx).await?;
            return Ok(());
        }
    }

    if let Some((provider_id, version)) =
        ctx.providers.lookup_version(&hash, ctx).await?
    {
        let provider = ctx.providers.get(provider_id)?;
        let project = provider
            .get_project(&version.project_id, ctx)
            .await?;
        let artifact = PackageStore::download_and_cache(
            provider_id,
            &project,
            &version,
            false,
            None,
            ctx,
        )
        .await?;
        PackageStore::link_artifact(&artifact, cluster, Some(&file_name), ctx).await?;
        return Ok(());
    }

    let url = entry
        .downloads
        .first()
        .cloned()
        .ok_or(PackageError::NoPrimaryFile)?;

    let external = ExternalFile {
        name: file_name.clone(),
        url,
        sha1: hash,
        size: entry.file_size,
        content_type,
    };

    let artifact = store::download_external(&external, false, None, ctx).await?;
    PackageStore::link_artifact(&artifact, cluster, Some(&file_name), ctx).await?;

    Ok(())
}

fn content_type_from_path(path: &str) -> ContentType {
    let top = path.split('/').next().unwrap_or("");
    ContentType::from_folder_name(top).unwrap_or(ContentType::Mod)
}

fn io_err(err: impl std::error::Error + Send + Sync + 'static) -> polyio::IOError {
    polyio::IOError::PathIOError {
        source: std::io::Error::other(err),
        path: String::new(),
    }
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn install_mrpack_to_cluster(
    archive_path: PathBuf,
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    MrpackInstaller::install_archive(archive_path, cluster_id, ctx).await
}
