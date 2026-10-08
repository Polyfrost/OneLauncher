//! Imports instances from third-party launchers (Prism and its MultiMC
//! relatives, the Modrinth App)
//!
//! Unlike the onboarding migration these never map onto OneClient's curated
//! clusters: every imported instance becomes its own isolated cluster with its
//! content registered in the package store

mod import;
mod ini;
mod java;
pub mod modrinth_app;
pub mod prism;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::LauncherResult;
use oneclient_common::domain::{GameLoader, ProviderId};

pub use import::{ExternalImportReport, import_instance};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExternalLauncher {
    Prism,
    PolyMc,
    MultiMc,
    ModrinthApp,
}

impl ExternalLauncher {
    pub fn id(self) -> &'static str {
        match self {
            Self::Prism => "prism",
            Self::PolyMc => "polymc",
            Self::MultiMc => "multimc",
            Self::ModrinthApp => "modrinth_app",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Prism => "Prism Launcher",
            Self::PolyMc => "PolyMC",
            Self::MultiMc => "MultiMC",
            Self::ModrinthApp => "Modrinth App",
        }
    }
}

/// Only the overrides the source instance actually set; anything left `None`
/// falls back to OneClient's global settings
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalSettings {
    pub mem_max: Option<u32>,
    pub jvm_args: Option<String>,
    pub env: Vec<(String, String)>,
    pub resolution: Option<(u32, u32)>,
    pub fullscreen: Option<bool>,
    pub hook_pre: Option<String>,
    pub hook_wrapper: Option<String>,
    pub hook_post: Option<String>,
    /// The Java major the instance ran on, when the source records one;
    /// `jvm_args` were written for it
    pub java_major: Option<u32>,
}

impl ExternalSettings {
    /// Nothing to write into a settings profile; `java_major` describes the
    /// source rather than being a setting itself
    pub fn is_empty(&self) -> bool {
        *self
            == Self {
                java_major: self.java_major,
                ..Self::default()
            }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedPack {
    pub provider: ProviderId,
    pub project_id: String,
    pub version_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalInstance {
    pub launcher: ExternalLauncher,
    /// Source-local key (Prism folder name, Modrinth profile path)
    pub id: String,
    pub name: String,
    /// The `.minecraft` the game ran in
    pub game_dir: PathBuf,
    pub mc_version: String,
    pub loader: GameLoader,
    pub loader_version: Option<String>,
    pub icon: Option<PathBuf>,
    pub notes: Option<String>,
    pub groups: Vec<String>,
    pub played_secs: u64,
    pub settings: ExternalSettings,
    pub linked_pack: Option<LinkedPack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalDetection {
    pub launcher: ExternalLauncher,
    pub root: PathBuf,
    pub instances: Vec<ExternalInstance>,
}

/// Every install found on this machine, each launcher at most once per root
///
/// A launcher that cannot be read is logged and skipped so one corrupt
/// install never hides the others
#[tracing::instrument]
pub async fn detect_all() -> Vec<ExternalDetection> {
    let mut found = Vec::new();

    for (launcher, root) in prism::default_roots() {
        if found.iter().any(|d: &ExternalDetection| d.root == root) {
            continue;
        }
        match prism::detect_at(launcher, &root).await {
            Ok(Some(detection)) if !detection.instances.is_empty() => found.push(detection),
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(root = %root.display(), error = %err, "could not read {} install", launcher.display_name());
            }
        }
    }

    if let Some(root) = modrinth_app::default_root() {
        match modrinth_app::detect_at(&root).await {
            Ok(Some(detection)) if !detection.instances.is_empty() => found.push(detection),
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(root = %root.display(), error = %err, "could not read Modrinth App install");
            }
        }
    }

    found
}

/// For portable installs the user points at by hand
#[tracing::instrument]
pub async fn detect_folder(root: PathBuf) -> LauncherResult<Option<ExternalDetection>> {
    if modrinth_app::looks_like_root(&root) {
        return modrinth_app::detect_at(&root).await;
    }
    if let Some(launcher) = prism::looks_like_root(&root) {
        return prism::detect_at(launcher, &root).await;
    }
    Ok(None)
}

/// Whether two install roots are the same folder however they are spelled
/// (case, separators, a trailing slash, links), so one install picked by
/// hand is not listed a second time next to its detected self
pub fn same_folder(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(target_os = "linux")]
fn home_relative(path: &str) -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(path))
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_same_folder_matches_however_it_is_spelled() {
        let a = polyio::tempdir().await.unwrap();
        let b = polyio::tempdir().await.unwrap();
        let root = a.dir_path();

        assert!(same_folder(root, &root.join(".")));
        assert!(same_folder(root, &root.join("sub").join("..")));
        #[cfg(windows)]
        assert!(same_folder(
            root,
            Path::new(&root.to_string_lossy().to_uppercase())
        ));
        assert!(!same_folder(root, b.dir_path()));
    }
}
