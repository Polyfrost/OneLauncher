use freya::prelude::*;

use super::model::{LoaderChoice, LoaderMark, VersionRow};
use crate::AppAssets;
use crate::components::{AssetImage, Icon, IconType, filled_pill, pill};
use crate::theme::colors;
use crate::ui::border_all_color;

pub const CARD_RADIUS: f32 = 12.;
pub const VERSION_ROW_H: f32 = 48.;
pub const VERSION_ROW_SPACING: f32 = 4.;
pub const LOADER_MARK_SIZE: f32 = 22.;
pub const LOADER_ROW_MARK_SIZE: f32 = 34.;
pub const BUNDLE_COLUMNS: usize = 3;
pub const BUNDLE_CARD_H: f32 = 132.;
pub const LOADER_COLUMNS: usize = 2;
pub const LOADER_CARD_H: f32 = 124.;

const MARKER_SIZE: f32 = 18.;
const MARKER_MARK: f32 = 12.;
const MARKER_DOT: f32 = 7.;
const MARKER_RADIUS: f32 = 5.;

#[derive(PartialEq)]
pub struct SelectCard {
    pub id: String,
    pub selected: bool,
    pub height: Option<f32>,
    pub padding: Gaps,
    pub content: Element,
    pub on_press: EventHandler<()>,
}

impl Component for SelectCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let hovered = *hovering.read();
        let on_press = self.on_press.clone();

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let ring = if self.selected || focused {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        rect()
            .key(self.id.clone())
            .width(Size::fill())
            .height(self.height.map_or_else(Size::auto, Size::px))
            .padding(self.padding)
            .corner_radius(CornerRadius::new_all(CARD_RADIUS))
            .overflow(Overflow::Clip)
            .background(if hovered || focused {
                colors::component_bg_hover()
            } else {
                colors::component_bg()
            })
            .border(border_all_color(1., ring))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| on_press.call(()))
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .child(self.content.clone())
    }
}

pub fn loader_mark(choice: LoaderChoice, selected: bool, size: f32) -> Option<Element> {
    let mark = choice.mark()?;

    let drawn = match mark {
        LoaderMark::Tinted(icon) => {
            AppAssets::get_bytes(icon.path()).map(|_| Icon::new(icon).size(size).into_element())
        }
        LoaderMark::Image(path) => {
            AppAssets::get_bytes(path).map(|_| AssetImage::new(path, size).into_element())
        }
    };

    if let Some(drawn) = drawn {
        return Some(
            rect()
                .width(Size::px(size))
                .height(Size::px(size))
                .corner_radius(CornerRadius::new_all(5.))
                .overflow(Overflow::Clip)
                .opacity(if selected { 1.0 } else { 0.7 })
                .child(drawn)
                .into_element(),
        );
    }

    let color = if selected {
        colors::fg_primary()
    } else {
        colors::fg_secondary()
    };

    Some(
        rect()
            .width(Size::px(size))
            .height(Size::px(size))
            .center()
            .corner_radius(CornerRadius::new_all(5.))
            .border(border_all_color(1., colors::component_border()))
            .child(
                label()
                    .text(choice.short())
                    .font_size(9.)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .letter_spacing(0.4)
                    .color(color),
            )
            .into_element(),
    )
}

pub fn marker(selected: bool, checkbox: bool) -> Element {
    let radius = if checkbox { MARKER_RADIUS } else { 999. };

    rect()
        .width(Size::px(MARKER_SIZE))
        .height(Size::px(MARKER_SIZE))
        .center()
        .corner_radius(CornerRadius::new_all(radius))
        .background(if selected {
            colors::brand()
        } else {
            colors::page_elevated()
        })
        .border(border_all_color(
            1.,
            if selected {
                colors::brand()
            } else {
                colors::component_border()
            },
        ))
        .maybe_child(selected.then(|| {
            if checkbox {
                Icon::new(IconType::Check)
                    .size(MARKER_MARK)
                    .color(Color::WHITE)
                    .into_element()
            } else {
                rect()
                    .width(Size::px(MARKER_DOT))
                    .height(Size::px(MARKER_DOT))
                    .corner_radius(CornerRadius::new_all(999.))
                    .background(Color::WHITE)
                    .into_element()
            }
        }))
        .into_element()
}

