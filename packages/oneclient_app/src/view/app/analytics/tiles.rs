use freya::prelude::*;

use oneclient_core::game::{Analytics, Persona};

use crate::components::{Icon, IconType};
use crate::utils::format_duration_hm;
use crate::theme::colors;
use crate::ui::border_all_color;

use super::card;

pub(super) fn tiles_row(analytics: &Analytics) -> Element {
    let stats = &analytics.playtime;
    let avg_session = if stats.session_count > 0 {
        stats.total_secs / stats.session_count as i64
    } else {
        0
    };
    let total_joins: i64 = analytics.servers.iter().map(|s| s.joins).sum();

    let mut tiles = vec![
        stat_tile(IconType::ClockRewind, "Total playtime", format_duration_hm(stats.total_secs)),
        stat_tile(IconType::Play, "Sessions", stats.session_count.to_string()),
        stat_tile(IconType::Calendar, "Avg / session", format_duration_hm(avg_session)),
        stat_tile(IconType::Rocket02, "Active days", stats.active_days.to_string()),
    ];
    if !analytics.servers.is_empty() {
        tiles.push(stat_tile(
            IconType::Globe01,
            "Server joins",
            format!("{total_joins}"),
        ));
    }

    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .spacing(16.)
        .children(tiles)
        .into_element()
}

fn stat_tile(icon: IconType, caption: &str, value: String) -> Element {
    card()
        .width(Size::flex(1.0))
        .spacing(10.)
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(Icon::new(icon).size(16.).color(colors::fg_secondary()))
                .child(
                    label()
                        .text(caption.to_string())
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            label()
                .text(value)
                .font_size(26.)
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .into_element()
}

pub(super) fn personas_row(personas: &[Persona]) -> Element {
    let mut row = rect().horizontal().width(Size::fill()).spacing(16.);
    for persona in personas {
        row = row.child(persona_card(*persona));
    }
    row.into_element()
}

fn persona_icon(persona: Persona) -> IconType {
    match persona {
        Persona::NightOwl => IconType::ClockRewind,
        Persona::Gamer => IconType::Rocket02,
    }
}

fn persona_card(persona: Persona) -> Element {
    card()
        .width(Size::flex(1.0))
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(14.)
        .border(border_all_color(1., colors::brand()))
        .child(
            rect()
                .width(Size::px(44.))
                .height(Size::px(44.))
                .corner_radius(CornerRadius::new_all(22.))
                .background(colors::brand().with_a(40))
                .center()
                .child(Icon::new(persona_icon(persona)).size(22.).color(colors::brand())),
        )
        .child(
            rect()
                .vertical()
                .spacing(2.)
                .child(
                    label()
                        .text(persona.title().to_string())
                        .font_size(18.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(persona.description().to_string())
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
        .into_element()
}
