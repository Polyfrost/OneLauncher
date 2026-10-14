use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;

use crate::bundles::error::BundleError;
use crate::bundles::types::{
    BundleFile, BundleFileKind, BundleFileType, BundleManifest, ExternalFileMeta,
    content_type_from_bundle_path,
};
use crate::error::ContentResult;
use crate::packages::types::ExternalFile;
use oneclient_common::constants::MODRINTH_CDN_PREFIX;
use oneclient_common::domain::{GameLoader, ProviderId};

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
    pub java_version_override: Option<u32>,
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
    #[serde(rename = "type", default)]
    pub file_type: Option<BundleFileType>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub overrides: Option<PolyMrpackFileOverrides>,
}

#[derive(Debug, Default, Deserialize)]
struct PolyMrpackFileOverrides {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PolyMrpackHashes {
    pub sha1: String,
}

#[tracing::instrument(level = "debug")]
pub async fn read_meta_from_archive(path: &Path) -> ContentResult<PolyMrpackMeta> {
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
pub async fn read_manifest_from_archive(path: &Path) -> ContentResult<BundleManifest> {
    let file = tokio::fs::File::open(path).await?;
    let reader = tokio::io::BufReader::new(file);
    let manifest_bytes = polyio::try_read_zip_entry_bytes(reader, "modrinth.index.json").await?;
    parse_manifest_bytes(&manifest_bytes)
}

fn parse_manifest_bytes(bytes: &[u8]) -> ContentResult<BundleManifest> {
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
        java_version_override: manifest.java_version_override,
        files,
    })
}

pub(crate) struct PackDependencies {
    pub(crate) mc_version: Option<String>,
    pub(crate) loader: Option<(GameLoader, String)>,
}

pub(crate) fn scan_dependencies(
    deps: &HashMap<String, String>,
    accept: impl Fn(GameLoader) -> bool,
) -> PackDependencies {
    let mut scanned = PackDependencies {
        mc_version: None,
        loader: None,
    };

    for (key, value) in deps {
        let normalized = key.to_lowercase().replace(['_', '.', ' ', '-'], "");
        if normalized == "minecraft" {
            scanned.mc_version = Some(value.clone());
        } else if let Ok(parsed) = GameLoader::from_str(&normalized)
            && accept(parsed)
        {
            scanned.loader = Some((parsed, value.clone()));
        }
    }

    scanned
}

fn parse_dependencies(
    deps: &HashMap<String, String>,
) -> ContentResult<(String, GameLoader, String)> {
    let scanned = scan_dependencies(deps, |_| true);
    let (loader, loader_version) = scanned.loader.ok_or(BundleError::InvalidManifest)?;

    Ok((
        scanned.mc_version.ok_or(BundleError::InvalidManifest)?,
        loader,
        loader_version,
    ))
}

fn parse_bundle_file(file: &PolyMrpackFile) -> Option<BundleFile> {
    let mut kind = mrpack_file_kind(
        &file.path,
        &file.downloads,
        &file.hashes.sha1,
        file.file_size,
    )?;

    if let BundleFileKind::External { id, meta, .. } = &mut kind {
        *id = non_blank(file.id.as_deref()).map(|id| {
            if id.starts_with(EXTERNAL_ID_PREFIX) {
                id
            } else {
                format!("{EXTERNAL_ID_PREFIX}{id}")
            }
        });
        *meta = file.overrides.as_ref().and_then(external_meta);
    }

    Some(BundleFile {
        enabled: file.enabled,
        hidden: file.hidden,
        path: file.path.clone(),
        size: file.file_size,
        file_type: file.file_type.unwrap_or_default(),
        kind,
    })
}

pub(crate) fn mrpack_file_kind(
    path: &str,
    downloads: &[String],
    sha1: &str,
    size: u64,
) -> Option<BundleFileKind> {
    let sha1 = sha1.to_ascii_lowercase();

    if let Some(url) = downloads
        .iter()
        .find(|url| url.starts_with(MODRINTH_CDN_PREFIX))
    {
        let paths = url[MODRINTH_CDN_PREFIX.len()..]
            .split('/')
            .collect::<Vec<_>>();
        if paths.len() >= 4 {
            return Some(BundleFileKind::Managed {
                provider: ProviderId::Modrinth,
                project_id: paths[0].to_string(),
                version_id: paths[2].to_string(),
                sha1,
            });
        }
        tracing::error!("invalid modrinth file URL in bundle: '{url}'");
        return None;
    }

    let download_url = downloads.first().cloned()?;
    let file_name = path.split('/').next_back().unwrap_or(path).to_string();

    Some(BundleFileKind::External {
        file: ExternalFile {
            name: file_name,
            url: download_url,
            sha1,
            size,
            content_type: content_type_from_bundle_path(path),
        },
        id: None,
        meta: None,
    })
}

const EXTERNAL_ID_PREFIX: &str = "ext:";

fn non_blank(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn external_meta(overrides: &PolyMrpackFileOverrides) -> Option<ExternalFileMeta> {
    let meta = ExternalFileMeta {
        name: non_blank(overrides.name.as_deref()),
        description: non_blank(overrides.description.as_deref()),
        authors: overrides
            .authors
            .iter()
            .filter_map(|author| non_blank(Some(author)))
            .collect(),
        icon_url: non_blank(overrides.icon.as_deref()),
    };
    (meta != ExternalFileMeta::default()).then_some(meta)
}
