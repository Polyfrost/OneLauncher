use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;

use crate::LauncherResult;
use crate::bundles::error::BundleError;
use crate::bundles::types::{
    BundleFile, BundleFileKind, BundleManifest, content_type_from_bundle_path,
};
use crate::constants::MODRINTH_CDN_PREFIX;
use crate::packages::domain::{GameLoader, ProviderId};
use crate::packages::types::ExternalFile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolyMrpackMeta {
    pub name: String,
    pub version_id: String,
    pub category: String,
    pub enabled: bool,
    pub mc_version: String,
    pub loader: GameLoader,
    pub loader_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PolyMrpackManifest {
    pub category: String,
    pub enabled: bool,
    pub version_id: String,
    pub name: String,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(default)]
    pub files: Vec<PolyMrpackFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PolyMrpackFile {
    pub path: String,
    pub hashes: PolyMrpackHashes,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    pub file_size: u64,
    pub enabled: bool,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Deserialize)]
struct PolyMrpackHashes {
    pub sha1: String,
}

#[tracing::instrument(level = "debug")]
pub async fn read_meta_from_archive(path: &Path) -> LauncherResult<PolyMrpackMeta> {
    let manifest = read_manifest_from_archive(path).await?;
    Ok(PolyMrpackMeta {
        name: manifest.name,
        version_id: manifest.version_id,
        category: manifest.category,
        enabled: manifest.enabled,
        mc_version: manifest.mc_version,
        loader: manifest.loader,
        loader_version: manifest.loader_version,
    })
}

#[tracing::instrument(level = "debug")]
pub async fn read_manifest_from_archive(path: &Path) -> LauncherResult<BundleManifest> {
    let file = tokio::fs::File::open(path).await?;
    let reader = tokio::io::BufReader::new(file);
    let manifest_bytes = polyio::try_read_zip_entry_bytes(reader, "modrinth.index.json").await?;
    parse_manifest_bytes(&manifest_bytes)
}

fn parse_manifest_bytes(bytes: &[u8]) -> LauncherResult<BundleManifest> {
    let manifest: PolyMrpackManifest =
        serde_json::from_slice(bytes).map_err(|_| BundleError::InvalidManifest)?;

    let (mc_version, loader, loader_version) = parse_dependencies(&manifest.dependencies)?;
    let files = manifest
        .files
        .iter()
        .filter_map(parse_bundle_file)
        .collect();

    Ok(BundleManifest {
        name: manifest.name,
        version_id: manifest.version_id,
        category: manifest.category,
        mc_version,
        loader,
        loader_version,
        enabled: manifest.enabled,
        files,
    })
}

fn parse_dependencies(
    deps: &HashMap<String, String>,
) -> LauncherResult<(String, GameLoader, String)> {
    let mut mc_version = None;
    let mut loader = None;
    let mut loader_version = None;

    for (key, value) in deps {
        let normalized = key.to_lowercase().replace(['_', '.', ' ', '-'], "");
        if normalized == "minecraft" {
            mc_version = Some(value.clone());
        } else if let Ok(parsed) = GameLoader::from_str(&normalized) {
            loader = Some(parsed);
            loader_version = Some(value.clone());
        }
    }

    Ok((
        mc_version.ok_or(BundleError::InvalidManifest)?,
        loader.ok_or(BundleError::InvalidManifest)?,
        loader_version.ok_or(BundleError::InvalidManifest)?,
    ))
}

fn parse_bundle_file(file: &PolyMrpackFile) -> Option<BundleFile> {
    if let Some(url) = file
        .downloads
        .iter()
        .find(|url| url.starts_with(MODRINTH_CDN_PREFIX))
    {
        let paths = url[MODRINTH_CDN_PREFIX.len()..]
            .split('/')
            .collect::<Vec<_>>();
        if paths.len() >= 4 {
            return Some(BundleFile {
                enabled: file.enabled,
                hidden: file.hidden,
                path: file.path.clone(),
                size: file.file_size,
                kind: BundleFileKind::Managed {
                    provider: ProviderId::Modrinth,
                    project_id: paths[0].to_string(),
                    version_id: paths[2].to_string(),
                    sha1: file.hashes.sha1.to_ascii_lowercase(),
                },
            });
        }
        tracing::error!("invalid modrinth file URL in bundle: '{url}'");
        return None;
    }

    let download_url = file.downloads.first().cloned()?;
    let file_name = file
        .path
        .split('/')
        .next_back()
        .unwrap_or(&file.path)
        .to_string();

    Some(BundleFile {
        enabled: file.enabled,
        hidden: file.hidden,
        path: file.path.clone(),
        size: file.file_size,
        kind: BundleFileKind::External(ExternalFile {
            name: file_name,
            url: download_url,
            sha1: file.hashes.sha1.to_ascii_lowercase(),
            size: file.file_size,
            content_type: content_type_from_bundle_path(&file.path),
        }),
    })
}
