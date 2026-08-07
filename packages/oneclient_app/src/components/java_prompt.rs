use freya::prelude::*;
use oneclient_java::{JAVA_CHOICE_DOWNLOAD, JAVA_CHOICE_FOLDER, JavaVendor};
use oneclient_events::Answer;

use crate::components::{Button, Icon, IconType, JavaInstallManager, OverlayPopup};
use crate::hooks::{use_dispatch, use_notifications_snapshot};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);

#[derive(PartialEq)]
pub struct JavaPromptOverlay;

impl Component for JavaPromptOverlay {
    fn render(&self) -> impl IntoElement {
        // Hooks must run unconditionally before any early return to keep the hook count stable
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();
        let mut show_manager = use_state(|| false);

        let Some(prompt) = snapshot.pending_prompt.clone() else {
            return rect().into_element();
        };

        // Identified by the choices it offers rather than a prompt-kind enum
        if !prompt.has_choice(JAVA_CHOICE_DOWNLOAD) {
            return rect().into_element();
        }

        // The prompt carries no major so recover it from the folder choice's dialog title
        let major = prompt
            .choice(JAVA_CHOICE_FOLDER)
            .and_then(|choice| match &choice.input {
                Some(oneclient_events::ChoiceInput::Folder { title }) => title
                    .split_whitespace()
                    .find_map(|word| word.parse::<u32>().ok()),
                _ => None,
            })
            .unwrap_or(0);

        if *show_manager.read() {
            let install = dispatch.clone();
            return JavaInstallManager::new()
                .suggested(Some(major))
                .on_install(move |(vendor, _row_major): (JavaVendor, u32)| {
                    install.answer_prompt(
                        Answer::new(JAVA_CHOICE_DOWNLOAD).with_selection(vendor.to_string()),
                    );
                    show_manager.set(false);
                })
                .on_close(move |()| show_manager.set(false))
                .into_element();
        }

        let close = dispatch.clone();
        let cancel = dispatch.clone();
        let folder = dispatch.clone();

        let pick_folder = move |_| {
            let dispatch = folder.clone();
            spawn(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_title(format!("Select a Java {major} installation folder"))
                    .pick_folder()
                    .await
                {
                    dispatch.answer_prompt(
                        Answer::new(JAVA_CHOICE_FOLDER).with_folder(handle.path()),
                    );
                }
            });
        };

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
                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .spacing(10.)
                                    .child(
                                        Icon::new(IconType::FolderDownload)
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
                                                cancel.dismiss_prompt()
                                            })
                                            .text("Cancel"),
                                    )
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .on_press(pick_folder)
                                            .child(Icon::new(IconType::Folder).size(14.))
                                            .text("Choose folder"),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(move |_| show_manager.set(true))
                                            .child(Icon::new(IconType::DownloadCloud02).size(14.))
                                            .text("Download"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
