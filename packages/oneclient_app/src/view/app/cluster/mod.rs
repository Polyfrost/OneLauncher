mod logs;
mod overview;
mod package_manager;

mod cluster_settings;
mod mods;
mod process_logs;
mod screenshots;
mod shaders;
mod textures;

pub use logs::ClusterLogs;
pub use overview::ClusterOverview;

pub use cluster_settings::ClusterSettings;
pub use mods::ClusterMods;
pub use process_logs::ProcessLogs;
pub use screenshots::ClusterScreenshots;
pub use shaders::ClusterShaders;
pub use textures::ClusterTextures;

use freya::prelude::*;

use crate::theme::colors;

pub(crate) fn cluster_not_found() -> Element {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .child(
            label()
                .text("Cluster not found.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
