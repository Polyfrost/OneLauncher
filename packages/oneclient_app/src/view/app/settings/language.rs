use freya::prelude::*;

use crate::components::{IconType, Icon};
use super::settings_page;
use crate::theme::colors;
use crate::view::app::settings::section_header;

const LANGUAGES: &[(&str, &str)] = &[
    ("English", "English (US)"),
];

#[derive(PartialEq)]
pub struct SettingsLanguage;

impl Component for SettingsLanguage {
    fn render(&self) -> impl IntoElement {
        let selected = use_state(|| 0usize);

        settings_page()
            .child(section_header("AVAILABLE LANGUAGES"))
            .children(LANGUAGES.iter().enumerate().map(|(i, (native, english))| {
                language_row(i, native, english, selected).into_element()
            }))
            .into_element()
    }
}

fn language_row(
    index: usize,
    native: &'static str,
    english: &'static str,
    mut selected: State<usize>,
) -> impl IntoElement {
    let is_selected = *selected.read() == index;

    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_symmetric(12., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(move |_| *selected.write() = index)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(native)
                        .font_size(16.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(english)
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
        .maybe_child(
            is_selected.then(|| Icon::new(IconType::Check).size(20.).color(colors::brand()).into_element()),
        )
        .into_element()
}
