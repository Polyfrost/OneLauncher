use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::{ArtifactRow, ClusterKind, ClusterRow, OverrideType};
use oneclient_events::{GroupedProgressChild, GroupedProgressSession, TaskCategory, TaskPhase};

use super::install::install_loose_file;
use super::{
    LooseFile, MODPACK_BUNDLE_NAME, ModpackInstallReport, ModpackManifest, is_safe_relative_path,
    tracked_content_type, zip_entry,
};
use crate::bundles::BundleFileKind;
use crate::bundles::overrides::{OverrideLayers, sync_layered_overrides};
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};
use crate::packages::FileIdentity;
use crate::packages::store::{self, PackageStore, artifact_absolute_path};
use crate::packages::store::{cache_file_path, relative_cache_path};
use crate::packages::types::{ExternalFile, VersionDetail};
use oneclient_common::constants::MODRINTH_CDN_PREFIX;

pub const SHARE_ENTRY: &str = "oneclient.cluster.json";
pub const SHARE_FORMAT: u32 = 1;
pub const SHARE_EXTENSION: &str = "mrpack";

const INDEX_ENTRY: &str = "modrinth.index.json";
const OVERRIDES: &str = "overrides/";
const ICON_STEM: &str = "icon";
const ICON_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];
const MAX_ICON_BYTES: usize = 8 * 1024 * 1024;
const DOWNLOAD_CONCURRENCY: usize = 6;
const SHARED_OVERRIDES_KEY: &str = "shared";
const COPY_CHUNK: usize = 256 * 1024;
const INDEX_HOSTS: &[&str] = &[
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];
const EXTRA_HOSTS: &[&str] = &["edge.forgecdn.net", "mediafilez.forgecdn.net"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterShare {
    pub format: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub kind: ClusterKind,
    pub mc_version: String,
    pub mc_loader: GameLoader,
    #[serde(default)]
    pub mc_loader_version: Option<String>,
    #[serde(default)]
    pub dedicated_dir: bool,
    #[serde(default)]
    pub java: Option<SharedJava>,
    #[serde(default)]
    pub modpack: Option<SharedModpack>,
    #[serde(default)]
    pub bundles: Vec<SharedBundleChoice>,
    #[serde(default)]
    pub overrides: Vec<SharedOverride>,
    #[serde(default)]
    pub disabled: Vec<String>,
    #[serde(default)]
    pub downloads: Vec<SharedDownload>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedDownload {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

impl SharedDownload {
    fn external(&self) -> Option<ExternalFile> {
        let content_type = tracked_content_type(&self.path)?;
        let name = self
            .path
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty() && *name != "." && *name != "..")?
            .to_string();
        Some(ExternalFile {
            name,
            url: self.url.clone(),
            sha1: polyio::normalize_hash(&self.sha1),
            size: self.size,
            content_type,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShareExtras {
    pub embedded: Vec<ContentType>,
    pub downloads: Vec<(ContentType, u64)>,
    pub other_files: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackCarryover {
    pub archive: PathBuf,
    pub game_dir: PathBuf,
    pub files: Vec<(String, String)>,
    pub loose: Vec<LooseFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedJava {
    pub major: u32,
    #[serde(default)]
    pub vendor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedModpack {
    pub provider: ProviderId,
    pub project_id: String,
    pub version_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedBundleChoice {
    pub name: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedOverride {
    pub bundle: String,
    pub package_id: String,
    pub state: String,
}

impl SharedOverride {
    #[must_use]
    pub fn override_type(&self) -> Option<OverrideType> {
        OverrideType::parse(&self.state)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedFileSource {
    Download { url: String, local: Option<PathBuf> },
    Embed(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedFile {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub enabled: bool,
    pub source: SharedFileSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShareScope {
    pub skip_bundles: bool,
    pub skip_modpack: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ShareContent {
    pub files: Vec<SharedFile>,
    pub overrides: Vec<SharedOverride>,
    pub bundles: Vec<SharedBundleChoice>,
    pub missing: Vec<String>,
    pub scanned_bytes: u64,
    pub carryover: Option<PackCarryover>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShareExportReport {
    pub linked: usize,
    pub embedded: Vec<String>,
    pub missing: Vec<String>,
    pub carried: usize,
}

#[derive(Debug, Clone)]
pub struct ShareIcon {
    pub extension: String,
    pub bytes: Vec<u8>,
}

struct ExportMeter<'a> {
    child: Option<&'a GroupedProgressChild>,
    done: u64,
    total: u64,
}

impl<'a> ExportMeter<'a> {
    fn new(child: Option<&'a GroupedProgressChild>, done: u64, total: u64) -> Self {
        let meter = Self {
            child,
            done,
            total: total.max(1),
        };
        meter.publish();
        meter
    }

    fn advance(&mut self, size: u64) {
        self.done = (self.done + weight(size)).min(self.total);
        self.publish();
    }

    fn publish(&self) {
        if let Some(child) = self.child {
            child.set_progress(self.done, Some(self.total));
        }
    }
}

fn weight(size: u64) -> u64 {
    size.max(1)
}

struct Candidate {
    path: String,
    file_name: String,
    hash: String,
    enabled: bool,
    local: Option<(PathBuf, u64)>,
    known_url: Option<String>,
}

impl Candidate {
    fn size(&self) -> u64 {
        self.local.as_ref().map_or(0, |(_, size)| *size)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexOut<'a> {
    format_version: u32,
    game: &'static str,
    version_id: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<&'a str>,
    files: Vec<IndexFileOut>,
    dependencies: BTreeMap<&'static str, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexFileOut {
    path: String,
    hashes: BTreeMap<&'static str, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<BTreeMap<&'static str, &'static str>>,
    downloads: Vec<String>,
    file_size: u64,
}

#[tracing::instrument(level = "debug")]
pub async fn read_cluster_share(path: &Path) -> ContentResult<Option<ClusterShare>> {
    let Some(bytes) = zip_entry(path, SHARE_ENTRY).await? else {
        return Ok(None);
    };
    let share: ClusterShare = serde_json::from_slice(&bytes)?;
    if share.format > SHARE_FORMAT {
        return Err(ContentError::InvalidData {
            reason: format!(
                "This instance was exported by a newer OneClient (format {}). Update OneClient to import it.",
                share.format
            ),
        });
    }
    Ok(Some(share))
}

#[tracing::instrument(level = "debug", skip(share))]
pub async fn read_share_icon(path: &Path, share: &ClusterShare) -> Option<ShareIcon> {
    let entry = share.icon.as_deref()?;
    let extension = icon_extension(entry)?;
    let bytes = zip_entry(path, entry).await.ok().flatten()?;
    (bytes.len() <= MAX_ICON_BYTES).then_some(ShareIcon { extension, bytes })
}

fn icon_extension(entry: &str) -> Option<String> {
    let (stem, extension) = entry.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    (stem == ICON_STEM && ICON_EXTENSIONS.contains(&extension.as_str())).then_some(extension)
}

#[must_use]
pub fn icon_entry(extension: &str) -> Option<String> {
    let extension = extension.to_ascii_lowercase();
    ICON_EXTENSIONS
        .contains(&extension.as_str())
        .then(|| format!("{ICON_STEM}.{extension}"))
}

#[tracing::instrument(level = "debug", skip(progress, ctx))]
pub async fn collect_share_content(
    cluster_id: i64,
    scope: ShareScope,
    progress: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<ShareContent> {
    let tracked: HashMap<String, String> = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .filter_map(|row| Some((row.hash, row.bundle_name?)))
        .collect();
    let referenced = |bundle: &str| {
        if bundle == MODPACK_BUNDLE_NAME {
            scope.skip_modpack
        } else if super::is_imported_bundle(bundle) {
            false
        } else {
            scope.skip_bundles
        }
    };

    let mut content = ShareContent::default();
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    let mut rows = artifact_dao::list_cluster_artifacts_detailed(&ctx.db, cluster_id).await?;
    rows.sort_by_key(|row| row.enabled == 0);
    for row in rows {
        if tracked
            .get(&row.hash)
            .is_some_and(|bundle| referenced(bundle))
        {
            continue;
        }
        let Some(content_type) = ContentType::from_repr(row.content_type as u8)
            .filter(|content_type| shareable(*content_type))
        else {
            continue;
        };
        let file_name = row
            .cluster_file_name
            .trim_end_matches(".disabled")
            .to_string();
        let path = format!("{}/{}", content_type.folder_name(), file_name);
        if !is_safe_relative_path(&path) || !seen.insert(path.clone()) {
            continue;
        }

        let artifact = artifact_dao::get_artifact_by_hash(&ctx.db, &row.hash).await?;
        let local = match &artifact {
            Some(artifact) => local_file(artifact).await,
            None => None,
        };
        let known_url = match (&artifact, row.provider, &row.project_id, &row.version_id) {
            (Some(artifact), Some(provider), Some(project_id), Some(version_id))
                if provider == ProviderId::Modrinth as i64 =>
            {
                modrinth_cache_url(artifact, content_type, project_id, version_id)
            }
            _ => None,
        };
        candidates.push(Candidate {
            path,
            file_name,
            hash: polyio::normalize_hash(&row.hash),
            enabled: row.enabled != 0,
            local,
            known_url,
        });
    }

    content.scanned_bytes = candidates
        .iter()
        .map(|candidate| weight(candidate.local.as_ref().map_or(0, |(_, size)| *size)))
        .sum();
    let mut meter = ExportMeter::new(progress, 0, content.scanned_bytes * 2);

    let unknown: Vec<FileIdentity> = candidates
        .iter()
        .filter(|candidate| candidate.known_url.is_none())
        .map(|candidate| FileIdentity::from_sha1(&candidate.hash))
        .collect();
    let found = ctx
        .providers
        .lookup_versions(&unknown, ctx)
        .await
        .inspect_err(|err| {
            tracing::debug!(files = unknown.len(), error = %err, "could not look up downloads for exported files");
        })
        .unwrap_or_default();

    for candidate in candidates {
        meter.advance(candidate.size());

        let download = match &candidate.known_url {
            Some(url) => Some((url.clone(), candidate.size())),
            None => found
                .get(&candidate.hash)
                .and_then(|(_, version)| version_download(version, &candidate.hash)),
        };
        let (source, size) = match (download, candidate.local) {
            (Some((url, size)), local) => (
                SharedFileSource::Download {
                    url,
                    local: local.map(|(path, _)| path),
                },
                size,
            ),
            (None, Some((path, size))) => (SharedFileSource::Embed(path), size),
            (None, None) => {
                content.missing.push(candidate.file_name);
                continue;
            }
        };

        content.files.push(SharedFile {
            path: candidate.path,
            sha1: candidate.hash,
            size,
            enabled: candidate.enabled,
            source,
        });
    }

    content.overrides = bundle_dao::list_overrides(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .filter(|row| referenced(&row.bundle_name))
        .map(|row| SharedOverride {
            bundle: row.bundle_name,
            package_id: row.package_id,
            state: row.override_type,
        })
        .collect();

    if scope.skip_bundles {
        content.bundles = bundle_dao::list_bundle_choices(&ctx.db, cluster_id)
            .await?
            .into_iter()
            .map(|(name, accepted)| SharedBundleChoice { name, accepted })
            .collect();
    }

    content.files.sort_by(|a, b| a.path.cmp(&b.path));
    content.missing.sort();
    if !scope.skip_modpack {
        content.carryover = pack_carryover(cluster_id, ctx).await;
    }
    Ok(content)
}

async fn pack_carryover(cluster_id: i64, ctx: &ContentCtx) -> Option<PackCarryover> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await.ok()?;
    let hash = cluster.linked_modpack_hash.as_deref()?;
    let carried = async {
        let artifact = artifact_dao::get_artifact_by_hash(&ctx.db, hash)
            .await?
            .ok_or_else(|| ContentError::InvalidData {
                reason: "the modpack file for this instance is gone".to_string(),
            })?;
        let archive = artifact_absolute_path(&artifact.path)?;
        let manifest = super::read_modpack(&archive, ctx).await?;
        let prefixes: Vec<&str> = manifest
            .override_prefixes
            .iter()
            .map(String::as_str)
            .collect();
        let layers = OverrideLayers::open(&archive, &prefixes).await?;
        let files = layers
            .entries()
            .iter()
            .filter(|(rel, _)| tracked_content_type(rel).is_none())
            .cloned()
            .collect();
        let game_dir =
            oneclient_common::paths::cluster_game_dir(&cluster.folder_name, cluster.is_isolated())?;
        ContentResult::Ok(PackCarryover {
            archive,
            game_dir,
            files,
            loose: manifest.contents.loose,
        })
    }
    .await;

    carried
        .inspect_err(|err| {
            tracing::warn!(cluster_id, error = %err, "could not read the modpack's own files to carry them over");
        })
        .ok()
}

fn shareable(content_type: ContentType) -> bool {
    matches!(
        content_type,
        ContentType::Mod | ContentType::ResourcePack | ContentType::Shader
    )
}

async fn local_file(artifact: &ArtifactRow) -> Option<(PathBuf, u64)> {
    let path = artifact_absolute_path(&artifact.path).ok()?;
    let size = polyio::stat(&path).await.ok()?.len();
    Some((path, size))
}

fn modrinth_cache_url(
    artifact: &ArtifactRow,
    content_type: ContentType,
    project_id: &str,
    version_id: &str,
) -> Option<String> {
    let expected = cache_file_path(
        content_type,
        ProviderId::Modrinth,
        project_id,
        version_id,
        &artifact.file_name,
    )
    .and_then(|path| relative_cache_path(&path))
    .ok()?;
    (expected == artifact.path)
        .then(|| modrinth_cdn_url(project_id, version_id, &artifact.file_name))
}

fn modrinth_cdn_url(project_id: &str, version_id: &str, file_name: &str) -> String {
    format!(
        "{MODRINTH_CDN_PREFIX}{}/versions/{}/{}",
        encode_segment(project_id),
        encode_segment(version_id),
        encode_segment(file_name)
    )
}

fn encode_segment(segment: &str) -> String {
    segment.bytes().fold(String::new(), |mut out, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
        out
    })
}

async fn index_hashes(
    sha1: &str,
    local: Option<&Path>,
) -> Option<(BTreeMap<&'static str, String>, u64)> {
    let local = local?;
    let sha1 = polyio::normalize_hash(sha1);
    let (actual, sha512) = polyio::sha1_and_sha512_file(local).await.ok()?;
    if polyio::normalize_hash(&actual) != sha1 {
        return None;
    }
    let size = polyio::stat(local).await.ok()?.len();
    Some((BTreeMap::from([("sha1", sha1), ("sha512", sha512)]), size))
}

fn version_download(version: &VersionDetail, hash: &str) -> Option<(String, u64)> {
    version
        .files
        .iter()
        .find(|file| polyio::normalize_hash(&file.sha1) == hash)
        .filter(|file| trusted_download(&file.url))
        .map(|file| (file.url.clone(), file.size))
}

fn https_host(url: &str) -> Option<String> {
    let url = url::Url::parse(url).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    url.host_str().map(str::to_ascii_lowercase)
}

fn index_download(url: &str) -> bool {
    https_host(url).is_some_and(|host| INDEX_HOSTS.contains(&host.as_str()))
}

#[must_use]
pub fn trusted_download(url: &str) -> bool {
    https_host(url).is_some_and(|host| {
        INDEX_HOSTS.contains(&host.as_str()) || EXTRA_HOSTS.contains(&host.as_str())
    })
}

#[tracing::instrument(level = "debug", skip(share))]
pub async fn read_share_extras(path: &Path, share: &ClusterShare) -> ContentResult<ShareExtras> {
    let layers = OverrideLayers::open(path, &[OVERRIDES]).await?;
    let embedded = layers
        .entries()
        .iter()
        .filter_map(|(rel, _)| tracked_content_type(rel))
        .collect();
    let downloads = share
        .downloads
        .iter()
        .filter(|download| trusted_download(&download.url))
        .filter_map(|download| Some((tracked_content_type(&download.path)?, download.size)))
        .collect();
    let other_files = layers
        .entries()
        .iter()
        .filter(|(rel, _)| tracked_content_type(rel).is_none())
        .count();
    Ok(ShareExtras {
        embedded,
        downloads,
        other_files,
    })
}

#[tracing::instrument(level = "debug", skip(share, content, icon, progress), fields(name = %share.name))]
pub async fn write_share_archive(
    dest: &Path,
    share: &ClusterShare,
    content: &ShareContent,
    icon: Option<&ShareIcon>,
    progress: Option<&GroupedProgressChild>,
) -> ContentResult<ShareExportReport> {
    let mut meter = ExportMeter::new(progress, content.scanned_bytes, content.scanned_bytes * 2);
    let mut share = share.clone();
    share.format = SHARE_FORMAT;
    share.icon = icon.and_then(|icon| icon_entry(&icon.extension));
    share.disabled = content
        .files
        .iter()
        .filter(|file| !file.enabled)
        .map(|file| file.sha1.clone())
        .collect();
    share.overrides = content.overrides.clone();
    share.bundles = content.bundles.clone();
    share.downloads = Vec::new();

    let mut report = ShareExportReport {
        missing: content.missing.clone(),
        ..ShareExportReport::default()
    };
    let mut index_files = Vec::new();
    let mut embedded = Vec::new();
    let mut carried_entries = Vec::new();

    if let Some(carry) = &content.carryover {
        for loose in &carry.loose {
            let local = carry.game_dir.join(polyio::sanitize_path(&loose.path));
            let indexed = if index_download(&loose.url) {
                index_hashes(&loose.sha1, Some(&local)).await
            } else {
                None
            };
            if let Some((hashes, file_size)) = indexed {
                index_files.push(IndexFileOut {
                    path: loose.path.clone(),
                    hashes,
                    env: None,
                    downloads: vec![loose.url.clone()],
                    file_size,
                });
                report.carried += 1;
            } else if local.is_file() {
                embedded.push((format!("{OVERRIDES}{}", loose.path), local));
                report.carried += 1;
            } else {
                report.missing.push(loose.path.clone());
            }
        }
        carried_entries = carry.files.clone();
        report.carried += carried_entries.len();
    }

    for file in &content.files {
        match &file.source {
            SharedFileSource::Download { url, .. } if !index_download(url) => {
                share.downloads.push(SharedDownload {
                    path: file.path.clone(),
                    sha1: file.sha1.clone(),
                    size: file.size,
                    url: url.clone(),
                });
                report.linked += 1;
                meter.advance(file.size);
            }
            SharedFileSource::Download { url, local } => {
                match index_hashes(&file.sha1, local.as_deref()).await {
                    Some((hashes, file_size)) => index_files.push(IndexFileOut {
                        path: file.path.clone(),
                        hashes,
                        env: (!file.enabled).then(optional_env),
                        downloads: vec![url.clone()],
                        file_size,
                    }),
                    None => share.downloads.push(SharedDownload {
                        path: file.path.clone(),
                        sha1: file.sha1.clone(),
                        size: file.size,
                        url: url.clone(),
                    }),
                }
                report.linked += 1;
                meter.advance(file.size);
            }
            SharedFileSource::Embed(source) => {
                embedded.push((format!("{OVERRIDES}{}", file.path), source.clone()));
                report.embedded.push(file.path.clone());
            }
        }
    }

    let summary = share
        .description
        .as_deref()
        .and_then(|text| text.lines().map(str::trim).find(|line| !line.is_empty()));
    let index = IndexOut {
        format_version: 1,
        game: "minecraft",
        version_id: "1.0.0",
        name: &share.name,
        summary,
        files: index_files,
        dependencies: index_dependencies(&share),
    };

    let staging = polyio::temp_sibling(dest);
    let carried = content
        .carryover
        .as_ref()
        .map(|carry| (carry.archive.as_path(), carried_entries.as_slice()));
    let written = async {
        write_zip(
            &staging, &index, &share, icon, &embedded, carried, &mut meter,
        )
        .await?;
        polyio::rename(&staging, dest).await?;
        ContentResult::Ok(())
    }
    .await;
    if let Err(err) = written {
        polyio::remove_file(&staging).await.ok();
        return Err(err);
    }

    tracing::info!(
        linked = report.linked,
        embedded = report.embedded.len(),
        missing = report.missing.len(),
        "exported instance"
    );
    Ok(report)
}

fn optional_env() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([("client", "optional"), ("server", "optional")])
}

fn index_dependencies(share: &ClusterShare) -> BTreeMap<&'static str, String> {
    let mut dependencies = BTreeMap::from([("minecraft", share.mc_version.clone())]);
    let key = match share.mc_loader {
        GameLoader::Vanilla => None,
        GameLoader::Fabric | GameLoader::Ornithe => Some("fabric-loader"),
        GameLoader::Quilt => Some("quilt-loader"),
        GameLoader::Forge => Some("forge"),
        GameLoader::NeoForge => Some("neoforge"),
    };
    if let (Some(key), Some(version)) = (key, share.mc_loader_version.as_ref()) {
        dependencies.insert(key, version.clone());
    }
    dependencies
}

async fn write_zip(
    dest: &Path,
    index: &IndexOut<'_>,
    share: &ClusterShare,
    icon: Option<&ShareIcon>,
    embedded: &[(String, PathBuf)],
    carried: Option<(&Path, &[(String, String)])>,
    meter: &mut ExportMeter<'_>,
) -> ContentResult<()> {
    if let Some(parent) = dest.parent() {
        polyio::create_dir_all(parent).await?;
    }
    let file = tokio::fs::File::create(dest).await?;
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);

    let deflate = async_zip::Compression::Deflate;
    let stored = async_zip::Compression::Stored;
    write_entry(
        &mut writer,
        INDEX_ENTRY,
        &serde_json::to_vec_pretty(index)?,
        deflate,
    )
    .await?;
    write_entry(
        &mut writer,
        SHARE_ENTRY,
        &serde_json::to_vec_pretty(share)?,
        deflate,
    )
    .await?;
    if let (Some(icon), Some(entry)) = (icon, share.icon.as_deref()) {
        write_entry(&mut writer, entry, &icon.bytes, stored).await?;
    }
    for (entry, source) in embedded {
        stream_entry(&mut writer, entry, source, meter).await?;
    }
    if let Some((archive, files)) = carried
        && !files.is_empty()
    {
        let names: HashSet<&str> = files.iter().map(|(_, entry)| entry.as_str()).collect();
        let mut cursor = polyio::ZipEntryCursor::open(archive, |name| names.contains(name)).await?;
        for (rel, entry) in files {
            let bytes = cursor.read(entry).await?;
            write_entry(&mut writer, &format!("{OVERRIDES}{rel}"), &bytes, deflate).await?;
        }
    }

    writer.close().await.map_err(zip_error)?;
    Ok(())
}

async fn write_entry(
    writer: &mut async_zip::tokio::write::ZipFileWriter<tokio::fs::File>,
    name: &str,
    data: &[u8],
    compression: async_zip::Compression,
) -> ContentResult<()> {
    let builder = async_zip::ZipEntryBuilder::new(name.to_string().into(), compression);
    writer
        .write_entry_whole(builder, data)
        .await
        .map_err(zip_error)
}

async fn stream_entry(
    writer: &mut async_zip::tokio::write::ZipFileWriter<tokio::fs::File>,
    name: &str,
    source: &Path,
    meter: &mut ExportMeter<'_>,
) -> ContentResult<()> {
    use futures_lite::AsyncWriteExt;
    use tokio::io::AsyncReadExt;

    let builder =
        async_zip::ZipEntryBuilder::new(name.to_string().into(), async_zip::Compression::Stored);
    let mut entry = writer
        .write_entry_stream(builder)
        .await
        .map_err(zip_error)?;
    let mut file = tokio::fs::File::open(source).await?;
    let mut buffer = vec![0u8; COPY_CHUNK];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        entry.write_all(&buffer[..read]).await?;
        meter.advance(read as u64);
    }
    entry.close().await.map_err(zip_error)?;
    Ok(())
}

fn zip_error(err: async_zip::error::ZipError) -> ContentError {
    ContentError::Io(polyio::IOError::from(err))
}

#[tracing::instrument(skip(manifest, share, progress, ctx), fields(name = %share.name))]
pub async fn install_shared_content(
    archive_path: &Path,
    manifest: &ModpackManifest,
    share: &ClusterShare,
    cluster_id: i64,
    progress: Option<&GroupedProgressSession>,
    ctx: &ContentCtx,
) -> ContentResult<ModpackInstallReport> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let linked_before: HashSet<String> = PackageStore::list_linked_artifacts(cluster_id, ctx)
        .await?
        .into_iter()
        .map(|artifact| artifact.hash)
        .collect();

    let mut report = ModpackInstallReport::default();
    let mut downloads = Vec::new();
    for file in &manifest.contents.files {
        let external = match &file.kind {
            BundleFileKind::Managed { sha1, .. } => manifest.contents.direct.get(sha1).cloned(),
            BundleFileKind::External { file, .. } => Some(file.clone()),
        };
        match external {
            Some(external) => downloads.push(external),
            None => report.failed.push(file.display_name()),
        }
    }
    for download in &share.downloads {
        match download.external() {
            Some(external) => downloads.push(external),
            None => report.failed.push(download.path.clone()),
        }
    }
    downloads.retain(|file| {
        let trusted = trusted_download(&file.url);
        if !trusted {
            tracing::warn!(file = %file.name, url = %file.url, "refusing a shared download from an unknown host");
            report.failed.push(file.name.clone());
        }
        trusted
    });

    let cluster_ref = &cluster;
    let results = futures_util::stream::iter(downloads.into_iter().map(|file| async move {
        let result = download_and_link(&file, cluster_ref, progress, ctx).await;
        (file, result)
    }))
    .buffer_unordered(DOWNLOAD_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    let mut stored = Vec::new();
    for (file, result) in results {
        match result {
            Ok(row) => {
                stored.push((row, file.content_type));
                report.installed += 1;
            }
            Err(err) => {
                tracing::warn!(file = %file.name, error = %err, "failed to install a shared file");
                report.failed.push(file.name);
            }
        }
    }
    PackageStore::describe_artifacts(&stored, ctx).await;

    import_embedded(archive_path, cluster_id, &mut report, ctx).await?;
    if cluster.is_isolated() {
        apply_shared_files(archive_path, manifest, &cluster, progress, &mut report, ctx).await?;
    }
    report
        .failed
        .extend(manifest.contents.unresolved.iter().cloned());

    for hash in &share.disabled {
        let hash = polyio::normalize_hash(hash);
        if linked_before.contains(&hash) || !keeps_disabled_locally(&cluster, &hash, ctx).await? {
            continue;
        }
        if let Err(err) = PackageStore::set_artifact_enabled_to(cluster_id, &hash, false, ctx).await
        {
            tracing::warn!(hash, error = %err, "could not disable a shared file");
        }
    }

    tracing::info!(
        cluster_id,
        installed = report.installed,
        failed = report.failed.len(),
        "shared instance content installed"
    );
    Ok(report)
}

async fn apply_shared_files(
    archive_path: &Path,
    manifest: &ModpackManifest,
    cluster: &ClusterRow,
    progress: Option<&GroupedProgressSession>,
    report: &mut ModpackInstallReport,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let root = oneclient_common::paths::cluster_game_dir(&cluster.folder_name, true)?;
    sync_layered_overrides(
        archive_path,
        SHARED_OVERRIDES_KEY,
        &root,
        &[OVERRIDES],
        &|rel| tracked_content_type(rel).is_none(),
        true,
        Some(&ctx.events),
    )
    .await?;

    for file in &manifest.contents.loose {
        if !trusted_download(&file.url) {
            tracing::warn!(path = %file.path, url = %file.url, "refusing a shared file from an unknown host");
            report.failed.push(file.path.clone());
            continue;
        }
        if let Err(err) = install_loose_file(file, &root, progress, ctx).await {
            tracing::warn!(path = %file.path, error = %err, "failed to install a shared file");
            report.failed.push(file.path.clone());
        }
    }
    Ok(())
}

async fn download_and_link(
    file: &ExternalFile,
    cluster: &ClusterRow,
    progress: Option<&GroupedProgressSession>,
    ctx: &ContentCtx,
) -> ContentResult<ArtifactRow> {
    let child = progress.map(|p| {
        let child = p.child(file.name.clone(), file.size.max(1), TaskCategory::Packages);
        child.set_phase(TaskPhase::Downloading);
        child
    });
    let result = async {
        let row = store::download_external(file, false, child.as_ref(), ctx).await?;
        PackageStore::link_artifact(&row, cluster, Some(&file.name), ctx).await?;
        Ok(row)
    }
    .await;
    if let Some(child) = child {
        child.finish();
    }
    result
}

async fn keeps_disabled_locally(
    cluster: &ClusterRow,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<bool> {
    if !artifact_dao::is_cluster_linked(&ctx.db, cluster.id, hash).await? {
        return Ok(false);
    }
    if cluster.is_isolated() {
        return Ok(true);
    }
    let global = artifact_dao::get_artifact_by_hash(&ctx.db, hash)
        .await?
        .and_then(|artifact| ContentType::from_repr(artifact.content_type as u8))
        .is_some_and(ContentType::is_global);
    if !global {
        return Ok(true);
    }
    let sharing = artifact_dao::list_clusters_linking(&ctx.db, hash).await?;
    Ok(sharing.iter().all(|id| *id == cluster.id))
}

async fn import_embedded(
    archive_path: &Path,
    cluster_id: i64,
    report: &mut ModpackInstallReport,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let mut layers = OverrideLayers::open(archive_path, &[OVERRIDES]).await?;
    let bundled: Vec<(String, String, ContentType)> = layers
        .entries()
        .iter()
        .filter_map(|(rel, entry)| {
            let content_type = tracked_content_type(rel)?;
            Some((rel.clone(), entry.clone(), content_type))
        })
        .collect();
    if bundled.is_empty() {
        return Ok(());
    }

    let staging = polyio::tempdir().await?;
    let mut files = Vec::new();
    for (rel, entry, content_type) in bundled {
        let Some(bytes) = layers.read(&entry).await else {
            report.failed.push(rel);
            continue;
        };
        let Some(file_name) = rel.rsplit('/').next() else {
            continue;
        };
        let path = staging
            .dir_path()
            .join(content_type.folder_name())
            .join(file_name);
        if let Some(parent) = path.parent() {
            polyio::create_dir_all(parent).await?;
        }
        polyio::write(&path, bytes).await?;
        files.push((path, content_type));
    }

    let imported = PackageStore::import_local_files(&files, cluster_id, ctx).await?;
    report.installed += imported.imported.len();
    for (path, err) in imported.failed {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        tracing::warn!(file = %name, error = %err, "failed to import a file embedded in the shared instance");
        report.failed.push(name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share() -> ClusterShare {
        ClusterShare {
            format: SHARE_FORMAT,
            name: "My Pack".into(),
            description: Some("\n  First line\nSecond".into()),
            tags: vec!["pvp".into()],
            kind: ClusterKind::Modded,
            mc_version: "1.21.1".into(),
            mc_loader: GameLoader::Fabric,
            mc_loader_version: Some("0.16.5".into()),
            dedicated_dir: true,
            java: Some(SharedJava {
                major: 21,
                vendor: Some("Temurin".into()),
            }),
            modpack: None,
            bundles: Vec::new(),
            overrides: Vec::new(),
            disabled: Vec::new(),
            downloads: Vec::new(),
            icon: None,
        }
    }

    #[test]
    fn the_share_file_round_trips_through_json() {
        let original = share();
        let bytes = serde_json::to_vec(&original).unwrap();
        let parsed: ClusterShare = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn a_minimal_share_file_fills_in_defaults() {
        let parsed: ClusterShare = serde_json::from_value(serde_json::json!({
            "format": 1,
            "name": "Bare",
            "kind": "vanilla",
            "mc_version": "1.21.1",
            "mc_loader": "minecraft"
        }))
        .unwrap();
        assert_eq!(parsed.mc_loader, GameLoader::Vanilla);
        assert!(!parsed.dedicated_dir);
        assert!(parsed.disabled.is_empty());
        assert!(parsed.modpack.is_none());
    }

    #[test]
    fn only_https_links_to_known_hosts_are_trusted() {
        assert!(trusted_download(
            "https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar"
        ));
        assert!(trusted_download(
            "https://edge.forgecdn.net/files/1/2/a.jar"
        ));
        assert!(!trusted_download(
            "http://cdn.modrinth.com/data/a/versions/b/c.jar"
        ));
        assert!(!trusted_download(
            "https://cdn.modrinth.com.evil.example/c.jar"
        ));
        assert!(!trusted_download("https://example.com/c.jar"));
        assert!(!trusted_download("file:///C:/Windows/evil.jar"));
        assert!(index_download(
            "https://github.com/a/b/releases/download/1/c.jar"
        ));
        assert!(!index_download("https://edge.forgecdn.net/files/1/2/a.jar"));
    }

    #[tokio::test]
    async fn links_modrinth_does_not_allow_go_in_the_oneclient_file() {
        let scratch = polyio::testing::ScratchDir::new("share-index-hosts");
        let dest = scratch.path().join("hosts.mrpack");
        let file = |path: &str, url: &str| SharedFile {
            path: path.into(),
            sha1: format!("{:0>40}", path.len()),
            size: 5,
            enabled: true,
            source: SharedFileSource::Download {
                url: url.into(),
                local: None,
            },
        };
        let content = ShareContent {
            files: vec![
                file(
                    "mods/sodium.jar",
                    "https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar",
                ),
                file(
                    "mods/jei.jar",
                    "https://edge.forgecdn.net/files/1/2/jei.jar",
                ),
            ],
            ..ShareContent::default()
        };

        let report = write_share_archive(&dest, &share(), &content, None, None)
            .await
            .unwrap();
        assert_eq!(report.linked, 2);

        let entries =
            polyio::read_zip_file_entries(&dest, |name| name == INDEX_ENTRY || name == SHARE_ENTRY)
                .await
                .unwrap();
        let read = |name: &str| -> serde_json::Value {
            let (_, bytes) = entries.iter().find(|(entry, _)| entry == name).unwrap();
            serde_json::from_slice(bytes).unwrap()
        };
        let index_paths: Vec<String> = read(INDEX_ENTRY)["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["path"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(index_paths, vec!["mods/sodium.jar".to_string()]);
        let shared: ClusterShare = serde_json::from_value(read(SHARE_ENTRY)).unwrap();
        assert_eq!(shared.downloads.len(), 1);
        assert_eq!(shared.downloads[0].path, "mods/jei.jar");
    }

    #[test]
    fn only_known_icon_entries_are_read() {
        assert_eq!(icon_extension("icon.png").as_deref(), Some("png"));
        assert_eq!(icon_extension("icon.JPG").as_deref(), Some("jpg"));
        assert_eq!(icon_extension("overrides/icon.png"), None);
        assert_eq!(icon_extension("icon.exe"), None);
        assert_eq!(icon_extension("cover.png"), None);
        assert_eq!(icon_entry("WEBP").as_deref(), Some("icon.webp"));
        assert_eq!(icon_entry("bmp"), None);
    }

    #[test]
    fn dependencies_name_the_loader_the_way_modrinth_does() {
        let deps = index_dependencies(&share());
        assert_eq!(deps.get("minecraft").map(String::as_str), Some("1.21.1"));
        assert_eq!(
            deps.get("fabric-loader").map(String::as_str),
            Some("0.16.5")
        );

        let mut vanilla = share();
        vanilla.mc_loader = GameLoader::Vanilla;
        assert_eq!(index_dependencies(&vanilla).len(), 1);

        let mut ornithe = share();
        ornithe.mc_version = "1.8.9".into();
        ornithe.mc_loader = GameLoader::Ornithe;
        let deps = index_dependencies(&ornithe);
        assert_eq!(
            deps.get("fabric-loader").map(String::as_str),
            Some("0.16.5")
        );
        assert!(!deps.contains_key("ornithe"));
    }

    #[test]
    fn modrinth_links_are_spelled_the_way_the_cdn_spells_them() {
        assert_eq!(
            modrinth_cdn_url(
                "5ZwdcRci",
                "xOA9hbpf",
                "ImmediatelyFast-Fabric-1.15.3+26.1.jar"
            ),
            "https://cdn.modrinth.com/data/5ZwdcRci/versions/xOA9hbpf/ImmediatelyFast-Fabric-1.15.3%2B26.1.jar"
        );
        assert_eq!(encode_segment("a b[1]"), "a%20b%5B1%5D");
        assert_eq!(encode_segment("żółw.jar"), "%C5%BC%C3%B3%C5%82w.jar");
    }

    #[test]
    fn exported_index_parses_back_as_a_modpack() {
        let index = IndexOut {
            format_version: 1,
            game: "minecraft",
            version_id: "1.0.0",
            name: "My Pack",
            summary: Some("First line"),
            files: vec![IndexFileOut {
                path: "mods/sodium.jar".into(),
                hashes: BTreeMap::from([("sha1", "aaaa".to_string())]),
                env: Some(optional_env()),
                downloads: vec![
                    "https://cdn.modrinth.com/data/AANobbMI/versions/tFw0iWAk/sodium.jar".into(),
                ],
                file_size: 10,
            }],
            dependencies: index_dependencies(&share()),
        };
        let bytes = serde_json::to_vec(&index).unwrap();
        let manifest = super::super::mrpack::parse(&bytes).unwrap();

        assert_eq!(manifest.name, "My Pack");
        assert_eq!(manifest.loader, GameLoader::Fabric);
        assert_eq!(manifest.contents.files.len(), 1);
        assert!(manifest.contents.direct.contains_key("aaaa"));
        assert!(manifest.contents.optional.contains("AANobbMI"));
    }
}
