use freya::prelude::*;

use crate::components::ScrollArea;
use crate::hooks::{try_global_analytics, use_global_analytics};
use crate::theme::colors;
use crate::view::app::analytics_body;

#[derive(PartialEq)]
pub struct Stats;

impl Component for Stats {
    fn render(&self) -> impl IntoElement {
        let query = use_global_analytics();
        let analytics = try_global_analytics(&query);

        let body: Element = match &analytics {
            None => loading_state(),
            Some(a) if a.playtime.session_count == 0 => empty_state(),
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
                            .padding(40.)
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
                .text("Statistics")
                .font_size(32.)
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text("How, when, and how much you play.")
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn loading_state() -> Element {
    centered_note("Crunching your playtime…")
}

fn empty_state() -> Element {
    centered_note("No playtime recorded yet. Launch a game and come back!")
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
