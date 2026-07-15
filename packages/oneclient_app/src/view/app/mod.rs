mod accounts;
mod analytics;
mod clusters;
mod debug;
mod home;
mod skins;
mod stats;

pub mod browser;
pub mod cluster;
pub mod settings;

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

pub use accounts::Accounts;
pub(crate) use analytics::analytics_body;
pub use clusters::Clusters;
pub use debug::Debug;
pub use home::Home;
pub use skins::AccountSkins;
pub use stats::Stats;
