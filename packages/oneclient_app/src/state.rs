//! [`AppChannel`] gives per-concern subscription a single `watch` channel woke every
//! consumer and deep-cloned the inbox on every event

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use freya::radio::RadioChannel;
use oneclient_common::domain::ProviderId;
use oneclient_content::packages::release_migration::ReleaseMigrationPlan;
use oneclient_core::clusters::Cluster;
use oneclient_core::relocate::{RelocationOutcome, RelocationPlan};
use oneclient_core::settings::LauncherSettings;
use oneclient_events::LaunchStage;

use crate::notifications::{InboxEntry, NotificationState, PendingPrompt};

/// Writing through a channel wakes only the components that subscribed to it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AppChannel {
    Launcher,
    Settings,
    Notifications,
    Game,
    AccountSwitcher,
    ControlCenter,
    MicrosoftLogin,
    Installs,
    StorageScan,
    Relocation,
    PendingLaunch,
    ReleaseMigration,
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
    pub control_center_open: bool,
    pub microsoft_login: Option<LoginProgress>,
    pub installs: InstallState,
    pub storage_scan: Option<StorageScanProgress>,
    pub relocation: RelocationState,
    pub pending_launch: Option<String>,
    pub release_migration: Option<ReleaseMigrationPrompt>,
    pub release_migration_checking: HashSet<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptOrigin {
    NewRelease,
    Manual,
    Simulated,
    Fake,
}

#[derive(Clone, Debug)]
pub enum ReleasePlanState {
    Loading,
    Ready(ReleaseMigrationPlan),
    Failed,
}

#[derive(Clone, Debug)]
pub struct ReleaseMigrationPrompt {
    pub key: String,
    pub target: Cluster,
    pub target_dedicated: bool,
    pub java_major: Option<u32>,
    pub sources: Vec<Cluster>,
    pub selected: i64,
    pub plans: HashMap<i64, ReleasePlanState>,
    pub origin: PromptOrigin,
}

impl ReleaseMigrationPrompt {
    #[must_use]
    pub fn source(&self) -> Option<&Cluster> {
        self.sources
            .iter()
            .find(|cluster| cluster.id == self.selected)
    }

    #[must_use]
    pub fn plan(&self) -> Option<&ReleasePlanState> {
        self.plans.get(&self.selected)
    }

    #[must_use]
    pub fn is_cross_loader(&self) -> bool {
        self.source()
            .is_some_and(|source| source.mc_loader != self.target.mc_loader)
    }
}

/// A move of the data folder owns the whole window while it runs: the router
/// swaps to [`crate::routes::Route::Relocating`] so nothing else can be touched
/// until the copy settles
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RelocationState {
    /// What is being moved where; cleared when the user leaves the move screen
    pub plan: Option<RelocationPlan>,
    /// Bytes written so far and the total to write, reported by the copy itself
    pub copied: u64,
    pub total: u64,
    /// Set once the copy settles, which turns the screen into its result
    pub result: Option<Result<RelocationOutcome, String>>,
}

