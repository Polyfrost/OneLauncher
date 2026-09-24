use std::path::PathBuf;

use freya::prelude::*;
use oneclient_common::parse_mc_version;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Button, CARD_BG, CARD_GRID_H, CARD_H, CARD_NAME, CardLayout, GRID_GAP, GRID_MIN_W, Icon,
    IconType, LocalImage, OverlayPopup, ScrollArea, Segment, SegmentedControl, TextInput, badge,
    icon_box, kebab_button, meta_size, meta_text, on_secondary, open_folder_button,
};
use crate::theme::colors;
use crate::ui::border_all_color;

use super::package_manager::{CARD_SPACING, GRID_MAX_COLS, notice_bar};

const CONTROL_H: f32 = 34.;
const SEARCH_W: f32 = 180.;
const IMAGE_EDGE: u32 = 128;
const FIRST_DATAPACK_MAJOR: u32 = 13;

#[derive(Clone, Copy)]
pub(super) struct RowHeights {
    pub list: f32,
    pub grid: f32,
}

pub(super) const PACKAGE_ROWS: RowHeights = RowHeights {
    list: CARD_H,
    grid: CARD_GRID_H,
};

pub(crate) fn supports_datapacks(mc_version: &str) -> bool {
    parse_mc_version(mc_version).is_none_or(|p| p.major >= FIRST_DATAPACK_MAJOR)
}

pub(super) fn matches_search(needle: &str, fields: &[&str]) -> bool {
    needle.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(needle))
}

pub(super) fn toolbar_panel(leading: Option<Element>, controls: Vec<Element>) -> Element {
    let mut top_corners = CornerRadius::new_all(0.);
    top_corners.fill_top(12.);

    rect()
        .vertical()
        .width(Size::fill())
        .overflow(Overflow::Clip)
        .padding(Gaps::new_symmetric(8., 12.))
        .corner_radius(top_corners)
        .background(colors::page_elevated())
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(8.)
                .content(Content::Flex)
                .child(
                    rect()
                        .horizontal()
                        .width(Size::flex(1.0))
                        .height(Size::px(CONTROL_H))
                        .cross_align(Alignment::Center)
                        .maybe_child(leading),
                )
                .children(controls),
        )
        .into_element()
}

pub(super) fn search_input(search: State<String>) -> Element {
    TextInput::new(search)
        .placeholder("Search...")
        .width(Size::px(SEARCH_W))
        .leading(
            Icon::new(IconType::SearchMd)
                .size(14.)
                .color(colors::fg_secondary())
                .into_element(),
        )
        .into_element()
}

pub(super) fn folder_button(folder: PathBuf) -> Element {
    open_folder_button(folder)
        .width(Size::px(CONTROL_H))
        .height(Size::px(CONTROL_H))
        .into_element()
}

pub(super) fn layout_toggle(layout: State<ViewLayout>) -> Element {
    SegmentedControl::new(layout)
        .height(CONTROL_H)
        .icon_size(15.)
        .equal_width(CONTROL_H)
        .segment(Segment::new(ViewLayout::List).icon(IconType::ParagraphWrap))
        .segment(Segment::new(ViewLayout::Grid).icon(IconType::DotsGrid))
        .into_element()
}

pub(super) fn toolbar_action(icon: IconType, text: &str) -> Button {
    Button::new()
        .primary()
        .height(Size::px(CONTROL_H))
        .font_size(12.)
        .child(Icon::new(icon).size(15.))
        .text(text.to_string())
}

pub(super) fn content_box(
    count: usize,
    layout: CardLayout,
    heights: RowHeights,
    row: impl Fn(usize) -> Element + 'static,
    empty: Option<Element>,
    notices: Vec<String>,
) -> Element {
    let scroll = (count > 0).then(|| {
        let area = ScrollArea::new()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .scrollbar_gutter(true);
        match layout {
            CardLayout::List => area.lazy(count, heights.list, CARD_SPACING, row),
            CardLayout::Grid => area.lazy_grid(
                count,
                heights.grid,
                GRID_GAP,
                GRID_MIN_W,
                GRID_MAX_COLS,
                row,
            ),
        }
        .into_element()
    });

    let mut bottom_corners = CornerRadius::new_all(0.);
    bottom_corners.fill_bottom(12.);

    let empty = if count == 0 { empty } else { None };

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .padding(Gaps::new(0., 12., 12., 12.))
        .corner_radius(bottom_corners)
        .background(colors::page_elevated())
        .overflow(Overflow::Clip)
        .content(Content::Flex)
        .children(notices.into_iter().map(notice_bar))
        .maybe_child(scroll)
        .maybe_child(empty.map(|empty| {
            rect()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .child(empty)
                .into_element()
        }))
        .into_element()
}

#[derive(Clone, PartialEq)]
pub(super) enum CardIcon {
    Image(PathBuf),
    Symbol(IconType),
}

#[derive(PartialEq)]
pub(super) struct FolderCard {
    pub icon: CardIcon,
    pub title: String,
    pub badge: (IconType, String),
    pub subtitle: String,
    pub description: Option<String>,
    pub size: u64,
    pub layout: CardLayout,
    pub on_context: EventHandler<(f32, f32)>,
}

