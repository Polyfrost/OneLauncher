use freya::prelude::*;

use oneclient_core::game::{Analytics, Persona};

use crate::components::{Icon, IconType};
use crate::theme::colors;
use crate::ui::{border_all_color, columns_for, flow_grid};
use crate::utils::{format_duration, plural};

use super::card;

pub(super) fn tiles_row(analytics: &Analytics, _force_all: bool) -> Element {
    TilesRow {
        analytics: analytics.clone(),
    }
    .into_element()
}

const TILE_GAP: f32 = 14.;
const TILE_MIN_W: f32 = 190.;
const TILE_H: f32 = 104.;

#[derive(PartialEq)]
struct TilesRow {
    analytics: Analytics,
}

impl Component for TilesRow {
    fn render(&self) -> impl IntoElement {
        let width = use_state(|| 0f32);
        let cols = columns_for(*width.read(), TILE_MIN_W, 4, TILE_GAP);

        let stats = &self.analytics.playtime;
        let avg_session = if stats.session_count > 0 {
            stats.total_secs / stats.session_count as i64
        } else {
            0
        };

        let hero = hero_tile(
            IconType::ClockRewind,
            "Total playtime",
            format_duration(stats.total_secs),
            format!(
                "over {} session{}",
                stats.session_count,
                plural(stats.session_count as i64)
            ),
        );

        let mut rest = vec![
            stat_tile(IconType::Play, "Sessions", stats.session_count.to_string()),
            stat_tile(
                IconType::Sliders04,
                "Avg / session",
                format_duration(avg_session),
            ),
            stat_tile(
                IconType::Maximize01,
                "Longest session",
                format_duration(stats.longest_session_secs),
            ),
            stat_tile(
                IconType::Rocket02,
                "Day streak",
                streak_value(stats.current_streak),
            ),
            stat_tile(
                IconType::CheckCircle,
                "Best streak",
                streak_value(stats.longest_streak),
            ),
            stat_tile(
                IconType::Calendar,
                "Days played",
                stats.active_days.to_string(),
            ),
        ];

        let mut root = rect().vertical().width(Size::fill()).spacing(TILE_GAP);

        if cols >= 3 {
            let alongside: Vec<Element> = rest.drain(..(cols - 2).min(rest.len())).collect();
            let mut lead = rect()
                .horizontal()
                .content(Content::Flex)
                .width(Size::fill())
                .height(Size::px(TILE_H))
                .spacing(TILE_GAP)
                .child(
                    rect()
                        .width(Size::flex(2.0))
                        .height(Size::fill())
                        .child(hero),
                );
            for tile in alongside {
                lead = lead.child(
                    rect()
                        .width(Size::flex(1.0))
                        .height(Size::fill())
                        .child(tile),
                );
            }
            root = root.child(lead);
        } else {
            root = root.child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(TILE_H))
                    .child(hero),
            );
        }

        let rows: Vec<Element> = rest
            .into_iter()
            .map(|tile| {
                rect()
                    .height(Size::px(TILE_H))
                    .width(Size::flex(1.0))
                    .child(tile)
                    .into_element()
            })
            .collect();

        root.child(flow_grid(rows, cols, width, TILE_GAP))
    }
}

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
        .padding(Gaps::new_symmetric(16., 20.))
        .spacing(10.)
        .background(colors::brand().with_a(18))
        .border(border_all_color(1., colors::brand().with_a(90)))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(icon_chip(icon, 24., colors::brand()))
                .child(
                    label()
                        .text(caption.to_string())
                        .font_size(12.)
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
                        .font_size(30.)
                        .font_weight(FontWeight::BOLD)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .child(
                    rect().margin(Gaps::new(0., 0., 5., 0.)).child(
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
        )
        .into_element()
}

pub(super) fn personas_row(personas: &[Persona]) -> Element {
    rect()
        .horizontal()
        .content(Content::Wrap {
            wrap_spacing: Some(10.),
        })
        .width(Size::fill())
        .spacing(10.)
        .children(personas.iter().map(|p| persona_badge(*p)))
        .into_element()
}

fn persona_badge(persona: Persona) -> Element {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(7.)
        .padding(Gaps::new_symmetric(6., 11.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(
            Icon::new(persona_icon(persona))
                .size(13.)
                .color(colors::brand()),
        )
        .child(
            label()
                .text(persona.title().to_string())
                .font_size(13.)
                .font_weight(FontWeight::SEMI_BOLD)
                .max_lines(1)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(persona.description().to_string())
                .font_size(11.)
                .max_lines(1)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn persona_icon(persona: Persona) -> IconType {
    match persona {
        Persona::Veteran => IconType::ClockRewind,
        Persona::Marathoner => IconType::Maximize01,
        Persona::Regular => IconType::CheckCircle,
        Persona::Loyalist => IconType::Key01,
        Persona::Explorer => IconType::Globe01,
        Persona::NightOwl => IconType::Eye,
        Persona::EarlyBird => IconType::Bell01,
        Persona::WeekendWarrior => IconType::Calendar,
        Persona::Gamer => IconType::Rocket02,
        Persona::Sprinter => IconType::Play,
    }
}
