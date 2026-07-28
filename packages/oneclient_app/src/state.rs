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

use std::collections::HashMap;
use std::sync::Arc;

use freya::radio::RadioChannel;
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
}

impl GameState {
    #[must_use]
    pub fn stage(&self, cluster_id: i64) -> Option<LaunchStage> {
        self.stages.get(&cluster_id).copied()
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
