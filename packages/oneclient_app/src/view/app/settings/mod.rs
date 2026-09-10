mod accounts;
mod appearance;

mod apis;
mod changelog;
mod developer;
mod java;
mod language;
mod launcher;
mod minecraft;
mod storage;

use freya::prelude::*;

pub use accounts::SettingsAccounts;
pub use apis::SettingsApis;
pub use changelog::SettingsChangelog;
pub use developer::SettingsDeveloper;
pub use appearance::SettingsAppearance;
pub use java::SettingsJava;
pub use language::SettingsLanguage;
pub use launcher::SettingsLauncher;
pub use minecraft::SettingsMinecraft;
pub use storage::SettingsStorage;

use crate::{
    components::{Button, Icon, IconType, OverlayPopup},
    hooks::ResetNotice,
    theme::colors,
    ui::border_all_color,
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
    settings_row_inner(icon, title, description, trailing, false)
}

pub fn settings_row_disabled(
    icon: IconType,
    title: &'static str,
    description: impl Into<String>,
    trailing: impl IntoElement,
) -> impl IntoElement {
    settings_row_inner(icon, title, description, trailing, true)
}

fn settings_row_inner(
    icon: IconType,
    title: &'static str,
    description: impl Into<String>,
    trailing: impl IntoElement,
    disabled: bool,
) -> impl IntoElement {
    let description = description.into();
    rect()
        .maybe(disabled, |el| el.opacity(0.4))
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

pub fn take_notice(mut announce: State<bool>, notice: ResetNotice) -> Option<ResetNotice> {
    if !*announce.peek() {
        return None;
    }
    announce.set(false);
    Some(notice)
}

const RESET_BLOCKED: &str =
    "Minecraft is running. Close the game before resetting these options.";

pub fn reset_row(
    mut confirming: State<bool>,
    blocked: bool,
    description: &'static str,
    summary: &'static str,
    on_reset: EventHandler<()>,
) -> Element {
    let row = if blocked {
        settings_row_disabled(
            IconType::RefreshCw01,
            "Reset to defaults",
            RESET_BLOCKED,
            Button::new().danger().small().disabled(true).text("Reset"),
        )
        .into_element()
    } else {
        settings_row(
            IconType::RefreshCw01,
            "Reset to defaults",
            description,
            Button::new()
                .danger()
                .small()
                .on_press(move |_| confirming.set(true))
                .text("Reset"),
        )
        .into_element()
    };

    let mut section = rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .child(section_header("RESET"))
        .child(row);

    if blocked {
        if *confirming.peek() {
            confirming.set(false);
        }
    } else if *confirming.read() {
        section = section.child(confirm_reset(confirming, summary, on_reset));
    }

    section.into_element()
}

fn confirm_reset(
    mut confirming: State<bool>,
    summary: &'static str,
    on_reset: EventHandler<()>,
) -> Element {
    let card = rect()
        .vertical()
        .width(Size::px(440.))
        .max_width(Size::window_percent(90.))
        .spacing(14.)
        .padding(Gaps::new_all(20.))
        .corner_radius(CornerRadius::new_all(14.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(Icon::new(IconType::RefreshCw01).size(20.))
                .child(
                    label()
                        .text("Reset to defaults?")
                        .font_size(16.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(
            label()
                .text(summary)
                .font_size(12.)
                .max_lines(8)
                .width(Size::fill())
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
                        .on_press(move |_| confirming.set(false))
                        .text("Cancel"),
                )
                .child(
                    Button::new()
                        .danger()
                        .on_press(move |_| {
                            on_reset.call(());
                            confirming.set(false);
                        })
                        .text("Reset"),
                ),
        );

    OverlayPopup::new()
        .on_close(move |_| confirming.set(false))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(card),
        )
        .into_element()
}
