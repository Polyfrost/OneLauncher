mod fs;
pub mod oneclient_v1;
pub mod vanilla;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::LauncherResult;
use crate::packages::domain::GameLoader;

pub use fs::copy_tree;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MigrationSource {
    OneClientV1,
    Vanilla,
}

impl MigrationSource {
    pub const ALL: &'static [MigrationSource] =
        &[MigrationSource::OneClientV1, MigrationSource::Vanilla];

    pub fn id(self) -> &'static str {
        match self {
            MigrationSource::OneClientV1 => "oneclient_v1",
            MigrationSource::Vanilla => "vanilla",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            MigrationSource::OneClientV1 => "OneClient",
            MigrationSource::Vanilla => "Minecraft",
        }
    }

    pub fn from_id(id: &str) -> Option<MigrationSource> {
        Self::ALL.iter().copied().find(|s| s.id() == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInstance {
    /// Source-local identifier (v1: the cluster row id). Purely informational.
    pub instance_id: i64,
    pub folder_name: String,
    pub mc_version: String,
	/// Used for any "migrated" clusters (e.g. 26.1 fabric -> 26.1.2 fabric)
    pub target_mc_version: Option<String>,
    pub mc_loader: GameLoader,
    /// Bundle categories the user had installed on this instance, e.g.
    /// `["HUD", "Performance"]`. Empty when the source has no category concept.
    pub categories: Vec<String>,
    pub has_game_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationDetection {
    pub source: MigrationSource,
    pub root: PathBuf,
    pub instances: Vec<SourceInstance>,
}

impl SourceInstance {
    #[must_use]
    pub fn import_version(&self) -> &str {
        self.target_mc_version.as_deref().unwrap_or(&self.mc_version)
    }
}

/// Where an imported game directory should land in *this* launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportTarget {
    /// Copy into the shared `<launcher_dir>/.minecraft`.
    Shared,
    /// Copy into a specific new cluster's own folder and mark it dedicated so
    /// `Cluster::game_dir()` resolves there instead of the shared dir.
    Dedicated { new_cluster_id: i64 },
}

#[tracing::instrument]
pub async fn detect() -> LauncherResult<Option<MigrationDetection>> {
    for source in MigrationSource::ALL.iter().copied() {
        let detection = match source {
            MigrationSource::OneClientV1 => oneclient_v1::detect().await?,
            MigrationSource::Vanilla => vanilla::detect().await?,
        };

        if let Some(detection) = detection
            && !detection.instances.is_empty()
        {
            return Ok(Some(detection));
        }
    }
    Ok(None)
}

#[tracing::instrument]
pub async fn import_game_dir(
    source: MigrationSource,
    folder_name: &str,
    target: ImportTarget,
) -> LauncherResult<()> {
    match source {
        MigrationSource::OneClientV1 => oneclient_v1::import_game_dir(folder_name, target).await,
        MigrationSource::Vanilla => vanilla::import_game_dir(target).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_is_the_last_resort() {
        assert_eq!(MigrationSource::ALL.last(), Some(&MigrationSource::Vanilla));
    }

    #[test]
    fn import_version_prefers_resolved_target() {
        let mut instance = SourceInstance {
            instance_id: 1,
            folder_name: "26.1 fabric".to_string(),
            mc_version: "26.1".to_string(),
            target_mc_version: None,
            mc_loader: GameLoader::Fabric,
            categories: Vec::new(),
            has_game_dir: true,
        };
        assert_eq!(instance.import_version(), "26.1");

        instance.target_mc_version = Some("26.1.2".to_string());
        assert_eq!(instance.import_version(), "26.1.2");
    }

    #[test]
    fn ids_round_trip() {
        for source in MigrationSource::ALL.iter().copied() {
            assert_eq!(MigrationSource::from_id(source.id()), Some(source));
        }
    }
}