impl Component for FolderCard {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);

        let icon = card_icon(
            &self.icon,
            match self.layout {
                CardLayout::List => 44.,
                CardLayout::Grid => 40.,
            },
        );
        let on_context = Some(self.on_context.clone());

        match self.layout {
            CardLayout::List => rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::px(CARD_H))
                .cross_align(Alignment::Center)
                .spacing(12.)
                .padding(Gaps::new_all(10.))
                .corner_radius(CornerRadius::new_all(8.))
                .background(CARD_BG)
                .content(Content::Flex)
                .on_secondary_down(on_secondary(on_context.clone()))
                .child(self.list_info(icon))
                .child(meta_size(self.size))
                .child(kebab_button(self.on_context.clone()))
                .into_element(),
            CardLayout::Grid => {
                let hovering = *hovered.read();
                let muted = CARD_NAME.with_a(127);

                let header = rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(11.)
                    .content(Content::Flex)
                    .child(icon)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(3.)
                            .child(
                                label()
                                    .text(self.title.clone())
                                    .font_size(14.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .width(Size::fill())
                                    .color(Color::WHITE),
                            )
                            .child(meta_text(self.grid_meta(), muted)),
                    )
                    .child(kebab_button(self.on_context.clone()));

                let description = self.description.clone().map(|text| {
                    label()
                        .text(text)
                        .font_size(11.)
                        .line_height(1.45)
                        .max_lines(2)
                        .text_overflow(TextOverflow::Ellipsis)
                        .width(Size::fill())
                        .color(colors::fg_primary().with_a(183))
                        .into_element()
                });

                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::fill())
                    .spacing(9.)
                    .padding(Gaps::new_all(14.))
                    .corner_radius(CornerRadius::new_all(6.))
                    .background(if hovering {
                        colors::component_bg_hover()
                    } else {
                        colors::component_bg()
                    })
                    .border(border_all_color(
                        1.,
                        if hovering {
                            colors::component_border_hover()
                        } else {
                            colors::component_border()
                        },
                    ))
                    .overflow(Overflow::Clip)
                    .content(Content::Flex)
                    .on_pointer_enter(move |_| hovered.set(true))
                    .on_pointer_leave(move |_| hovered.set(false))
                    .on_secondary_down(on_secondary(on_context))
                    .child(header)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .maybe_child(description),
                    )
                    .into_element()
            }
        }
    }
}

impl FolderCard {
    fn grid_meta(&self) -> String {
        let mut parts = vec![self.subtitle.clone(), self.badge.1.clone()];
        if self.size > 0 {
            parts.push(crate::utils::format_size(self.size));
        }
        parts.join(" \u{b7} ")
    }

    fn list_info(&self, icon: Element) -> Element {
        rect()
            .horizontal()
            .width(Size::flex(1.0))
            .cross_align(Alignment::Center)
            .spacing(12.)
            .content(Content::Flex)
            .child(icon)
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .cross_align(Alignment::Center)
                            .spacing(8.)
                            .child(
                                label()
                                    .text(self.title.clone())
                                    .font_size(15.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .max_width(Size::percent(60.))
                                    .color(CARD_NAME),
                            )
                            .child(badge(
                                Icon::new(self.badge.0)
                                    .size(12.)
                                    .color(colors::fg_secondary())
                                    .into_element(),
                                self.badge.1.clone(),
                            )),
                    )
                    .child(
                        label()
                            .text(self.subtitle.clone())
                            .font_size(10.)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
                    )
                    .maybe_child(self.description.clone().map(|text| {
                        label()
                            .text(text)
                            .font_size(11.)
                            .max_lines(2)
                            .text_overflow(TextOverflow::Ellipsis)
                            .width(Size::fill())
                            .color(colors::fg_secondary())
                            .into_element()
                    })),
            )
            .into_element()
    }
}

pub(super) fn card_icon(icon: &CardIcon, size: f32) -> Element {
    match icon {
        CardIcon::Image(path) => rect()
            .width(Size::px(size))
            .height(Size::px(size))
            .corner_radius(CornerRadius::new_all(8.))
            .overflow(Overflow::Clip)
            .background(colors::component_bg())
            .child(LocalImage::new(path.clone(), IMAGE_EDGE, true).skeleton(true))
            .into_element(),
        CardIcon::Symbol(symbol) => icon_box(*symbol, size),
    }
}

pub(super) fn confirm_dialog(
    title: String,
    body: String,
    on_cancel: impl FnMut() + Clone + 'static,
    mut on_confirm: impl FnMut() + 'static,
) -> Element {
    let mut close = on_cancel.clone();
    let mut cancel = on_cancel;

    OverlayPopup::new()
        .on_close(move |_| close())
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .vertical()
                        .width(Size::px(400.))
                        .max_width(Size::window_percent(90.))
                        .spacing(14.)
                        .padding(Gaps::new_all(20.))
                        .corner_radius(CornerRadius::new_all(14.))
                        .background(CARD_BG)
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            label()
                                .text(title)
                                .font_size(16.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(body)
                                .font_size(12.)
                                .color(colors::fg_secondary()),
                        )
                        .child(
                            rect()
                                .horizontal()
                                .width(Size::fill())
                                .main_align(Alignment::End)
                                .spacing(8.)
                                .child(
                                    Button::new()
                                        .secondary()
                                        .on_press(move |_| cancel())
                                        .text("Cancel"),
                                )
                                .child(
                                    Button::new()
                                        .danger()
                                        .on_press(move |_| on_confirm())
                                        .child(Icon::new(IconType::Trash01).size(14.))
                                        .text("Delete"),
                                ),
                        ),
                ),
        )
        .into_element()
}
