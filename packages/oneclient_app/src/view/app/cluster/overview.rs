use freya::prelude::*;

use crate::components::ScrollArea;
use crate::hooks::{try_cluster_analytics, use_cluster_analytics};
use crate::layout::cluster_content;
use crate::theme::colors;
use crate::view::app::{analytics_body, analytics_placeholder};

use super::{cluster_not_found, load_cluster};

#[derive(PartialEq)]
pub struct ClusterOverview {
    pub cluster_id: i64,
}

impl Component for ClusterOverview {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;

        let Some(cluster) = load_cluster(cluster_id) else {
            return cluster_content().child(cluster_not_found()).into_element();
        };

        let query = use_cluster_analytics(cluster_id);
        let analytics = try_cluster_analytics(&query);

        let body: Element = match &analytics {
            None => centered_note("Loading play history…"),
            Some(a) if a.playtime.session_count == 0 => analytics_placeholder(
                "No sessions recorded for this cluster yet. Launch it and come back!",
            ),
            Some(a) => analytics_body(a),
        };

        cluster_content()
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(24.)
                            .child(overview_header(&cluster.name))
                            .child(body),
                    ),
            )
            .into_element()
    }
}

fn overview_header(name: &str) -> Element {
    rect()
        .vertical()
        .spacing(2.)
        .child(
            label()
                .text("Overview")
                .font_size(20.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(format!("Play history & servers for {name}"))
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn centered_note(text: &str) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::px(240.))
        .center()
        .child(
            label()
                .text(text.to_string())
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
