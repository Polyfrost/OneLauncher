use std::collections::HashMap;
use std::sync::Arc;

use oneclient_core::notification::LaunchStage;
use oneclient_core::settings::LauncherSettings;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameSnapshot {
    pub stages: HashMap<i64, LaunchStage>,
    pub error: Option<String>,
    pub logs: HashMap<i64, Arc<Vec<Arc<str>>>>,
}

impl GameSnapshot {
    pub fn stage(&self, cluster_id: i64) -> Option<LaunchStage> {
        self.stages.get(&cluster_id).copied()
    }

    pub fn is_busy(&self, cluster_id: i64) -> bool {
        self.stage(cluster_id).is_some_and(LaunchStage::is_busy)
    }

    pub fn is_running(&self, cluster_id: i64) -> bool {
        self.stage(cluster_id) == Some(LaunchStage::Running)
    }

    pub fn is_active(&self, cluster_id: i64) -> bool {
        matches!(
            self.stage(cluster_id),
            Some(s) if s != LaunchStage::Exited
        )
    }

    pub fn logs_for(&self, cluster_id: i64) -> Arc<Vec<Arc<str>>> {
        self.logs
            .get(&cluster_id)
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncStatus {
    Idle,
    Loading,
    Ready,
    Error,
}

#[derive(Clone, Debug, Default)]
pub struct LauncherInit {
    pub ready: bool,
    pub fetching: bool,
    pub error: Option<String>,
    pub data_dir: String,
}

#[derive(Clone, Debug)]
pub struct SettingsSnapshot {
    pub settings: LauncherSettings,
    pub status: AsyncStatus,
    pub saving: bool,
    pub error: Option<String>,
}

impl Default for SettingsSnapshot {
    fn default() -> Self {
        Self {
            settings: LauncherSettings::default(),
            status: AsyncStatus::Idle,
            saving: false,
            error: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProfilesSnapshot {
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ClustersSnapshot {
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct JavaSnapshot {
    pub generation: u64,
}

#[derive(Clone, Debug, Default)]
pub struct BridgeSnapshot {
    pub launcher: LauncherInit,
    pub settings: SettingsSnapshot,
    pub profiles: ProfilesSnapshot,
    pub clusters: ClustersSnapshot,
    pub java: JavaSnapshot,
    pub notifications: crate::notifications::NotificationSnapshot,
    pub game: GameSnapshot,
}
