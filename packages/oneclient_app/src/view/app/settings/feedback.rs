use freya::prelude::*;

use crate::components::{Button, TextInput};
use crate::i18n::t;
use crate::layout::{SettingsShell, SettingsTab};
use crate::theme::{colors, text_size};

#[derive(PartialEq)]
pub struct SettingsFeedback;

impl Component for SettingsFeedback {
    fn render(&self) -> impl IntoElement {
        SettingsShell::new(
            SettingsTab::Feedback,
            t("oneclient.routes.settings_feedback"),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(4.)
                .padding(Gaps::new(0., 0., 8., 0.))
                .child(
                    label()
                        .text(t("oneclient.settings.feedback.title"))
                        .font_size(text_size(16.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(t("oneclient.settings.feedback.description"))
                        .font_size(text_size(12.))
                        .color(colors::fg_primary()),
                ),
        )
        .child(feedback_box())
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::End)
                .padding(Gaps::new(12., 0., 0., 0.))
                .child(
                    Button::new()
                        .primary()
                        .text(t("oneclient.settings.feedback.send")),
                ),
        )
        .into_element()
    }
}

fn feedback_box() -> impl IntoElement {
    let feedback = use_state(String::new);

    TextInput::new(feedback)
        .width(Size::fill())
        .placeholder(t("oneclient.settings.feedback.placeholder"))
        .height(Size::px(160.))
}
