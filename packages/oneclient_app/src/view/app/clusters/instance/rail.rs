use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_common::parse_mc_version;

use super::shell::RAIL_WIDTH;
use crate::components::DynamicArt;

const HAIRLINE: Color = Color::from_argb(31, 255, 255, 255);
const CHIP_BG: Color = Color::from_argb(26, 255, 255, 255);
const SUB: Color = Color::from_argb(184, 213, 219, 255);
const MUTED: Color = Color::from_argb(140, 213, 219, 255);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Done,
    Current,
    Pending,
}

pub struct Rail {
    pub art: DynamicArt,
    pub title: String,
    pub subtitle: String,
    pub card: Element,
    pub tags: Vec<String>,
}

pub fn version_art(version: Option<&str>, loader: Option<GameLoader>) -> DynamicArt {
    match version.and_then(parse_mc_version) {
        Some(parsed) => DynamicArt::for_version(parsed.major, parsed.key(), loader),
        None => DynamicArt::fallback(),
    }
}

pub fn steps_card(rows: Vec<(&'static str, String, RowState)>) -> Element {
    rows_list(
        rows.into_iter()
            .map(|(name, value, state)| {
                let name_color = match state {
                    RowState::Current => Color::WHITE,
                    RowState::Done => SUB,
                    RowState::Pending => MUTED,
                };
                row(name, &value, name_color)
            })
            .collect(),
    )
}

pub fn facts_card(rows: Vec<(&'static str, String)>) -> Element {
    rows_list(
        rows.into_iter()
            .map(|(name, value)| row(name, &value, SUB))
            .collect(),
    )
}

fn rows_list(rows: Vec<Element>) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .children(rows)
        .into_element()
}

fn row(name: &'static str, value: &str, name_color: Color) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            label()
                .text(name)
                .width(Size::px(76.))
                .font_size(13.)
                .max_lines(1)
                .color(name_color),
        )
        .child(
            label()
                .text(value.to_string())
                .width(Size::flex(1.0))
                .font_size(13.)
                .font_weight(FontWeight::MEDIUM)
                .max_lines(1)
                .text_align(TextAlign::Right)
                .color(Color::WHITE),
        )
        .into_element()
}

pub fn rail(parts: Rail) -> Element {
    let Rail {
        art,
        title,
        subtitle,
        card,
        tags,
    } = parts;

    rect()
        .width(Size::px(RAIL_WIDTH))
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .position(Position::new_absolute())
                .child(art),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .layer(Layer::Relative(3))
                .main_align(Alignment::End)
                .padding(Gaps::new_all(24.))
                .spacing(18.)
                .background(
                    LinearGradient::new()
                        .angle(0.)
                        .stop((Color::from_argb(77, 11, 16, 19), 0.))
                        .stop((Color::from_argb(26, 11, 16, 19), 20.))
                        .stop((Color::from_argb(204, 11, 16, 19), 56.))
                        .stop((Color::from_argb(247, 11, 16, 19), 100.)),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(7.)
                        .child(
                            label()
                                .text(title)
                                .font_size(30.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .line_height(1.06)
                                .max_lines(2)
                                .color(Color::WHITE),
                        )
                        .child(
                            label()
                                .text(subtitle)
                                .font_size(13.)
                                .line_height(1.45)
                                .max_lines(3)
                                .color(SUB),
                        ),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(14.)
                        .child(card)
                        .maybe_child((!tags.is_empty()).then(|| {
                            rect()
                                .vertical()
                                .width(Size::fill())
                                .spacing(8.)
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .height(Size::px(1.))
                                        .background(HAIRLINE),
                                )
                                .child(
                                    rect()
                                        .horizontal()
                                        .width(Size::fill())
                                        .content(Content::wrap_spacing(6.))
                                        .spacing(6.)
                                        .children(tags.iter().map(|tag| {
                                            rect()
                                                .padding(Gaps::new_symmetric(3., 8.))
                                                .corner_radius(CornerRadius::new_all(6.))
                                                .background(CHIP_BG)
                                                .child(
                                                    label()
                                                        .text(tag.clone())
                                                        .font_size(11.)
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .color(Color::WHITE),
                                                )
                                                .into_element()
                                        })),
                                )
                                .into_element()
                        })),
                ),
        )
        .into_element()
}
