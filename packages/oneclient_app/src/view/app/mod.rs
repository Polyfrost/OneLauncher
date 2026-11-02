mod analytics;
mod clusters;
mod debug;
mod home;
mod stats;
mod wardrobe;

pub mod browser;
pub mod cluster;
pub mod settings;

use crate::hooks::{settled_or_loading, use_clusters, use_game_snapshot};
use crate::i18n::t;
use crate::state::{GameState as GameSnapshot, LauncherInit};
use oneclient_core::clusters::{Cluster, DirTarget};
use oneclient_events::LaunchStage;

pub fn launch_button_state(
    game: &GameSnapshot,
    cluster_id: i64,
    syncing: bool,
    shared_dir_taken: bool,
) -> (&'static str, bool) {
    let state = match game.stage(cluster_id) {
        Some(LaunchStage::Checking) => (t("oneclient.app.launch.checking"), false),
        Some(LaunchStage::Waiting) => (t("oneclient.app.launch.waiting"), false),
        Some(LaunchStage::Downloading) => (t("oneclient.app.launch.downloading"), false),
        Some(LaunchStage::Launching) => (t("oneclient.app.launch.launching"), false),
        Some(LaunchStage::Running) => (t("oneclient.app.launch.running"), false),
        // The click's own flag held for a beat after a failure so a burst of clicks starts one game
        _ if game.is_launch_pending(cluster_id) => (t("oneclient.app.launch.launching"), false),
        _ if game.is_checking_updates(cluster_id) => (t("oneclient.app.launch.checking"), false),
        _ if game.is_syncing_shared(cluster_id) => (t("oneclient.app.launch.syncing"), false),
        // Core refuses it anyway the shared game dir holds one game at a time
        _ if shared_dir_taken => (t("oneclient.app.launch.folder_in_use"), false),
        _ => (t("oneclient.app.launch.launch"), true),
    };
    // Block launching while the startup bundle download is still running
    if syncing && state.1 {
        return (state.0, false);
    }
    state
}

/// Whether another shared cluster is starting or running in the shared game dir
/// dedicated clusters never collide so any number of them may run alongside
pub fn shared_dir_taken(game: &GameSnapshot, clusters: &[Cluster], cluster_id: i64) -> bool {
    let Some(this) = clusters.iter().find(|c| c.id == cluster_id) else {
        return false;
    };

    this.dir_target != DirTarget::Dedicated
        && (game.shared_sync_touches(&this.dir_target)
            || clusters.iter().any(|c| {
                c.id != cluster_id
                    && (game.is_active(c.id) || game.is_launch_pending(c.id))
                    && c.shares_dir_with(this)
            }))
}

pub fn use_shared_dir_taken(cluster_id: i64) -> bool {
    let game = use_game_snapshot();
    let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
    shared_dir_taken(&game, &clusters, cluster_id)
}

pub fn launch_syncing(launcher: &LauncherInit, uses_bundles: bool) -> bool {
    launcher.fetching || (uses_bundles && launcher.syncing_bundles)
}

pub(crate) use analytics::{analytics_body, analytics_placeholder};
pub use clusters::Clusters;
pub(crate) use clusters::{ClusterPickerButton, DeleteInstanceModal};
pub use debug::Debug;
pub use home::Home;
pub use stats::Stats;
pub use wardrobe::Wardrobe;
