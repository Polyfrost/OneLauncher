use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_core::CreateClusterOptions;
use oneclient_db::models::ClusterId;

use crate::components::{Button, Dropdown, OverlayPopup, Segment, SegmentedControl, TextInput};
use crate::hooks::{use_active_cluster_id, use_dispatch};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const CARD_WIDTH_PX: f32 = 460.;
const MAX_NAME_CHARS: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectoryMode {
    Shared,
    Dedicated,
}

impl DirectoryMode {
    fn is_dedicated(self) -> bool {
        matches!(self, Self::Dedicated)
    }

    fn description(self) -> &'static str {
        match self {
            Self::Shared => {
                "Mods, resource packs and shaders stay separate, but worlds, options and \
                 configs are shared with other instances of this version. Only one shared \
                 instance can run at a time."
            }
            Self::Dedicated => {
                "This instance gets its own game folder, so worlds, options and configs are \
                 kept apart from every other instance. Uses more disk space."
            }
        }
    }
}

#[derive(PartialEq)]
pub struct ClusterCreateDialog {
    targets: Vec<(String, GameLoader)>,
    source: Option<(ClusterId, String, GameLoader)>,
    on_close: EventHandler<()>,
}

impl ClusterCreateDialog {
    pub fn new(
        targets: Vec<(String, GameLoader)>,
        on_close: impl Into<EventHandler<()>>,
    ) -> Self {
        Self {
            targets,
            source: None,
            on_close: on_close.into(),
        }
    }

    pub fn duplicating(
        mut self,
        source_id: ClusterId,
        mc_version: impl Into<String>,
        mc_loader: GameLoader,
    ) -> Self {
        self.source = Some((source_id, mc_version.into(), mc_loader));
        self
    }
}

impl Component for ClusterCreateDialog {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let active = use_active_cluster_id();

        let name = use_state(String::new);
        let mode = use_state(|| DirectoryMode::Shared);
        let mut target_index = use_state(|| 0_usize);

        let trimmed = name.read().trim().to_string();
        let too_long = trimmed.chars().count() > MAX_NAME_CHARS;

        let duplicating = self.source.is_some();
        let title = if duplicating {
            "Duplicate instance"
        } else {
            "New instance"
        };

        let target = match &self.source {
            Some((_, version, loader)) => Some((version.clone(), *loader)),
            None => {
                let index = (*target_index.read()).min(self.targets.len().saturating_sub(1));
                self.targets.get(index).cloned()
            }
        };

        let can_submit = !trimmed.is_empty() && !too_long && target.is_some();

        let close_on_scrim = self.on_close.clone();
        let close_on_cancel = self.on_close.clone();
        let close_on_submit = self.on_close.clone();

        let source_id = self.source.as_ref().map(|(id, _, _)| *id);
        let submit_target = target.clone();

        let submit = move |_| {
            let trimmed = trimmed.clone();
            if trimmed.is_empty() || trimmed.chars().count() > MAX_NAME_CHARS {
                return;
            }

            let Some((mc_version, mc_loader)) = submit_target.clone() else {
                return;
            };

            let dedicated = mode.read().is_dedicated();

            match source_id {
                Some(source_id) => {
                    dispatch.duplicate_cluster(source_id, trimmed, dedicated, active)
                }
                None => {
                    let options = CreateClusterOptions::new(trimmed, mc_version, mc_loader)
                        .dedicated(dedicated);
                    dispatch.create_cluster(options, active);
                }
            }

            close_on_submit.call(());
        };

        let target_label = target
            .as_ref()
            .map(|(version, loader)| format!("{version} {loader}"))
            .unwrap_or_else(|| "No versions available".to_string());

        let target_options: Vec<String> = self
            .targets
            .iter()
            .map(|(version, loader)| format!("{version} {loader}"))
            .collect();

        OverlayPopup::new()
            .on_close(move |_| close_on_scrim.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(CARD_WIDTH_PX))
                            .max_width(Size::window_percent(90.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(CARD_BG)
                            .border(border_all_color(1., colors::component_border()))
                            .child(heading(title))
                            .child(field_label("Name"))
                            .child(
                                TextInput::new(name)
                                    .width(Size::fill())
                                    .placeholder("Skyblock, PvP, Modded survival..."),
                            )
                            .maybe_child(name_hint(too_long))
                            .child(field_label("Version"))
                            .child(if duplicating {
                                locked_target(&target_label)
                            } else {
                                Dropdown::new(target_label.clone(), target_options)
                                    .width(Size::fill())
                                    .height(Size::px(34.))
                                    .max_lines(1)
                                    .on_select(move |idx: usize| {
                                        *target_index.write() = idx;
                                    })
                                    .into_element()
                            })
                            .child(field_label("Game folder"))
                            .child(
                                SegmentedControl::new(mode)
                                    .fill()
                                    .segment(Segment::new(DirectoryMode::Shared).label("Shared"))
                                    .segment(
                                        Segment::new(DirectoryMode::Dedicated).label("Separate"),
                                    ),
                            )
                            .child(
                                label()
                                    .text(mode.read().description())
                                    .font_size(11.)
                                    .max_lines(4)
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
                            )
                            .maybe_child(duplicate_note(duplicating))
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .text("Cancel")
                                            .on_press(move |_| close_on_cancel.call(())),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .enabled(can_submit)
                                            .text(if duplicating { "Duplicate" } else { "Create" })
                                            .on_press(submit),
                                    ),
                            ),
                    ),
            )
    }
}

fn heading(title: &str) -> Element {
    label()
        .text(title.to_string())
        .font_size(16.)
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_primary())
        .into_element()
}

fn locked_target(text: &str) -> Element {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_symmetric(8., 12.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .child(
            label()
                .text(text.to_string())
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn field_label(text: &str) -> Element {
    label()
        .text(text.to_string())
        .font_size(12.)
        .font_weight(FontWeight::MEDIUM)
        .color(colors::fg_primary())
        .into_element()
}

fn name_hint(too_long: bool) -> Option<Element> {
    if !too_long {
        return None;
    }

    Some(
        label()
            .text(format!("Keep the name under {MAX_NAME_CHARS} characters."))
            .font_size(11.)
            .color(colors::danger())
            .into_element(),
    )
}

fn duplicate_note(duplicating: bool) -> Option<Element> {
    if !duplicating {
        return None;
    }

    Some(
        label()
            .text("The current mod list and bundle choices are copied into the new instance.")
            .font_size(11.)
            .max_lines(3)
            .width(Size::fill())
            .color(colors::fg_secondary())
            .into_element(),
    )
}
