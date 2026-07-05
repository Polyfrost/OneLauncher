mod appearance;

mod apis;
mod changelog;
mod developer;
// mod feedback;
mod java;
mod language;
mod launcher;
mod minecraft;

use freya::prelude::*;

pub use apis::SettingsApis;
pub use changelog::SettingsChangelog;
pub use developer::SettingsDeveloper;
// pub use feedback::SettingsFeedback;
pub use java::SettingsJava;
pub use appearance::SettingsAppearance;
pub use language::SettingsLanguage;
pub use launcher::SettingsLauncher;
pub use minecraft::SettingsMinecraft;

use crate::{
    components::{IconType, Icon},
    theme::colors,
};

pub fn settings_page() -> Rect {
    rect().vertical().width(Size::fill()).spacing(4.)
}

pub fn section_header(text: &'static str) -> impl IntoElement {
    rect()
        .padding(Gaps::new(16., 0., 8., 2.))
        .child(
            label()
                .text(text)
                .font_size(13.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

pub fn settings_row(
    icon: IconType,
    title: &'static str,
    description: impl Into<String>,
    trailing: impl IntoElement,
) -> impl IntoElement {
    let description = description.into();
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_symmetric(12., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .child(Icon::new(icon))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title)
                        .font_size(16.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(description)
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(trailing)
        .into_element()
}
