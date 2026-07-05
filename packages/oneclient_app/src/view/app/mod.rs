mod analytics;
mod home;
mod accounts;
mod clusters;
mod debug;
mod skins;
mod stats;

pub mod browser;
pub mod settings;
pub mod cluster;

use crate::bridge::GameSnapshot;
use oneclient_core::notification::LaunchStage;

pub fn launch_button_state(game: &GameSnapshot, cluster_id: i64) -> (&'static str, bool) {
    match game.stage(cluster_id) {
        Some(LaunchStage::Checking) => ("Checking", false),
        Some(LaunchStage::Downloading) => ("Downloading", false),
        Some(LaunchStage::Launching) => ("Launching", false),
        Some(LaunchStage::Running) => ("Running", false),
        _ => ("Launch", true),
    }
}

pub(crate) use analytics::analytics_body;
pub use accounts::Accounts;
pub use home::Home;
pub use clusters::Clusters;
pub use debug::Debug;
pub use skins::AccountSkins;
pub use stats::Stats;
