use freya::prelude::*;

use crate::components::{Icon, IconType, ScrollArea};
use crate::hooks::{try_cluster_analytics, use_cluster, use_cluster_analytics};
use crate::i18n::t;
use crate::layout::cluster_content;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::view::app::analytics_body;

use super::cluster_not_found;

#[derive(PartialEq)]
pub struct ClusterOverview {
    pub cluster_id: i64,
}

impl Component for ClusterOverview {
    fn render(&self) -> impl IntoElement {
        let cluster = use_cluster(self.cluster_id);
        let query = use_cluster_analytics(self.cluster_id);
        if cluster.is_none() {
            return cluster_content().child(cluster_not_found()).into_element();
        };
        let analytics = try_cluster_analytics(&query);
        let body = match &analytics {
            None => activity_notice(
                IconType::ClockRewind,
                t("oneclient.cluster.overview.loading"),
                None,
            ),
            Some(analytics) if analytics.playtime.session_count == 0 => activity_notice(
                IconType::BarChartSquare02,
                t("oneclient.cluster.overview.no_history"),
                Some(t("oneclient.cluster.overview.start_history")),
            ),
            Some(analytics) => analytics_body(analytics),
        };

        cluster_content()
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .scrollbar_gutter(true)
                    .padding(Gaps::new(0., 8., 14., 0.))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .max_width(Size::px(1360.))
                            .spacing(16.)
                            .child(
                                label()
                                    .text(t("oneclient.cluster.overview.title"))
                                    .font_size(text_size(18.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(body),
                    ),
            )
            .into_element()
    }
}

fn activity_notice(icon: IconType, title: &str, hint: Option<&str>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .padding(Gaps::new_all(17.))
        .spacing(14.)
        .background(colors::page_elevated())
        .corner_radius(rounded(12.))
        .border(border_all_color(1., colors::component_border()))
        .child(Icon::new(icon).size(24.).color(colors::fg_primary()))
        .child(
            rect()
                .vertical()
                .spacing(3.)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(text_size(15.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .maybe_child(hint.map(|hint| {
                    label()
                        .text(hint.to_string())
                        .font_size(text_size(13.))
                        .color(colors::fg_secondary())
                })),
        )
        .into_element()
}