pub struct WideCard {
    pub icon: IconType,
    pub title: String,
    pub badge: Option<String>,
    pub blurb: String,
    pub meta: String,
    pub selected: bool,
    pub on_press: EventHandler<()>,
}

pub fn wide_card(card: WideCard) -> Element {
    let WideCard {
        icon,
        title,
        badge,
        blurb,
        meta,
        selected,
        on_press,
    } = card;
    let title_id = title.clone();

    let content = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Start)
        .spacing(16.)
        .child(
            rect()
                .width(Size::px(44.))
                .height(Size::px(44.))
                .center()
                .corner_radius(CornerRadius::new_all(10.))
                .background(if selected {
                    colors::brand()
                } else {
                    colors::page_elevated()
                })
                .border(border_all_color(1., colors::component_border()))
                .child(Icon::new(icon).size(22.).color(if selected {
                    Color::WHITE
                } else {
                    colors::fg_primary()
                })),
        )
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(7.)
                .child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(10.)
                        .child(
                            label()
                                .text(title)
                                .font_size(15.)
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_primary()),
                        )
                        .maybe_child(
                            badge.map(|badge| filled_pill(badge, colors::brand(), Color::WHITE)),
                        ),
                )
                .child(
                    label()
                        .text(blurb)
                        .font_size(12.)
                        .line_height(1.45)
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(meta)
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(marker(selected, false))
        .into_element();

    SelectCard {
        id: title_id,
        selected,
        height: None,
        padding: Gaps::new_all(16.),
        content,
        on_press,
    }
    .into_element()
}

pub struct CellCard {
    pub id: String,
    pub title: String,
    pub blurb: String,
    pub meta: Option<String>,
    pub corner: Option<Element>,
    pub selected: bool,
    pub checkbox: bool,
    pub on_press: EventHandler<()>,
}

pub fn cell_card(card: CellCard) -> Element {
    let CellCard {
        id,
        title,
        blurb,
        meta,
        corner,
        selected,
        checkbox,
        on_press,
    } = card;

    let content = rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .main_align(Alignment::SpaceBetween)
        .spacing(8.)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(8.)
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .spacing(10.)
                        .child(marker(selected, checkbox))
                        .child(
                            label()
                                .text(title)
                                .width(Size::flex(1.0))
                                .font_size(14.)
                                .font_weight(FontWeight::MEDIUM)
                                .max_lines(1)
                                .color(colors::fg_primary()),
                        )
                        .maybe_child(corner),
                )
                .child(
                    label()
                        .text(blurb)
                        .width(Size::fill())
                        .font_size(12.)
                        .line_height(1.4)
                        .max_lines(3)
                        .color(colors::fg_secondary()),
                ),
        )
        .maybe_child(meta.map(|meta| {
            label()
                .text(meta)
                .font_size(11.)
                .max_lines(1)
                .color(colors::fg_secondary())
                .into_element()
        }))
        .into_element();

    SelectCard {
        id,
        selected,
        height: None,
        padding: Gaps::new_all(14.),
        content,
        on_press,
    }
    .into_element()
}

pub fn version_row(row: &VersionRow, selected: bool, on_press: EventHandler<()>) -> Element {
    let content = rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(14.)
        .child(marker(selected, false))
        .child(
            label()
                .text(row.id.clone())
                .width(Size::px(110.))
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .max_lines(1)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(row.meta.clone())
                .width(Size::flex(1.0))
                .font_size(12.)
                .max_lines(1)
                .text_align(TextAlign::Right)
                .color(colors::fg_secondary()),
        )
        .maybe_child(
            row.badge
                .as_ref()
                .map(|badge| pill(None, badge.clone(), colors::fg_secondary())),
        )
        .maybe_child((!row.date.is_empty()).then(|| {
            label()
                .text(row.date.clone())
                .width(Size::px(84.))
                .font_size(12.)
                .max_lines(1)
                .text_align(TextAlign::Right)
                .color(colors::fg_secondary())
                .into_element()
        }))
        .into_element();

    SelectCard {
        id: row.id.clone(),
        selected,
        height: Some(VERSION_ROW_H),
        padding: Gaps::new_symmetric(0., 14.),
        content,
        on_press,
    }
    .into_element()
}
