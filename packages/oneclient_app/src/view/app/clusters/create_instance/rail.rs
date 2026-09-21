use freya::prelude::*;
use oneclient_common::parse_mc_version;

use super::{Picks, RAIL_WIDTH, Step, TypeChoice, Wizard};
use crate::components::{DynamicArt, Icon, IconType};
use crate::theme::colors;

const CARD_BG: Color = Color::from_argb(158, 11, 16, 19);
const HAIRLINE: Color = Color::from_argb(31, 255, 255, 255);
const CHIP_BG: Color = Color::from_argb(26, 255, 255, 255);
const SUB: Color = Color::from_argb(184, 213, 219, 255);
const MUTED: Color = Color::from_argb(140, 213, 219, 255);

fn art(picks: &Picks, cover: Option<std::path::PathBuf>) -> DynamicArt {
    let parsed = picks.version.as_deref().and_then(parse_mc_version);
    match parsed {
        Some(parsed) => DynamicArt::for_version(parsed.major, parsed.key(), picks.loader),
        None => DynamicArt::fallback(),
    }
    .picked_cover(cover)
}

fn step_value(picks: &Picks, step: Step, tags: usize) -> String {
    match step {
        Step::Type => match picks.choice {
            Some(TypeChoice::OneClient) => "OneClient".to_string(),
            Some(TypeChoice::Scratch) => "From scratch".to_string(),
            None => String::new(),
        },
        Step::Loader => picks.loader_label(),
        Step::Version => picks
            .version
            .clone()
            .unwrap_or_else(|| "Not chosen".to_string()),
        Step::Bundles => {
            let taken = picks.taken_bundles().len();
            if taken == 0 {
                "None".to_string()
            } else {
                format!("{taken} selected")
            }
        }
        Step::Customize => match tags {
            0 => "Optional".to_string(),
            1 => "1 tag".to_string(),
            many => format!("{many} tags"),
        },
    }
}

fn step_row(picks: &Picks, position: usize, step: Step, tags: usize) -> Element {
    let done = position < picks.index;
    let now = position == picks.index;

    let dot_bg = if done {
        colors::brand()
    } else {
        Color::TRANSPARENT
    };
    let dot_border = if done || now {
        colors::brand()
    } else {
        HAIRLINE
    };

    let value = if done || now {
        step_value(picks, step, tags)
    } else {
        String::new()
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(11.)
        .child(
            rect()
                .width(Size::px(20.))
                .height(Size::px(20.))
                .center()
                .corner_radius(CornerRadius::new_all(6.))
                .background(dot_bg)
                .border(crate::ui::border_all_color(1., dot_border))
                .child(if done {
                    Icon::new(IconType::Check)
                        .size(12.)
                        .color(Color::WHITE)
                        .into_element()
                } else {
                    label()
                        .text((position + 1).to_string())
                        .font_size(11.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(if now { Color::WHITE } else { MUTED })
                        .into_element()
                }),
        )
        .child(
            label()
                .text(step.label())
                .width(Size::px(76.))
                .font_size(12.)
                .max_lines(1)
                .color(if now { Color::WHITE } else { SUB }),
        )
        .child(
            label()
                .text(value)
                .width(Size::flex(1.0))
                .font_size(13.)
                .font_weight(FontWeight::MEDIUM)
                .max_lines(1)
                .text_align(TextAlign::Right)
                .color(if done { Color::WHITE } else { SUB }),
        )
        .into_element()
}

pub fn rail(wizard: Wizard, picks: &Picks) -> Element {
    let cover = wizard.cover.read().clone();
    let description = wizard.description.read().trim().to_string();
    let tags = wizard.tags.read().clone();

    let title = if picks.name.trim().is_empty() {
        "New instance".to_string()
    } else {
        picks.name.clone()
    };

    let subtitle = if description.is_empty() {
        match picks.choice {
            None => "Pick a type to get started".to_string(),
            Some(TypeChoice::OneClient) => match &picks.version {
                Some(version) => format!("OneClient · {version}"),
                None => "OneClient".to_string(),
            },
            Some(TypeChoice::Scratch) => match &picks.version {
                Some(version) => format!("{version} · {}", picks.loader_label()),
                None => picks.loader_label(),
            },
        }
    } else {
        description
    };

    let show_tags = picks.step == Step::Customize && !tags.is_empty();

    rect()
        .width(Size::px(RAIL_WIDTH))
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .position(Position::new_absolute())
                .child(art(picks, cover)),
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
                        .angle(180.)
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
                        .spacing(12.)
                        .padding(Gaps::new_all(16.))
                        .corner_radius(CornerRadius::new_all(12.))
                        .background(CARD_BG)
                        .border(crate::ui::border_all_color(1., HAIRLINE))
                        .child(
                            label()
                                .text(format!("Step {} of {}", picks.index + 1, picks.steps.len()))
                                .font_size(11.)
                                .font_weight(FontWeight::MEDIUM)
                                .letter_spacing(1.6)
                                .color(SUB),
                        )
                        .child(rect().vertical().width(Size::fill()).spacing(10.).children(
                            picks.steps.iter().enumerate().map(|(position, step)| {
                                step_row(picks, position, *step, tags.len())
                            }),
                        ))
                        .maybe_child(show_tags.then(|| {
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
