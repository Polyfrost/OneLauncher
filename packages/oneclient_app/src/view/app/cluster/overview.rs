use freya::prelude::*;

use crate::components::ScrollArea;
use crate::hooks::{try_cluster_analytics, use_cluster_analytics};
use crate::i18n::{t, tf};
use crate::layout::cluster_content;
use crate::theme::colors;
use crate::ui::centered_note;
use crate::view::app::{analytics_body, analytics_placeholder};

use super::cluster_not_found;
use crate::hooks::use_cluster;

#[derive(PartialEq)]
pub struct ClusterOverview {
    pub cluster_id: i64,
}

impl Component for ClusterOverview {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;

        let query = use_cluster_analytics(cluster_id);

        let Some(cluster) = use_cluster(cluster_id) else {
            return cluster_content().child(cluster_not_found()).into_element();
        };

        let analytics = try_cluster_analytics(&query);

        let body: Element = match &analytics {
            None => centered_note(t("oneclient.cluster.overview.loading")),
            Some(a) if a.playtime.session_count == 0 => {
                analytics_placeholder(t("oneclient.cluster.overview.empty"))
            }
            Some(a) => analytics_body(a),
        };

        cluster_content()
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .scrollbar_gutter(true)
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
                .text(t("oneclient.cluster.overview.title"))
                .font_size(20.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(tf("oneclient.cluster.overview.subtitle", &[&name]))
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