/// In-flight installs so the button that started one stays disabled until it lands
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstallState {
    pending: HashSet<(i64, ProviderId, String)>,
    missing_lookups: HashSet<(i64, String)>,
    missing_found: HashMap<(i64, String), String>,
    pub flagged: Option<FlaggedInstallPrompt>,
    pub modpack_busy: bool,
    pub modpack_project: Option<(ProviderId, String)>,
    pub modpack_cluster: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlaggedInstallPrompt {
    pub cluster_id: i64,
    pub provider: ProviderId,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub mc_version: String,
    pub explanation: Option<String>,
    pub alternatives: Vec<oneclient_content::packages::ResolvedAlternative>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MissingModInstall {
    Idle,
    Searching,
    Installing(String),
    Finished(String),
}

impl InstallState {
    pub fn begin_missing_lookup(&mut self, cluster_id: i64, mod_id: &str) {
        self.missing_lookups
            .insert((cluster_id, mod_id.to_string()));
    }

    pub fn end_missing_lookup(
        &mut self,
        cluster_id: i64,
        mod_id: &str,
        project_id: Option<String>,
    ) {
        let key = (cluster_id, mod_id.to_string());
        self.missing_lookups.remove(&key);
        match project_id {
            Some(project_id) => {
                self.missing_found.insert(key, project_id);
            }
            None => {
                self.missing_found.remove(&key);
            }
        }
    }

    #[must_use]
    pub fn missing_mod_install(&self, cluster_id: i64, mod_id: &str) -> MissingModInstall {
        let key = (cluster_id, mod_id.to_string());
        if self.missing_lookups.contains(&key) {
            return MissingModInstall::Searching;
        }

        match self.missing_found.get(&key) {
            Some(project_id)
                if self.is_installing(cluster_id, ProviderId::Modrinth, project_id) =>
            {
                MissingModInstall::Installing(project_id.clone())
            }
            Some(project_id) => MissingModInstall::Finished(project_id.clone()),
            None => MissingModInstall::Idle,
        }
    }

    pub fn begin(&mut self, cluster_id: i64, provider: ProviderId, project_id: String) {
        self.pending.insert((cluster_id, provider, project_id));
    }

    pub fn finish(&mut self, cluster_id: i64, provider: ProviderId, project_id: &str) {
        self.pending
            .remove(&(cluster_id, provider, project_id.to_string()));
    }

    #[must_use]
    pub fn is_modpack_job(&self, cluster_id: i64) -> bool {
        self.modpack_cluster == Some(cluster_id)
    }

    #[must_use]
    pub fn package_busy(
        &self,
        is_modpack: bool,
        cluster_id: i64,
        provider: ProviderId,
        project_id: &str,
    ) -> (bool, bool) {
        if !is_modpack {
            return (self.is_installing(cluster_id, provider, project_id), false);
        }
        let installing =
            self.modpack_project
                .as_ref()
                .is_some_and(|(busy_provider, busy_project)| {
                    *busy_provider == provider && busy_project == project_id
                });
        (installing, self.modpack_busy)
    }

    #[must_use]
    pub fn cluster_installing(&self, cluster_id: i64) -> bool {
        self.pending.iter().any(|(id, _, _)| *id == cluster_id)
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
    pub syncing_bundles: bool,
    pub error: Option<String>,
    pub data_dir: String,
    pub snapshots: usize,
    pub needs_location: bool,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageScanProgress {
    pub label: String,
    pub current: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchBlock {
    Starting(i64),
    Running(i64),
}

impl LaunchBlock {
    #[must_use]
    pub fn cluster_id(self) -> i64 {
        match self {
            Self::Starting(id) | Self::Running(id) => id,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameState {
    pub stages: HashMap<i64, LaunchStage>,
    pub error: Option<String>,
    crashes: Vec<Arc<oneclient_events::GameCrash>>,
    pub logs: HashMap<i64, Arc<Vec<Arc<str>>>>,
    /// Launches started from the UI but not yet answered by core which takes a few
    /// hundred ms every click in that window otherwise spawns its own game
    pending: HashSet<i64>,
    shared_syncs: HashSet<i64>,
}

impl GameState {
    pub fn set_shared_sync(&mut self, cluster_id: i64, syncing: bool) {
        if syncing {
            self.shared_syncs.insert(cluster_id);
        } else {
            self.shared_syncs.remove(&cluster_id);
        }
    }

    #[must_use]
    pub fn is_syncing_shared(&self, cluster_id: i64) -> bool {
        self.shared_syncs.contains(&cluster_id)
    }

    #[must_use]
    pub fn any_shared_sync(&self) -> bool {
        !self.shared_syncs.is_empty()
    }

    #[must_use]
    pub fn stage(&self, cluster_id: i64) -> Option<LaunchStage> {
        self.stages.get(&cluster_id).copied()
    }

    #[must_use]
    pub fn current_crash(&self) -> Option<Arc<oneclient_events::GameCrash>> {
        self.crashes.first().cloned()
    }

    pub fn push_crash(&mut self, crash: oneclient_events::GameCrash) {
        self.clear_crash(crash.cluster_id);
        self.crashes.push(Arc::new(crash));
    }

    pub fn clear_crash(&mut self, cluster_id: i64) {
        self.crashes.retain(|crash| crash.cluster_id != cluster_id);
    }

    /// Returns false if a launch is already in flight the re-entrancy guard
    pub fn begin_launch(&mut self, cluster_id: i64) -> bool {
        if self.block_for(cluster_id).is_some() {
            return false;
        }
        self.pending.insert(cluster_id);
        true
    }

    fn block_for(&self, cluster_id: i64) -> Option<LaunchBlock> {
        if self.is_running(cluster_id) {
            Some(LaunchBlock::Running(cluster_id))
        } else if self.is_active(cluster_id) || self.is_launch_pending(cluster_id) {
            Some(LaunchBlock::Starting(cluster_id))
        } else {
            None
        }
    }

    /// `shared` lists the other clusters that would share the launch's game dir
    /// empty for a dedicated cluster with parallel clusters allowed only those
    /// can be in the way
    #[must_use]
    pub fn launch_block(
        &self,
        cluster_id: i64,
        parallel: bool,
        shared: &[i64],
    ) -> Option<LaunchBlock> {
        if let Some(block) = self.block_for(cluster_id) {
            return Some(block);
        }

        self.stages
            .keys()
            .chain(self.pending.iter())
            .copied()
            .filter(|id| *id != cluster_id && (!parallel || shared.contains(id)))
            .find_map(|id| self.block_for(id))
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

    pub fn running_clusters(&self) -> impl Iterator<Item = i64> + '_ {
        self.stages
            .iter()
            .filter(|(_, stage)| **stage == LaunchStage::Running)
            .map(|(id, _)| *id)
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
    use crate::view::app::{launch_button_state, shared_dir_taken};
    use crate::view::onboarding::test_support::cluster;

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
    fn a_shortcut_is_told_which_cluster_is_in_the_way() {
        let mut game = GameState::default();
        game.stages.insert(1, LaunchStage::Running);

        assert_eq!(
            game.launch_block(1, false, &[]),
            Some(LaunchBlock::Running(1))
        );
        assert_eq!(
            game.launch_block(2, false, &[]),
            Some(LaunchBlock::Running(1))
        );
        assert_eq!(game.launch_block(2, true, &[]), None);
    }

    #[test]
    fn parallel_clusters_still_hold_one_shared_game() {
        let mut game = GameState::default();
        game.stages.insert(1, LaunchStage::Running);

        // 2 shares the game dir with 1 and 3 has its own
        assert_eq!(
            game.launch_block(2, true, &[1]),
            Some(LaunchBlock::Running(1))
        );
        assert_eq!(game.launch_block(3, true, &[]), None);
        assert_eq!(
            game.launch_block(1, true, &[]),
            Some(LaunchBlock::Running(1))
        );
    }

    #[test]
    fn a_game_that_is_still_coming_up_still_blocks() {
        let mut game = GameState::default();
        game.stages.insert(1, LaunchStage::Downloading);

        assert_eq!(
            game.launch_block(1, false, &[]),
            Some(LaunchBlock::Starting(1))
        );
        assert_eq!(
            game.launch_block(2, false, &[]),
            Some(LaunchBlock::Starting(1))
        );
    }

    #[test]
    fn a_claim_with_no_stage_yet_blocks_too() {
        let mut game = GameState::default();
        game.begin_launch(1);

        assert_eq!(
            game.launch_block(1, false, &[]),
            Some(LaunchBlock::Starting(1))
        );
        assert_eq!(
            game.launch_block(2, false, &[]),
            Some(LaunchBlock::Starting(1))
        );
    }

    #[test]
    fn an_exited_game_is_out_of_the_way() {
        let mut game = GameState::default();
        game.stages.insert(1, LaunchStage::Exited);

        assert_eq!(game.launch_block(2, false, &[]), None);
        assert_eq!(game.launch_block(1, false, &[]), None);
    }

    #[test]
    fn the_button_disables_on_the_claim_alone() {
        let mut game = GameState::default();
        assert_eq!(
            launch_button_state(&game, 1, false, false),
            ("Launch", true)
        );

        game.begin_launch(1);
        assert_eq!(
            launch_button_state(&game, 1, false, false),
            ("Launching", false)
        );

        // Held past a failure which parks the stage at `Exited`
        game.stages.insert(1, LaunchStage::Exited);
        assert_eq!(
            launch_button_state(&game, 1, false, false),
            ("Launching", false)
        );

        game.finish_launch(1);
        assert_eq!(
            launch_button_state(&game, 1, false, false),
            ("Launch", true)
        );
    }

    #[test]
    fn a_queued_launch_says_it_is_waiting() {
        let mut game = GameState::default();
        game.stages.insert(1, LaunchStage::Waiting);

        assert_eq!(
            launch_button_state(&game, 1, false, false),
            ("Waiting", false)
        );
        assert!(game.is_busy(1));
    }

    #[test]
    fn a_taken_shared_dir_blocks_only_an_idle_cluster() {
        let mut game = GameState::default();
        assert_eq!(
            launch_button_state(&game, 1, false, true),
            ("Folder in use", false)
        );

        game.stages.insert(1, LaunchStage::Running);
        assert_eq!(
            launch_button_state(&game, 1, false, true),
            ("Running", false)
        );
    }

    fn crash(cluster_id: i64, title: &str) -> oneclient_events::GameCrash {
        oneclient_events::GameCrash {
            cluster_id,
            cluster_name: format!("Cluster {cluster_id}"),
            title: title.to_string(),
            exit: String::new(),
            played_secs: 0,
            cause: None,
            suspects: Vec::new(),
            remedy: None,
            fixes: Vec::new(),
            excerpt: Vec::new(),
            game_dir: None,
            mod_issues: Vec::new(),
        }
    }

    #[test]
    fn crashes_from_parallel_clusters_queue_up_oldest_first() {
        let mut game = GameState::default();
        game.push_crash(crash(1, "first"));
        game.push_crash(crash(2, "second"));

        assert_eq!(game.current_crash().unwrap().title, "first");

        game.clear_crash(1);
        assert_eq!(game.current_crash().unwrap().title, "second");

        game.clear_crash(2);
        assert!(game.current_crash().is_none());
    }

    #[test]
    fn a_cluster_keeps_only_its_latest_crash() {
        let mut game = GameState::default();
        game.push_crash(crash(1, "old"));
        game.push_crash(crash(2, "other"));
        game.push_crash(crash(1, "new"));

        assert_eq!(game.current_crash().unwrap().title, "other");

        game.clear_crash(2);
        assert_eq!(game.current_crash().unwrap().title, "new");

        game.clear_crash(1);
        assert!(game.current_crash().is_none());
    }

    #[test]
    fn a_missing_mod_install_is_tracked_from_lookup_to_finish() {
        let mut installs = InstallState::default();
        assert_eq!(
            installs.missing_mod_install(1, "sodium"),
            MissingModInstall::Idle
        );

        installs.begin_missing_lookup(1, "sodium");
        assert_eq!(
            installs.missing_mod_install(1, "sodium"),
            MissingModInstall::Searching
        );

        installs.begin(1, ProviderId::Modrinth, "AANobbMI".to_string());
        installs.end_missing_lookup(1, "sodium", Some("AANobbMI".to_string()));
        assert_eq!(
            installs.missing_mod_install(1, "sodium"),
            MissingModInstall::Installing("AANobbMI".to_string())
        );

        installs.finish(1, ProviderId::Modrinth, "AANobbMI");
        assert_eq!(
            installs.missing_mod_install(1, "sodium"),
            MissingModInstall::Finished("AANobbMI".to_string())
        );
        assert_eq!(
            installs.missing_mod_install(2, "sodium"),
            MissingModInstall::Idle
        );
    }

    #[test]
    fn a_lookup_that_finds_nothing_goes_back_to_idle() {
        let mut installs = InstallState::default();
        installs.begin_missing_lookup(1, "yet_another_config_lib_v3");
        installs.end_missing_lookup(1, "yet_another_config_lib_v3", None);

        assert_eq!(
            installs.missing_mod_install(1, "yet_another_config_lib_v3"),
            MissingModInstall::Idle
        );
    }

    #[test]
    fn a_shared_sync_takes_the_shared_dir_but_not_a_dedicated_one() {
        let mut game = GameState::default();
        let shared = cluster(1);
        let dedicated = Cluster {
            kind: oneclient_db::models::ClusterKind::Vanilla,
            ..cluster(2)
        };
        let clusters = [shared, dedicated];

        game.set_shared_sync(3, true);
        assert!(shared_dir_taken(&game, &clusters, 1));
        assert!(!shared_dir_taken(&game, &clusters, 2));
        assert_eq!(
            launch_button_state(&game, 3, false, false),
            ("Syncing", false)
        );

        game.set_shared_sync(3, false);
        assert!(!shared_dir_taken(&game, &clusters, 1));
    }
}
