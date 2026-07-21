use serde::{Deserialize, Serialize};

use crate::bundles::Bundle;
use crate::packages::domain::{ContentType, GameLoader, ProviderId};
use crate::packages::types::ExternalFile;

#[derive(Debug, Clone)]
pub struct BundleArchive {
    pub bundle: Bundle,
    pub manifest: BundleManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    pub name: String,
    pub version_id: String,
    pub category: String,
    pub mc_version: String,
    pub loader: GameLoader,
    pub loader_version: String,
    pub enabled: bool,
    pub files: Vec<BundleFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleFile {
    pub enabled: bool,
    pub hidden: bool,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub size: u64,
    pub kind: BundleFileKind,
}

impl BundleFile {
    pub fn content_type(&self) -> ContentType {
        if let BundleFileKind::External(ext) = &self.kind {
            return ext.content_type;
        }
        content_type_from_bundle_path(&self.path)
    }

    pub fn display_name(&self) -> String {
        let from_path = self.path.rsplit('/').next().filter(|s| !s.is_empty());
        if let Some(name) = from_path {
            return name.to_string();
        }
        match &self.kind {
            BundleFileKind::External(ext) => ext.name.clone(),
            BundleFileKind::Managed { project_id, .. } => project_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BundleFileKind {
    Managed {
        provider: ProviderId,
        project_id: String,
        version_id: String,
        sha1: String,
    },
    External(ExternalFile),
}

impl BundleFileKind {
    pub fn package_id(&self) -> String {
        match self {
            Self::Managed { project_id, .. } => project_id.clone(),
            Self::External(ext) => ext.sha1.clone(),
        }
    }

    pub fn bundle_version_id(&self) -> String {
        match self {
            Self::Managed { version_id, .. } => version_id.clone(),
            Self::External(ext) => ext.sha1.clone(),
        }
    }
}

pub fn managed_bundle_key(provider: ProviderId, package_id: &str) -> String {
    format!("m:{}:{package_id}", provider.dir_name())
}

pub fn external_bundle_key(sha1: &str) -> String {
    format!("e:{sha1}")
}

pub fn content_type_from_bundle_path(path: &str) -> ContentType {
    let top = path.split('/').next().unwrap_or("");
    ContentType::from_folder_name(top).unwrap_or(ContentType::Mod)
}

#[derive(Debug, Clone)]
pub struct BundlePackageUpdate {
    pub cluster_id: i64,
    pub installed_hash: String,
    pub installed_version_id: String,
    pub bundle_name: String,
    pub new_version_id: String,
    pub new_file: BundleFile,
}

#[derive(Debug, Clone)]
pub struct BundlePackageRemoval {
    pub cluster_id: i64,
    pub hash: String,
    pub package_id: String,
    pub bundle_name: String,
    /// Provider of the installed artifact being removed, resolved from the
    /// linked-artifact row at check time. `None` for external/local files.
    pub provider: Option<ProviderId>,
    /// Remote project id of the installed artifact, for meta-cache lookups.
    pub project_id: Option<String>,
    /// Best-effort human name (display name / file name) captured at check
    /// time, used as fallback when meta cache has no entry.
    pub display_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BundlePackageAddition {
    pub cluster_id: i64,
    pub bundle_name: String,
    pub new_file: BundleFile,
}

#[derive(Debug, Clone, Default)]
pub struct BundleUpdateCheckResult {
    pub cluster_id: i64,
    pub updates_available: Vec<BundlePackageUpdate>,
    pub removals_available: Vec<BundlePackageRemoval>,
    pub additions_available: Vec<BundlePackageAddition>,
}

#[derive(Debug, Clone, Default)]
pub struct ApplyBundleUpdatesResult {
    pub updates_applied: Vec<BundlePackageUpdate>,
    pub removals_applied: Vec<BundlePackageRemoval>,
    pub additions_applied: Vec<BundlePackageAddition>,
    pub updates_failed: Vec<String>,
    pub removals_failed: Vec<String>,
    pub additions_failed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileUpdateStatus {
    NotInstalled,
    RemovedByUser,
    UpToDate,
    UpdateAvailable {
        installed_version_id: String,
        new_version_id: String,
    },
}

#[derive(Debug, Clone)]
pub struct BundleWithUpdateStatus {
    pub archive: BundleArchive,
    pub files: Vec<(BundleFile, FileUpdateStatus)>,
    pub has_updates: bool,
}
