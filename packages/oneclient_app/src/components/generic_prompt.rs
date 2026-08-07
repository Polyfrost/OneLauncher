//! Fallback renderer for prompts no dedicated overlay claims
//! Without it an unrendered prompt leaves the core blocked on `ask()` blocking all later prompts

use freya::prelude::*;
use oneclient_events::{Answer, ChoiceInput, ChoiceStyle};
use oneclient_java::JAVA_CHOICE_DOWNLOAD;

use crate::components::{Button, OverlayPopup};
use crate::hooks::{use_dispatch, use_notifications_snapshot};
use crate::notifications::PendingPromptView;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::updater::UPDATE_CHOICE_INSTALL;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);

fn claimed_elsewhere(prompt: &PendingPromptView) -> bool {
    prompt.has_choice(JAVA_CHOICE_DOWNLOAD) || prompt.has_choice(UPDATE_CHOICE_INSTALL)
}

#[derive(PartialEq)]
pub struct GenericPromptOverlay;

impl Component for GenericPromptOverlay {
    fn render(&self) -> impl IntoElement {
        // Hooks run unconditionally before any early return to keep the hook count stable
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();

        let Some(prompt) = snapshot.pending_prompt.clone() else {
            return rect().into_element();
        };

        if claimed_elsewhere(&prompt) {
            return rect().into_element();
        }

        let close = dispatch.clone();

        let mut buttons = rect()
            .horizontal()
            .width(Size::fill())
            .main_align(Alignment::End)
            .spacing(8.);

        if let Some(dismiss) = prompt.dismiss.clone() {
            let cancel = dispatch.clone();
            buttons = buttons.child(
                Button::new()
                    .secondary()
                    .text(dismiss)
                    .on_press(move |_| cancel.dismiss_prompt()),
            );
        }

        for choice in &prompt.choices {
            let dispatch = dispatch.clone();
            let id = choice.id;
            let input = choice.input.clone();

            let button = Button::new().text(choice.label.clone());
            let button = match choice.style {
                ChoiceStyle::Primary => button.primary(),
                ChoiceStyle::Danger => button.danger(),
                ChoiceStyle::Secondary => button.secondary(),
            };

            buttons = buttons.child(button.on_press(move |_| match &input {
                // Answered from the dialog callback not the press since no folder is picked yet
                Some(ChoiceInput::Folder { title }) => {
                    let dispatch = dispatch.clone();
                    let title = title.clone();
                    spawn(async move {
                        if let Some(handle) = rfd::AsyncFileDialog::new()
                            .set_title(title)
                            .pick_folder()
                            .await
                        {
                            dispatch.answer_prompt(
                                Answer::new(id).with_folder(handle.path()),
                            );
                        }
                    });
                }
                // No picker here answering with `selection() == None` beats blocking the core
                Some(ChoiceInput::Selection { hint }) => {
                    tracing::warn!(
                        choice = id,
                        hint,
                        "a prompt asked for a selection with no overlay to collect it"
                    );
                    dispatch.answer_prompt(Answer::new(id));
                }
                None => dispatch.answer_prompt(Answer::new(id)),
            }));
        }

        OverlayPopup::new()
            .on_close(move |_| close.dismiss_prompt())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(440.))
                            .max_width(Size::window_percent(90.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(CARD_BG)
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text(prompt.title.clone())
                                    .font_size(16.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(prompt.question.clone())
                                    .font_size(12.)
                                    .max_lines(6)
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
                            )
                            .child(buttons),
                    ),
            )
            .into_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oneclient_events::Choice;

    fn prompt(choices: Vec<Choice>) -> PendingPromptView {
        PendingPromptView {
            title: "Title".to_string(),
            question: "Question".to_string(),
            choices,
            dismiss: Some("Cancel".to_string()),
        }
    }

    #[test]
    fn the_two_rich_overlays_keep_their_own_prompts() {
        assert!(claimed_elsewhere(&prompt(vec![Choice::primary(
            JAVA_CHOICE_DOWNLOAD,
            "Download",
        )])));
        assert!(claimed_elsewhere(&prompt(vec![Choice::primary(
            UPDATE_CHOICE_INSTALL,
            "Download",
        )])));
    }

    #[test]
    fn every_other_prompt_falls_through_to_the_generic_overlay() {
        for choices in [
            vec![Choice::new("launch", "Launch anyway")],
            vec![Choice::primary("verify", "Verify and repair")],
            vec![Choice::danger("delete", "Delete")],
            Vec::new(),
        ] {
            assert!(
                !claimed_elsewhere(&prompt(choices)),
                "an unclaimed prompt must render somewhere"
            );
        }
    }
}
