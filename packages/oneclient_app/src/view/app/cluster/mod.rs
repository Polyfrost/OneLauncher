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
use freya::query::QueryStateData;
use oneclient_core::clusters::Cluster;

use crate::hooks::use_clusters;
use crate::theme::colors;

pub(crate) fn load_cluster(cluster_id: i64) -> Option<Cluster> {
    let clusters_query = use_clusters();
    let reader = clusters_query.read();
    let list = match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. }
        | QueryStateData::Loading {
            res: Some(Ok(list)),
        } => list.clone(),
        _ => Vec::new(),
    };
    list.into_iter().find(|c| c.id == cluster_id)
}

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
