use freya::prelude::*;

use crate::components::ScrollArea;
use crate::hooks::{try_global_analytics, use_global_analytics};
use crate::i18n::t;
use crate::theme::colors;
use crate::ui::centered_note;
use crate::view::app::{analytics_body, analytics_placeholder};

#[derive(PartialEq)]
pub struct Stats;

impl Component for Stats {
    fn render(&self) -> impl IntoElement {
        let query = use_global_analytics();
        let analytics = try_global_analytics(&query);

        let body: Element = match &analytics {
            None => loading_state(),
            Some(a) if a.playtime.session_count == 0 => {
                analytics_placeholder(t("oneclient.stats.empty"))
            }
            Some(a) => analytics_body(a),
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .padding(Gaps::new(0., 40., 40., 40.))
                            .spacing(24.)
                            .child(header())
                            .child(body),
                    ),
            )
    }
}

fn header() -> Element {
    rect()
        .vertical()
        .spacing(2.)
        .child(
            label()
                .text(t("oneclient.stats.title"))
                .font_size(32.)
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(t("oneclient.stats.description"))
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn loading_state() -> Element {
    centered_note(t("oneclient.stats.loading"))
}
