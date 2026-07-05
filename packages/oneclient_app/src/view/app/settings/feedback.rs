use freya::prelude::*;

use crate::components::{Button, TextInput};
use crate::layout::{SettingsShell, SettingsTab};
use crate::theme::colors;

#[derive(PartialEq)]
pub struct SettingsFeedback;

impl Component for SettingsFeedback {
    fn render(&self) -> impl IntoElement {
        SettingsShell::new(SettingsTab::Feedback, "Feedback")
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(4.)
                    .padding(Gaps::new(0., 0., 8., 0.))
                    .child(
                        label()
                            .text("Help us improve OneClient")
                            .font_size(16.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text("Tell us what's working, what isn't, and what you'd like to see next.")
                            .font_size(12.)
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
                        Button::new().primary().text("Send Feedback"),
                    ),
            )
            .into_element()
    }
}

fn feedback_box() -> impl IntoElement {
    let feedback = use_state(String::new);

    TextInput::new(feedback)
        .width(Size::fill())
        .placeholder("Write your feedback here...")
        .height(Size::px(160.))
}
