use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_common::parse_mc_version;

use super::shell::RAIL_WIDTH;
use crate::components::DynamicArt;
use crate::theme::{colors, rounded, text_size};

const HAIRLINE: Color = Color::from_argb(31, 255, 255, 255);
const CHIP_BG: Color = Color::from_argb(26, 255, 255, 255);

fn sub() -> Color {
    colors::on_media().with_a(184)
}

fn muted() -> Color {
    colors::on_media().with_a(140)
}

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
                    RowState::Done => sub(),
                    RowState::Pending => muted(),
                };
                row(name, &value, name_color)
            })
            .collect(),
    )
}

pub fn facts_card(rows: Vec<(&'static str, String)>) -> Element {
    rows_list(
        rows.into_iter()
            .map(|(name, value)| row(name, &value, sub()))
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
                .font_size(text_size(13.))
                .max_lines(1)
                .color(name_color),
        )
        .child(
            label()
                .text(value.to_string())
                .width(Size::flex(1.0))
                .font_size(text_size(13.))
                .font_weight(FontWeight::MEDIUM)
                .max_lines(1)
                .text_align(TextAlign::Right)
                .color(rail_ink()),
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
    let backing = if colors::is_light() {
        colors::page_elevated()
    } else {
        Color::from_rgb(11, 16, 19)
    };

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
                        .stop((backing.with_a(77), 0.))
                        .stop((backing.with_a(26), 20.))
                        .stop((backing.with_a(204), 56.))
                        .stop((backing.with_a(247), 100.)),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(7.)
                        .child(
                            label()
                                .text(title)
                                .font_size(text_size(30.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .line_height(1.06)
                                .max_lines(2)
                                .color(rail_ink()),
                        )
                        .child(
                            label()
                                .text(subtitle)
                                .font_size(text_size(13.))
                                .line_height(1.45)
                                .max_lines(3)
                                .color(sub()),
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
                                        .background(rail_ink().with_a(31)),
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
                                                .corner_radius(rounded(6.))
                                                .background(rail_ink().with_a(26))
                                                .child(
                                                    label()
                                                        .text(tag.clone())
                                                        .font_size(text_size(11.))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .color(rail_ink()),
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
