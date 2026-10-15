mod checker;
mod curseforge;
mod install;
mod mrpack;
mod update;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::bundles::BundleFile;
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::PackageError;
use crate::packages::types::ExternalFile;
use oneclient_common::domain::{ContentType, GameLoader};

pub use update::{
    ModpackRelease, ModpackUpdateStatus, check_modpack_update, cluster_modpack, identify_modpack,
};

pub use install::{
    ModpackInstallReport, find_blocked_downloads, import_blocked_files, install_modpack,
    store_modpack_archive,
};

pub const MODPACK_BUNDLE_NAME: &str = "modpack";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModpackFormat {
    Modrinth,
    CurseForge,
}

#[derive(Debug, Clone)]
pub struct ModpackManifest {
    pub name: String,
    pub version: String,
    pub summary: Option<String>,
    pub mc_version: String,
    pub loader: GameLoader,
    pub loader_version: Option<String>,
    pub contents: ModpackContents,
    pub override_prefixes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ModpackContents {
    pub files: Vec<BundleFile>,
    pub direct: HashMap<String, ExternalFile>,
    pub optional: HashSet<String>,
    pub loose: Vec<LooseFile>,
    pub blocked: Vec<BlockedFile>,
    pub unresolved: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModpackSummary {
    pub mods: usize,
    pub resource_packs: usize,
    pub shaders: usize,
    pub other_files: usize,
    pub optional: usize,
    pub manual: usize,
    pub unresolved: usize,
    pub download_bytes: u64,
}

impl ModpackManifest {
    #[must_use]
    pub fn summary(&self) -> ModpackSummary {
        let contents = &self.contents;
        let mut summary = ModpackSummary {
            other_files: contents.loose.len(),
            optional: contents.optional.len(),
            manual: contents.blocked.len(),
            unresolved: contents.unresolved.len(),
            ..ModpackSummary::default()
        };

        for file in &contents.files {
            match file.content_type() {
                ContentType::ResourcePack => summary.resource_packs += 1,
                ContentType::Shader => summary.shaders += 1,
                _ => summary.mods += 1,
            }
            summary.download_bytes += file.size;
        }
        summary.download_bytes += contents.loose.iter().map(|file| file.size).sum::<u64>();
        summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LooseFile {
    pub path: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedFile {
    pub project_id: String,
    pub version_id: String,
    pub project_name: String,
    pub file_name: String,
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub content_type: ContentType,
    pub page_url: Option<String>,
}

#[tracing::instrument(level = "debug")]
pub async fn detect_format(path: &Path) -> ContentResult<ModpackFormat> {
    if zip_entry(path, mrpack::INDEX_ENTRY).await?.is_some() {
        return Ok(ModpackFormat::Modrinth);
    }
    match zip_entry(path, curseforge::MANIFEST_ENTRY).await? {
        Some(bytes) if curseforge::is_modpack_manifest(&bytes) => Ok(ModpackFormat::CurseForge),
        _ => Err(PackageError::UnsupportedModpackFormat.into()),
    }
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn read_modpack(path: &Path, ctx: &ContentCtx) -> ContentResult<ModpackManifest> {
    if let Some(bytes) = zip_entry(path, mrpack::INDEX_ENTRY).await? {
        let mut manifest = mrpack::parse(&bytes)?;
        checker::add_checker_mods(path, &mut manifest, ctx).await;
        return Ok(manifest);
    }

    let bytes = zip_entry(path, curseforge::MANIFEST_ENTRY)
        .await?
        .ok_or(PackageError::UnsupportedModpackFormat)?;
    curseforge::resolve(&bytes, ctx).await
}

async fn zip_entry(path: &Path, entry: &str) -> ContentResult<Option<Vec<u8>>> {
    let file = tokio::fs::File::open(path).await?;
    match polyio::try_read_zip_entry_bytes(tokio::io::BufReader::new(file), entry).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(polyio::IOError::FileNotFoundInZip { .. }) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn file_sha1(file: &BundleFile) -> &str {
    match &file.kind {
        crate::bundles::BundleFileKind::Managed { sha1, .. } => sha1,
        crate::bundles::BundleFileKind::External { file, .. } => &file.sha1,
    }
}

fn file_name_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_lowercase()
}

fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') || path.contains(':') {
        return false;
    }
    path.split(['/', '\\'])
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn tracked_folder(folder: &str) -> Option<ContentType> {
    match ContentType::from_folder_name(folder)? {
        content_type @ (ContentType::Mod | ContentType::ResourcePack | ContentType::Shader) => {
            Some(content_type)
        }
        _ => None,
    }
}

fn tracked_content_type(path: &str) -> Option<ContentType> {
    let (folder, file) = path.split_once('/')?;
    if file.is_empty() || file.contains('/') {
        return None;
    }
    tracked_folder(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_and_absolute_paths_are_rejected() {
        assert!(is_safe_relative_path("mods/sodium.jar"));
        assert!(is_safe_relative_path("config/sodium/options.json"));
        assert!(!is_safe_relative_path("../mods/evil.jar"));
        assert!(!is_safe_relative_path("mods/../../evil.jar"));
        assert!(!is_safe_relative_path("/etc/passwd"));
        assert!(!is_safe_relative_path("\\Windows\\evil.dll"));
        assert!(!is_safe_relative_path("C:/Windows/evil.dll"));
        assert!(!is_safe_relative_path("mods//evil.jar"));
        assert!(!is_safe_relative_path(""));
    }

    #[test]
    fn only_top_level_content_files_are_tracked() {
        assert_eq!(tracked_content_type("mods/a.jar"), Some(ContentType::Mod));
        assert_eq!(
            tracked_content_type("resourcepacks/a.zip"),
            Some(ContentType::ResourcePack)
        );
        assert_eq!(
            tracked_content_type("shaderpacks/a.zip"),
            Some(ContentType::Shader)
        );
        assert_eq!(tracked_content_type("mods/sub/a.jar"), None);
        assert_eq!(tracked_content_type("config/a.json"), None);
        assert_eq!(tracked_content_type("datapacks/a.zip"), None);
        assert_eq!(tracked_content_type("a.jar"), None);
    }
}
