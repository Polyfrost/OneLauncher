use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea};
use crate::theme::colors;
use crate::ui::border_all_color;

pub const DIALOG_WIDTH: f32 = 1020.;
pub const DIALOG_HEIGHT: f32 = 660.;
pub const RAIL_WIDTH: f32 = 368.;
pub const PANE_PADDING: f32 = 24.;

pub struct Shell {
    pub rail: Element,
    pub eyebrow: String,
    pub title: String,
    pub subtitle: String,
    pub body: Element,
    pub scrolls_itself: bool,
    pub note: String,
    pub secondary_label: String,
    pub primary_label: String,
    pub primary_enabled: bool,
    pub on_close: EventHandler<()>,
    pub on_secondary: EventHandler<()>,
    pub on_primary: EventHandler<()>,
}

pub fn shell(parts: Shell) -> Element {
    let Shell {
        rail,
        eyebrow,
        title,
        subtitle,
        body,
        scrolls_itself,
        note,
        secondary_label,
        primary_label,
        primary_enabled,
        on_close,
        on_secondary,
        on_primary,
    } = parts;

    let close_scrim = on_close.clone();

    OverlayPopup::new()
        .on_close(move |()| close_scrim.call(()))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .horizontal()
                        .width(Size::px(DIALOG_WIDTH))
                        .height(Size::px(DIALOG_HEIGHT))
                        .max_width(Size::window_percent(94.))
                        .max_height(Size::window_percent(92.))
                        .content(Content::Flex)
                        .corner_radius(CornerRadius::new_all(16.))
                        .overflow(Overflow::Clip)
                        .background(colors::page_elevated())
                        .border(border_all_color(1., colors::component_border()))
                        .child(rail)
                        .child(
                            rect()
                                .vertical()
                                .width(Size::flex(1.0))
                                .height(Size::fill())
                                .content(Content::Flex)
                                .child(header(eyebrow, title, subtitle, on_close))
                                .child(if scrolls_itself {
                                    rect()
                                        .width(Size::fill())
                                        .height(Size::flex(1.0))
                                        .padding(Gaps::new(0., PANE_PADDING, 8., PANE_PADDING))
                                        .child(body)
                                        .into_element()
                                } else {
                                    ScrollArea::new()
                                        .width(Size::fill())
                                        .height(Size::flex(1.0))
                                        .padding(Gaps::new(0., PANE_PADDING, 8., PANE_PADDING))
                                        .scrollbar_gutter(true)
                                        .child(body)
                                        .into_element()
                                })
                                .child(footer(
                                    note,
                                    secondary_label,
                                    primary_label,
                                    primary_enabled,
                                    on_secondary,
                                    on_primary,
                                )),
                        ),
                ),
        )
        .into_element()
}

fn header(eyebrow: String, title: String, subtitle: String, on_close: EventHandler<()>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Start)
        .spacing(16.)
        .padding(Gaps::new(22., PANE_PADDING, 16., PANE_PADDING))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(5.)
                .child(
                    label()
                        .text(eyebrow)
                        .font_size(11.)
                        .font_weight(FontWeight::MEDIUM)
                        .letter_spacing(1.6)
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(title)
                        .font_size(20.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(13.)
                        .line_height(1.35)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Button::new()
                .ghost()
                .icon()
                .alt("Close")
                .on_press(move |_| on_close.call(()))
                .child(Icon::new(IconType::XClose).size(16.)),
        )
        .into_element()
}

fn footer(
    note: String,
    secondary_label: String,
    primary_label: String,
    primary_enabled: bool,
    on_secondary: EventHandler<()>,
    on_primary: EventHandler<()>,
) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(20.)
        .padding(Gaps::new(16., PANE_PADDING, 16., PANE_PADDING))
        .border(
            Border::new()
                .fill(colors::component_border())
                .width(BorderWidth {
                    top: 1.,
                    right: 0.,
                    bottom: 0.,
                    left: 0.,
                }),
        )
        .child(
            label()
                .text(note)
                .width(Size::flex(1.0))
                .font_size(12.)
                .line_height(1.35)
                .max_lines(2)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .horizontal()
                .spacing(10.)
                .cross_align(Alignment::Center)
                .child(
                    Button::new()
                        .ghost()
                        .on_press(move |_| on_secondary.call(()))
                        .text(secondary_label),
                )
                .child(
                    Button::new()
                        .primary()
                        .enabled(primary_enabled)
                        .on_press(move |_| {
                            if primary_enabled {
                                on_primary.call(());
                            }
                        })
                        .text(primary_label),
                ),
        )
        .into_element()
}
