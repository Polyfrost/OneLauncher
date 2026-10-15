mod analytics;
mod clusters;
mod debug;
mod home;
mod skins;
mod stats;

pub mod browser;
pub mod cluster;
pub mod settings;

use crate::hooks::{settled_or_loading, use_clusters, use_game_snapshot};
use crate::state::{GameState as GameSnapshot, LauncherInit};
use oneclient_core::clusters::Cluster;
use oneclient_events::LaunchStage;

pub fn launch_button_state(
    game: &GameSnapshot,
    cluster_id: i64,
    syncing: bool,
    shared_dir_taken: bool,
) -> (&'static str, bool) {
    let state = match game.stage(cluster_id) {
        Some(LaunchStage::Checking) => ("Checking", false),
        Some(LaunchStage::Waiting) => ("Waiting", false),
        Some(LaunchStage::Downloading) => ("Downloading", false),
        Some(LaunchStage::Launching) => ("Launching", false),
        Some(LaunchStage::Running) => ("Running", false),
        // The click's own flag held for a beat after a failure so a burst of clicks starts one game
        _ if game.is_launch_pending(cluster_id) => ("Launching", false),
        // Core refuses it anyway the shared game dir holds one game at a time
        _ if shared_dir_taken => ("Folder in use", false),
        _ => ("Launch", true),
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
    // Active games are few so filter on them before touching the marker files
    let mut others = clusters
        .iter()
        .filter(|c| c.id != cluster_id && (game.is_active(c.id) || game.is_launch_pending(c.id)))
        .peekable();

    others.peek().is_some()
        && clusters
            .iter()
            .find(|c| c.id == cluster_id)
            .is_some_and(|c| !c.uses_dedicated_dir())
        && others.any(|c| !c.uses_dedicated_dir())
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
pub use debug::Debug;
pub use home::Home;
pub use skins::AccountSkins;
pub use stats::Stats;
