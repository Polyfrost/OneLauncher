//! The app's own state, held in a freya-radio station.
//!
//! One struct republished over a `watch` channel woke every consumer on every
//! change (a toast timer tick re-rendering components that only read
//! `data_dir`) and deep-cloned the whole notification inbox on publish, tens of
//! thousands of times during a download.
//!
//! [`AppChannel`] gives per-concern subscription instead, and because the
//! station only requires `'static`, this owns [`NotificationState`] outright
//! rather than cloning a snapshot out of it on every event.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use freya::radio::RadioChannel;
use oneclient_common::domain::ProviderId;
use oneclient_core::settings::LauncherSettings;
use oneclient_events::LaunchStage;

use crate::notifications::{InboxEntry, NotificationState, PendingPrompt};

/// What a component can subscribe to. Writing through a channel wakes only the
/// components that asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AppChannel {
    /// Startup progress and the resolved data directory.
    Launcher,
    /// Launcher settings and their save state.
    Settings,
    /// Inbox, toasts and the pending prompt.
    Notifications,
    /// Per-cluster launch stage and live log lines.
    Game,
    /// Whether the account switcher popover is open.
    AccountSwitcher,
    /// Live progress of an in-flight Microsoft sign-in.
    MicrosoftLogin,
    /// Packages currently being installed into a cluster.
    Installs,
}

impl RadioChannel<AppState> for AppChannel {}

#[derive(Default)]
pub struct AppState {
    pub launcher: LauncherInit,
    pub settings: SettingsState,
    /// The notification engine itself, not a snapshot of it.
    ///
    /// Both the event pump and UI actions fold into this, so it has to live where
    /// both can reach it, and nothing has to clone the inbox per event.
    pub notifications: NotificationState,
    /// Held beside the engine because it folds events into the inbox by `&mut`
    /// reference; keeping them together lets the engine stay a plain
    /// state machine with no channels of its own.
    pub inbox: Vec<InboxEntry>,
    pub prompt: Option<PendingPrompt>,
    /// Whether the notification centre panel is open.
    pub center_open: bool,
    pub game: GameState,
    pub account_switcher_open: bool,
    pub microsoft_login: Option<LoginProgress>,
    pub installs: InstallState,
}

/// Package installs that are in flight, so the button that started one can stay
/// disabled until it lands — and come back if it failed.
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
    /// True while the background cluster-bundle download is running. Launch is
    /// disabled until it clears.
    pub syncing_bundles: bool,
    pub error: Option<String>,
    pub data_dir: String,
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

/// Live progress of an in-flight Microsoft sign-in, rendered inside the sign-in
/// modal rather than as a toast. UI state: the core reports this as ordinary
/// progress and has no opinion about where it is shown.
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
    /// Clusters whose launch was started from the UI but has not been answered
    /// by core yet.
    ///
    /// Core takes a few hundred ms to report [`LaunchStage::Checking`], and
    /// every click that lands in that window used to spawn its own game.
    pending: HashSet<i64>,
}

impl GameState {
    #[must_use]
    pub fn stage(&self, cluster_id: i64) -> Option<LaunchStage> {
        self.stages.get(&cluster_id).copied()
    }

    /// Claims the launch for this cluster, returning false if one is already in
    /// flight — the re-entrancy guard behind the button's disabled state.
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

    /// The cluster the launcher treats as "the game", for the things that can
    /// only mean one of them: the window's hide-while-playing behaviour and the
    /// tray's log entry.
    ///
    /// Parallel clusters are allowed, so the lowest id wins rather than
    /// whichever the map happened to yield first — an arbitrary answer that
    /// changed between calls would flip the window back and forth.
    #[must_use]
    pub fn running_cluster(&self) -> Option<i64> {
        self.running_cluster_where(|_| true)
    }

    /// Like [`Self::running_cluster`], but only among clusters `is_candidate`
    /// accepts.
    #[must_use]
    pub fn running_cluster_where(&self, is_candidate: impl Fn(i64) -> bool) -> Option<i64> {
        self.stages
            .iter()
            .filter(|(id, stage)| **stage == LaunchStage::Running && is_candidate(**id))
            .map(|(id, _)| *id)
            .min()
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
    fn the_live_cluster_is_the_lowest_running_id() {
        let mut game = GameState::default();
        assert_eq!(game.running_cluster(), None);

        game.stages.insert(3, LaunchStage::Running);
        game.stages.insert(1, LaunchStage::Downloading);
        assert_eq!(game.running_cluster(), Some(3));

        game.stages.insert(1, LaunchStage::Running);
        assert_eq!(game.running_cluster(), Some(1));
    }

    #[test]
    fn a_game_this_session_did_not_start_is_not_the_windows_business() {
        let mut game = GameState::default();
        game.stages.insert(1, LaunchStage::Running);
        game.stages.insert(2, LaunchStage::Running);

        assert_eq!(game.running_cluster_where(|id| id == 2), Some(2));
        assert_eq!(game.running_cluster_where(|_| false), None);
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

        // Held past a failure, which parks the stage at `Exited`.
        game.stages.insert(1, LaunchStage::Exited);
        assert_eq!(launch_button_state(&game, 1, false), ("Launching", false));

        game.finish_launch(1);
        assert_eq!(launch_button_state(&game, 1, false), ("Launch", true));
    }
}
