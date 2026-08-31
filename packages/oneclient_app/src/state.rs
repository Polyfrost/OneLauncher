//! [`AppChannel`] gives per-concern subscription a single `watch` channel woke every
//! consumer and deep-cloned the inbox on every event

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use freya::radio::RadioChannel;
use oneclient_common::domain::ProviderId;
use oneclient_core::settings::LauncherSettings;
use oneclient_events::LaunchStage;

use crate::chat::ChatState;
use crate::notifications::{InboxEntry, NotificationState, PendingPrompt};

/// Writing through a channel wakes only the components that subscribed to it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AppChannel {
    Launcher,
    Settings,
    Notifications,
    Game,
    AccountSwitcher,
    MicrosoftLogin,
    Installs,
    Chat,
    ChatRoster,
    ChatPresence,
}

impl RadioChannel<AppState> for AppChannel {}

#[derive(Default)]
pub struct AppState {
    pub launcher: LauncherInit,
    pub settings: SettingsState,
    /// The engine itself not a snapshot both the pump and UI actions fold into it
    pub notifications: NotificationState,
    /// Held beside the engine which folds events into it by `&mut` so the engine
    /// stays a plain state machine with no channels of its own
    pub inbox: Vec<InboxEntry>,
    pub prompt: Option<PendingPrompt>,
    pub center_open: bool,
    pub game: GameState,
    pub account_switcher_open: bool,
    pub microsoft_login: Option<LoginProgress>,
    pub installs: InstallState,
    pub chat: ChatState,
}

/// In-flight installs so the button that started one stays disabled until it lands
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstallState {
    pending: HashSet<(i64, ProviderId, String)>,
}

impl InstallState {
    pub fn begin(&mut self, cluster_id: i64, provider: ProviderId, project_id: String) {
        self.pending.insert((cluster_id, provider, project_id));
    }

    pub fn finish(&mut self, cluster_id: i64, provider: ProviderId, project_id: &str) {
        self.pending
            .remove(&(cluster_id, provider, project_id.to_string()));
    }

    #[must_use]
    pub fn is_installing(&self, cluster_id: i64, provider: ProviderId, project_id: &str) -> bool {
        self.pending
            .contains(&(cluster_id, provider, project_id.to_string()))
    }
}

#[derive(Clone, Debug, Default)]
pub struct LauncherInit {
    pub ready: bool,
    pub fetching: bool,
    /// Launch is disabled until this clears
    pub syncing_bundles: bool,
    pub error: Option<String>,
    pub data_dir: String,
    /// Restorable database snapshots, for the startup failure screen
    pub snapshots: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AsyncStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    Error,
}

#[derive(Clone, Debug, Default)]
pub struct SettingsState {
    pub settings: LauncherSettings,
    pub status: AsyncStatus,
    pub saving: bool,
    pub error: Option<String>,
}

/// Rendered inside the sign-in modal rather than as a toast the core reports it
/// as ordinary progress and has no opinion about where it is shown
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginProgress {
    pub label: String,
    pub current: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameState {
    pub stages: HashMap<i64, LaunchStage>,
    pub error: Option<String>,
    pub logs: HashMap<i64, Arc<Vec<Arc<str>>>>,
    /// Launches started from the UI but not yet answered by core which takes a few
    /// hundred ms every click in that window otherwise spawns its own game
    pending: HashSet<i64>,
}

impl GameState {
    #[must_use]
    pub fn stage(&self, cluster_id: i64) -> Option<LaunchStage> {
        self.stages.get(&cluster_id).copied()
    }

    /// Returns false if a launch is already in flight the re-entrancy guard
    pub fn begin_launch(&mut self, cluster_id: i64) -> bool {
        if self.is_active(cluster_id) || self.is_launch_pending(cluster_id) {
            return false;
        }
        self.pending.insert(cluster_id);
        true
    }

    pub fn finish_launch(&mut self, cluster_id: i64) {
        self.pending.remove(&cluster_id);
    }

    #[must_use]
    pub fn is_launch_pending(&self, cluster_id: i64) -> bool {
        self.pending.contains(&cluster_id)
    }

    #[must_use]
    pub fn is_busy(&self, cluster_id: i64) -> bool {
        self.stage(cluster_id).is_some_and(LaunchStage::is_busy)
    }

    #[must_use]
    pub fn is_running(&self, cluster_id: i64) -> bool {
        self.stage(cluster_id) == Some(LaunchStage::Running)
    }

    #[must_use]
    pub fn is_active(&self, cluster_id: i64) -> bool {
        matches!(self.stage(cluster_id), Some(s) if s != LaunchStage::Exited)
    }

    #[must_use]
    pub fn logs_for(&self, cluster_id: i64) -> Arc<Vec<Arc<str>>> {
        self.logs.get(&cluster_id).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::app::launch_button_state;

    #[test]
    fn a_second_click_is_refused_before_core_answers() {
        let mut game = GameState::default();

        assert!(game.begin_launch(1));
        assert!(!game.begin_launch(1));
        assert!(game.begin_launch(2));
    }

    #[test]
    fn a_running_cluster_cannot_be_launched_again() {
        let mut game = GameState::default();
        game.stages.insert(1, LaunchStage::Running);

        assert!(!game.begin_launch(1));
    }

    #[test]
    fn the_claim_is_released_when_the_launch_settles() {
        let mut game = GameState::default();

        assert!(game.begin_launch(1));
        game.finish_launch(1);
        assert!(game.begin_launch(1));
    }

    #[test]
    fn the_button_disables_on_the_claim_alone() {
        let mut game = GameState::default();
        assert_eq!(launch_button_state(&game, 1, false), ("Launch", true));

        game.begin_launch(1);
        assert_eq!(launch_button_state(&game, 1, false), ("Launching", false));

        // Held past a failure which parks the stage at `Exited`
        game.stages.insert(1, LaunchStage::Exited);
        assert_eq!(launch_button_state(&game, 1, false), ("Launching", false));

        game.finish_launch(1);
        assert_eq!(launch_button_state(&game, 1, false), ("Launch", true));
    }
}
