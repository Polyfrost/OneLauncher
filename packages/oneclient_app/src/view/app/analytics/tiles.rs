use freya::prelude::*;

use oneclient_core::game::{Analytics, Persona};

use crate::components::{Icon, IconType};
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::{format_duration, plural};

use super::card;

pub(super) fn tiles_row(analytics: &Analytics, _force_all: bool) -> Element {
    let stats = &analytics.playtime;
    let avg_session = if stats.session_count > 0 {
        stats.total_secs / stats.session_count as i64
    } else {
        0
    };

    let lead = rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .height(Size::px(LEAD_H))
        .spacing(TILE_GAP)
        .child(rect().width(Size::flex(2.0)).child(hero_tile(
            IconType::ClockRewind,
            "Total playtime",
            format_duration(stats.total_secs),
            format!(
                "over {} session{}",
                stats.session_count,
                plural(stats.session_count as i64)
            ),
        )))
        .child(stat_tile(
            IconType::Play,
            "Sessions",
            stats.session_count.to_string(),
        ))
        .child(stat_tile(
            IconType::Sliders04,
            "Avg / session",
            format_duration(avg_session),
        ));

    let rest = rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .height(Size::px(TILE_H))
        .spacing(TILE_GAP)
        .child(stat_tile(
            IconType::Maximize01,
            "Longest session",
            format_duration(stats.longest_session_secs),
        ))
        .child(stat_tile(
            IconType::Rocket02,
            "Day streak",
            streak_value(stats.current_streak),
        ))
        .child(stat_tile(
            IconType::CheckCircle,
            "Best streak",
            streak_value(stats.longest_streak),
        ))
        .child(stat_tile(
            IconType::Calendar,
            "Days played",
            stats.active_days.to_string(),
        ));

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(TILE_GAP)
        .child(lead)
        .child(rest)
        .into_element()
}

const TILE_GAP: f32 = 14.;
const LEAD_H: f32 = 128.;
const TILE_H: f32 = 104.;

const PERSONAS_PER_ROW: usize = 3;

fn streak_value(days: usize) -> String {
    format!("{days}d")
}

fn icon_chip(icon: IconType, size: f32, tint: Color) -> Element {
    rect()
        .width(Size::px(size))
        .height(Size::px(size))
        .corner_radius(CornerRadius::new_all(size * 0.5))
        .background(tint.with_a(38))
        .center()
        .child(Icon::new(icon).size(size * 0.52).color(tint))
        .into_element()
}

fn hero_tile(icon: IconType, caption: &str, value: String, note: String) -> Element {
    card()
        .width(Size::fill())
        .height(Size::fill())
        .spacing(12.)
        .background(colors::brand().with_a(18))
        .border(border_all_color(1., colors::brand().with_a(90)))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(icon_chip(icon, 30., colors::brand()))
                .child(
                    label()
                        .text(caption.to_string())
                        .font_size(13.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::End)
                .spacing(8.)
                .child(
                    label()
                        .text(value)
                        .font_size(38.)
                        .font_weight(FontWeight::BOLD)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .child(
                    rect().margin(Gaps::new(0., 0., 7., 0.)).child(
                        label()
                            .text(note)
                            .font_size(12.)
                            .max_lines(1)
                            .color(colors::fg_secondary()),
                    ),
                ),
        )
        .into_element()
}

fn stat_tile(icon: IconType, caption: &str, value: String) -> Element {
    rect()
        .width(Size::flex(1.0))
        .child(
            card()
                .width(Size::fill())
                .height(Size::fill())
                .spacing(10.)
                .child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(8.)
                        .child(icon_chip(icon, 24., colors::fg_secondary()))
                        .child(
                            label()
                                .text(caption.to_string())
                                .font_size(12.)
                                .max_lines(1)
                                .width(Size::fill())
                                .color(colors::fg_secondary()),
                        ),
                )
                .child(
                    label()
                        .text(value)
                        .font_size(24.)
                        .font_weight(FontWeight::BOLD)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                ),
        )
        .into_element()
}

pub(super) fn personas_row(personas: &[Persona]) -> Element {
    let mut grid = rect().vertical().width(Size::fill()).spacing(16.);
    for chunk in personas.chunks(PERSONAS_PER_ROW) {
        let mut row = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .spacing(16.);
        for persona in chunk {
            row = row.child(persona_card(*persona));
        }
        for _ in chunk.len()..PERSONAS_PER_ROW {
            row = row.child(rect().width(Size::flex(1.0)));
        }
        grid = grid.child(row);
    }
    grid.into_element()
}

fn persona_icon(persona: Persona) -> IconType {
    match persona {
        Persona::Veteran => IconType::ClipboardCheck,
        Persona::Marathoner => IconType::Maximize01,
        Persona::Regular => IconType::CheckCircle,
        Persona::Loyalist => IconType::Key01,
        Persona::Explorer => IconType::Globe01,
        Persona::NightOwl => IconType::ClockRewind,
        Persona::EarlyBird => IconType::Bell01,
        Persona::WeekendWarrior => IconType::Calendar,
        Persona::Gamer => IconType::Rocket02,
        Persona::Sprinter => IconType::Play,
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
                .child(
                    Icon::new(persona_icon(persona))
                        .size(22.)
                        .color(colors::brand()),
                ),
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
