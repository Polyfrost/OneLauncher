use freya::prelude::*;

use crate::components::{Icon, IconType};
use crate::theme::colors;
use crate::ui::border_all_color;

pub fn warning_card(title: impl Into<String>, body: impl Into<String>) -> Rect {
    tinted_card(colors::danger(), title.into(), body.into())
}

pub fn caution_card(title: impl Into<String>, body: impl Into<String>) -> Rect {
    tinted_card(colors::code_warn(), title.into(), body.into())
}

fn tinted_card(accent: Color, title: String, body: String) -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .padding(Gaps::new_all(12.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(accent.with_a(24))
        .border(border_all_color(1., accent.with_a(110)))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(Icon::new(IconType::AlertTriangle).size(16.).color(accent))
                .child(
                    label()
                        .text(title)
                        .width(Size::flex(1.))
                        .font_size(13.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(2)
                        .color(colors::fg_primary()),
                ),
        )
        .child(
            label()
                .text(body)
                .width(Size::fill())
                .font_size(12.)
                .max_lines(4)
                .color(colors::fg_secondary()),
        )
}

pub fn warning_list(rows: impl IntoIterator<Item = (String, Option<String>)>) -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .padding(Gaps::new_all(10.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .children(rows.into_iter().map(|(name, note)| list_row(name, note)))
}

fn list_row(name: String, note: Option<String>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .child(
            label()
                .text(name)
                .width(Size::flex(1.))
                .font_size(12.5)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(colors::fg_primary()),
        )
        .maybe_child(note.map(|note| {
            label()
                .text(note)
                .font_size(11.)
                .max_lines(1)
                .color(colors::fg_secondary())
                .into_element()
        }))
        .into_element()
}
