use freya::prelude::*;
use oneclient_core::notification::{PromptKind, UserChoice};

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::{use_dispatch, use_notifications_snapshot};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);

#[derive(PartialEq)]
pub struct UpdatePromptOverlay;

impl Component for UpdatePromptOverlay {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();

        let Some(prompt) = snapshot.pending_prompt.clone() else {
            return rect().into_element();
        };

        if prompt.kind != PromptKind::Update {
            return rect().into_element();
        }

        let close = dispatch.clone();
        let cancel = dispatch.clone();
        let accept = dispatch.clone();

        OverlayPopup::new()
            .on_close(move |_| close.answer_prompt(UserChoice::Cancel))
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
                                    .max_lines(4)
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
                                            .on_press(move |_| {
                                                cancel.answer_prompt(UserChoice::Cancel)
                                            })
                                            .text("Not now"),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(move |_| {
                                                accept.answer_prompt(UserChoice::Accept)
                                            })
                                            .child(Icon::new(IconType::DownloadCloud02).size(14.))
                                            .text("Download"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
