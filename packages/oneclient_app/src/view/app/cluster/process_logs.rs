
use freya::prelude::*;

use crate::components::LogViewer;
use crate::hooks::use_game_snapshot;
use crate::layout::cluster_content;
use crate::theme::colors;

use super::{cluster_not_found, load_cluster};

#[derive(PartialEq)]
pub struct ProcessLogs {
    pub cluster_id: i64,
}

impl Component for ProcessLogs {
    fn render(&self) -> impl IntoElement {
        let game = use_game_snapshot();
        let Some(_cluster) = load_cluster(self.cluster_id) else {
            return cluster_not_found();
        };

        let active = game.stage(self.cluster_id).is_some();

        let body = if active {
            LogViewer::new("Game output", game.logs_for(self.cluster_id))
                .streaming(true)
                .into_element()
        } else {
            not_running()
        };

        cluster_content()
            .child(body)
            .into_element()
    }
}

fn not_running() -> Element {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .child(
            label()
                .text("This cluster is not running.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
