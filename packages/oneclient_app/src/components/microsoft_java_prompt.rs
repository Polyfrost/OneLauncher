use std::time::Duration;

use freya::prelude::*;
use oneclient_events::Answer;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::{use_dispatch, use_notifications_snapshot};
use crate::microsoft_java::{MICROSOFT_JAVA_CHOICE_INSTALL, MICROSOFT_JAVA_CHOICE_NEVER};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);

const HOLD_SECONDS: u8 = 3;

#[derive(PartialEq)]
pub struct MicrosoftJavaPromptOverlay;

impl Component for MicrosoftJavaPromptOverlay {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();
        let remaining = use_state(|| HOLD_SECONDS);

        let showing = snapshot
            .pending_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.has_choice(MICROSOFT_JAVA_CHOICE_INSTALL));

        use_side_effect_with_deps(&showing, move |&showing| {
            let mut remaining = remaining;

            if !showing {
                remaining.set(HOLD_SECONDS);
                return;
            }

            spawn(async move {
                let mut remaining = remaining;
                for step in (0..HOLD_SECONDS).rev() {
                    tokio::time::sleep(Duration::from_secs(1)).await;

                    // Re-armed while this was sleeping so a newer prompt owns the
                    // hold now and this countdown is stale
                    if { *remaining.peek() } != step + 1 {
                        return;
                    }

                    remaining.set(step);
                }
            });
        });

        let Some(prompt) = snapshot.pending_prompt.clone() else {
            return rect().into_element();
        };

        if !prompt.has_choice(MICROSOFT_JAVA_CHOICE_INSTALL) {
            return rect().into_element();
        }

        let held = *remaining.read();
        let locked = held > 0;

        let dismiss_label = prompt
            .dismiss
            .clone()
            .unwrap_or_else(|| "Cancel".to_string());
        let never_label = prompt
            .choice(MICROSOFT_JAVA_CHOICE_NEVER)
            .map(|choice| choice.label.clone())
            .unwrap_or_else(|| "Don't ask again".to_string());
        let install_label = prompt
            .choice(MICROSOFT_JAVA_CHOICE_INSTALL)
            .map(|choice| choice.label.clone())
            .unwrap_or_else(|| "Proceed".to_string());

        let close = dispatch.clone();
        let cancel = dispatch.clone();
        let never = dispatch.clone();
        let accept = dispatch.clone();

        OverlayPopup::new()
            // Closing from the backdrop is a dismissal too so it waits as well
            .on_close(move |_| {
                if !locked {
                    close.dismiss_prompt();
                }
            })
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
                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .spacing(10.)
                                    .child(
                                        Icon::new(IconType::DownloadCloud02)
                                            .size(20.)
                                            .color(colors::brand()),
                                    )
                                    .child(
                                        label()
                                            .text(prompt.title.clone())
                                            .font_size(16.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_primary()),
                                    ),
                            )
                            .child(
                                label()
                                    .text(prompt.question.clone())
                                    .font_size(12.)
                                    .max_lines(6)
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .cross_align(Alignment::Center)
                                    .main_align(Alignment::SpaceBetween)
                                    .child(
                                        label()
                                            .text(if locked {
                                                held.to_string()
                                            } else {
                                                String::new()
                                            })
                                            .font_size(12.)
                                            .color(colors::fg_secondary()),
                                    )
                                    .child(
                                        rect()
                                            .horizontal()
                                            .spacing(8.)
                                            .child(
                                                Button::new()
                                                    .secondary()
                                                    .disabled(locked)
                                                    .on_press(move |_| cancel.dismiss_prompt())
                                                    .text(dismiss_label),
                                            )
                                            .child(
                                                Button::new()
                                                    .secondary()
                                                    .disabled(locked)
                                                    .on_press(move |_| {
                                                        never.answer_prompt(Answer::new(
                                                            MICROSOFT_JAVA_CHOICE_NEVER,
                                                        ))
                                                    })
                                                    .text(never_label),
                                            )
                                            .child(
                                                Button::new()
                                                    .primary()
                                                    .on_press(move |_| {
                                                        accept.answer_prompt(Answer::new(
                                                            MICROSOFT_JAVA_CHOICE_INSTALL,
                                                        ))
                                                    })
                                                    .child(
                                                        Icon::new(IconType::DownloadCloud02)
                                                            .size(14.),
                                                    )
                                                    .text(install_label),
                                            ),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
