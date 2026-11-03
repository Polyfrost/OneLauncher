use freya::prelude::*;

use super::settings_page;
use crate::components::{Icon, IconType};
use crate::hooks::{Actions, use_dispatch, use_settings_snapshot};
use crate::i18n::{t, tf};
use crate::theme::colors;
use crate::translations;
use crate::view::app::settings::section_header;

#[derive(PartialEq)]
pub struct SettingsLanguage;

impl Component for SettingsLanguage {
    fn render(&self) -> impl IntoElement {
        let current = use_settings_snapshot().settings.language;
        let dispatch = use_dispatch();

        settings_page()
            .child(section_header(t(
                "oneclient.settings.language.section.available",
            )))
            .children(translations::choices().into_iter().map(|code| {
                let is_selected = code == current;
                language_row(code, is_selected, dispatch.clone()).into_element()
            }))
            .into_element()
    }
}

fn language_row(code: Option<String>, is_selected: bool, dispatch: Actions) -> impl IntoElement {
    let (native, english) = translations::names(code.as_deref());
    let coverage = translations::coverage(code.as_deref());

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_symmetric(12., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| {
            let code = code.clone();
            dispatch.edit_settings(|settings| settings.language = code);
        })
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
                .maybe_child((!english.is_empty()).then(|| {
                    label()
                        .text(english)
                        .font_size(12.)
                        .color(colors::fg_secondary())
                        .into_element()
                })),
        )
        .maybe_child(coverage.map(|percentage| {
            label()
                .text(tf(
                    "oneclient.onboarding.language.localized",
                    &[&percentage],
                ))
                .font_size(13.)
                .color(colors::fg_secondary())
                .into_element()
        }))
        .maybe_child(is_selected.then(|| {
            Icon::new(IconType::Check)
                .size(20.)
                .color(colors::brand())
                .into_element()
        }))
        .into_element()
}
