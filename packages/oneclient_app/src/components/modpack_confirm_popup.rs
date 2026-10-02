use std::collections::HashMap;

use freya::prelude::*;
use oneclient_content::modpacks::ModpackSummary;
use oneclient_content::packages::ResolvedAlternative;
use oneclient_core::clusters::{FlaggedModpack, FlaggedPackMod};

use super::flagged_install_popup::explanation_panel;
use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea, remote_icon};
use crate::hooks::{use_cached_image, use_dispatch, use_notifications_snapshot};
use crate::notifications::{FlaggedChoice, ModpackConfirm, ModpackImportView};
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::format_size;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 500.;
const DIALOG_PAD: f32 = 22.;
const FACT_LABEL_W: f32 = 110.;
const SECTION_GAP: f32 = 16.;
const WINDOW_SHARE: f32 = 0.85;
const MIN_BODY_H: f32 = 120.;
const EDGE_SLACK: f32 = 4.;
const ALT_ROW_H: f32 = 44.;
const ALT_ICON: f32 = 28.;

type Choices = State<HashMap<String, FlaggedChoice>>;

#[derive(PartialEq)]
pub struct ModpackConfirmPopup;

impl Component for ModpackConfirmPopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();

        let Some(confirm) = snapshot.modpack_confirm else {
            return rect().into_element();
        };

        let close = dispatch.clone();
        OverlayPopup::new()
            .on_close(move |_| close.cancel_modpack())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(ConfirmDialog { confirm }),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct ConfirmDialog {
    confirm: ModpackConfirm,
}

impl Component for ConfirmDialog {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let choices: Choices = use_state(HashMap::new);
        let header_h = use_state(|| 0f32);
        let footer_h = use_state(|| 0f32);
        let content_h = use_state(|| 0f32);

        let platform = Platform::get();
        let scale = (*platform.scale_factor.read() as f32).max(1.);
        let window_h = platform.root_size.read().height / scale;
        let room = window_h * WINDOW_SHARE
            - 2. * DIALOG_PAD
            - 2. * SECTION_GAP
            - *header_h.read()
            - *footer_h.read()
            - EDGE_SLACK;
        let body_h = content_h.read().min(room.max(MIN_BODY_H));

        dialog(
            &self.confirm,
            choices,
            dispatch,
            Measured {
                header_h,
                footer_h,
                content_h,
                body_h,
            },
        )
    }
}

struct Measured {
    header_h: State<f32>,
    footer_h: State<f32>,
    content_h: State<f32>,
    body_h: f32,
}

fn track_height(mut target: State<f32>) -> impl FnMut(Event<SizedEventData>) {
    move |e: Event<SizedEventData>| {
        let h = e.area.height();
        if (*target.peek() - h).abs() > 0.5 {
            target.set(h);
        }
    }
}

fn count_label(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn content_line(summary: &ModpackSummary) -> String {
    let parts: Vec<String> = [
        (summary.mods, "mod", "mods"),
        (summary.resource_packs, "resource pack", "resource packs"),
        (summary.shaders, "shader", "shaders"),
        (summary.other_files, "other file", "other files"),
    ]
    .into_iter()
    .filter(|(count, _, _)| *count > 0)
    .map(|(count, one, many)| count_label(count, one, many))
    .collect();

    if parts.is_empty() {
        "Configs and settings only".to_string()
    } else {
        parts.join(", ")
    }
}

fn notes(summary: &ModpackSummary) -> Vec<(String, Color)> {
    let mut notes = Vec::new();
    if summary.optional > 0 {
        notes.push((
            format!(
                "{} optional. They are installed turned off, so you can switch them on in the Mods tab.",
                count_label(summary.optional, "file is", "files are")
            ),
            colors::fg_secondary(),
        ));
    }
    if summary.manual > 0 {
        notes.push((
            format!(
                "{} must be downloaded by hand from CurseForge. The launcher will show you which ones.",
                count_label(summary.manual, "file", "files")
            ),
            colors::code_warn(),
        ));
    }
    if summary.unresolved > 0 {
        notes.push((
            format!(
                "{} could not be found on Modrinth or CurseForge and will be skipped.",
                count_label(summary.unresolved, "file", "files")
            ),
            colors::danger(),
        ));
    }
    notes
}

fn fact(name: &'static str, value: String) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .spacing(10.)
        .child(
            label()
                .text(name)
                .width(Size::px(FACT_LABEL_W))
                .font_size(12.5)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(value)
                .width(Size::flex(1.))
                .font_size(12.5)
                .max_lines(2)
                .color(colors::fg_primary()),
        )
        .into_element()
}

fn intro(confirm: &ModpackConfirm) -> (String, String) {
    match &confirm.import {
        None => (
            format!("Add {}?", confirm.pack_name),
            format!(
                "From {}. It is added as a new instance with its own folder.",
                confirm.source
            ),
        ),
        Some(import) => {
            let detail = match &import.previous_version {
                Some(version) if !version.trim().is_empty() => format!(
                    "This replaces version {version} of the pack in {}. Mods you skipped or removed stay that way.",
                    import.cluster_name
                ),
                _ => format!(
                    "Its mods are added next to the ones already in {}. Mods the instance already has are not added twice, and config files you edited are kept.",
                    import.cluster_name
                ),
            };
            (
                format!("Add {} to {}?", confirm.pack_name, import.cluster_name),
                detail,
            )
        }
    }
}

fn dialog(
    confirm: &ModpackConfirm,
    choices: Choices,
    dispatch: crate::Actions,
    measured: Measured,
) -> impl IntoElement {
    let summary = &confirm.summary;
    let version = if confirm.version.trim().is_empty() {
        "Not stated by the pack".to_string()
    } else {
        confirm.version.clone()
    };

    let mut facts = vec![
        fact("Instance", confirm.instance_name.clone()),
        fact("Version", version),
        fact(
            "Minecraft",
            format!("{} · {}", confirm.mc_version, confirm.loader),
        ),
        fact("Content", content_line(summary)),
    ];
    if summary.download_bytes > 0 {
        facts.push(fact(
            "Download",
            format!(
                "Up to {}, less for files the launcher already has",
                format_size(summary.download_bytes)
            ),
        ));
    }

    let (title, subtitle) = intro(confirm);
    let cancel = dispatch.clone();
    let add = dispatch;
    let importing = confirm.import.is_some();
    let flagged_section = confirm
        .import
        .as_ref()
        .filter(|import| !import.flagged.is_empty())
        .map(|import| flagged_section(import, &confirm.mc_version, choices).into_element());
    let flagged_pack = confirm
        .flagged
        .as_ref()
        .map(|flagged| flagged_pack_section(&confirm.pack_name, flagged).into_element());
    let add_button = Button::new().on_press(move |_| {
        if importing {
            add.confirm_modpack_import(choices.read().clone());
        } else {
            add.confirm_modpack();
        }
    });
    let add_button = match (flagged_pack.is_some(), importing) {
        (true, true) => add_button.danger().text("Add anyway"),
        (true, false) => add_button.danger().text("Install anyway"),
        (false, _) => add_button
            .primary()
            .child(Icon::new(IconType::Plus).size(15.))
            .text("Add"),
    };

    rect()
        .vertical()
        .width(Size::px(DIALOG_W))
        .height(Size::Inner)
        .max_width(Size::window_percent(95.))
        .max_height(Size::window_percent(85.))
        .overflow(Overflow::Clip)
        .corner_radius(CornerRadius::new_all(16.))
        .background(CARD_BG)
        .border(border_all_color(1., colors::component_border()))
        .shadow(Shadow::from((
            0.,
            18.,
            52.,
            0.,
            Color::from_argb(150, 0, 0, 0),
        )))
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::Inner)
                .padding(Gaps::new_all(DIALOG_PAD))
                .spacing(SECTION_GAP)
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(3.)
                        .on_sized(track_height(measured.header_h))
                        .child(
                            label()
                                .text(title)
                                .font_size(17.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .max_lines(2)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(subtitle)
                                .font_size(12.5)
                                .max_lines(3)
                                .color(colors::fg_secondary()),
                        ),
                )
                .child(
                    ScrollArea::new()
                        .width(Size::fill())
                        .height(Size::px(measured.body_h))
                        .scrollbar_gutter(true)
                        .child(
                            rect()
                                .vertical()
                                .width(Size::fill())
                                .spacing(SECTION_GAP)
                                .on_sized(track_height(measured.content_h))
                                .maybe_child(flagged_pack)
                                .child(
                                    rect()
                                        .vertical()
                                        .width(Size::fill())
                                        .spacing(8.)
                                        .padding(Gaps::new_all(12.))
                                        .corner_radius(CornerRadius::new_all(10.))
                                        .background(colors::component_bg())
                                        .border(border_all_color(1., colors::component_border()))
                                        .children(facts),
                                )
                                .children(notes(summary).into_iter().map(|(text, color)| {
                                    label()
                                        .text(text)
                                        .font_size(12.)
                                        .max_lines(3)
                                        .color(color)
                                        .into_element()
                                }))
                                .maybe_child(flagged_section),
                        ),
                )
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .cross_align(Alignment::Center)
                        .main_align(Alignment::End)
                        .spacing(8.)
                        .on_sized(track_height(measured.footer_h))
                        .child(
                            Button::new()
                                .ghost()
                                .on_press(move |_| cancel.cancel_modpack())
                                .text("Cancel"),
                        )
                        .child(add_button),
                ),
        )
}

fn flagged_pack_section(pack_name: &str, flagged: &FlaggedModpack) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(
                    Icon::new(IconType::AlertTriangle)
                        .size(16.)
                        .color(colors::code_warn()),
                )
                .child(
                    label()
                        .text("Flagged modpack")
                        .font_size(14.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(
            label()
                .text(format!(
                    "{pack_name} is flagged as a problematic modpack. Installing it is not recommended."
                ))
                .font_size(12.)
                .max_lines(3)
                .color(colors::fg_secondary()),
        )
        .maybe_child(
            flagged
                .explanation
                .clone()
                .map(|markdown| explanation_panel(markdown).into_element()),
        )
}

fn flagged_section(
    import: &ModpackImportView,
    mc_version: &str,
    choices: Choices,
) -> impl IntoElement {
    let count = import.flagged.len();
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(
                    Icon::new(IconType::AlertTriangle)
                        .size(16.)
                        .color(colors::code_warn()),
                )
                .child(
                    label()
                        .text(count_label(count, "flagged mod", "flagged mods"))
                        .font_size(14.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(
            label()
                .text("These mods are known to cause problems. Skipped mods are not installed, now or when you add this pack again.")
                .font_size(12.)
                .max_lines(3)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(10.)
                .children(import.flagged.iter().map(|flagged| {
                    FlaggedCard {
                        flagged: flagged.clone(),
                        mc_version: mc_version.to_string(),
                        choices,
                    }
                    .into_element()
                })),
        )
}

#[derive(PartialEq)]
struct FlaggedCard {
    flagged: FlaggedPackMod,
    mc_version: String,
    choices: Choices,
}

impl Component for FlaggedCard {
    fn render(&self) -> impl IntoElement {
        let package_id = self.flagged.package_id.clone();
        let choice = self
            .choices
            .read()
            .get(&package_id)
            .copied()
            .unwrap_or_default();
        let mut choices = self.choices;
        let mut choose = move |id: String, picked: FlaggedChoice| {
            choices.write().insert(id, picked);
        };

        let skip_id = package_id.clone();
        let keep_id = package_id.clone();
        let mut card = rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .padding(Gaps::new_all(12.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::component_bg())
            .border(border_all_color(1., colors::component_border()))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(8.)
                    .child(
                        label()
                            .text(self.flagged.name.clone())
                            .width(Size::flex(1.))
                            .font_size(13.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .child(choice_button(
                        "Skip",
                        choice == FlaggedChoice::Skip,
                        move || choose(skip_id.clone(), FlaggedChoice::Skip),
                    ))
                    .child(choice_button(
                        "Keep",
                        choice == FlaggedChoice::Keep,
                        move || choose(keep_id.clone(), FlaggedChoice::Keep),
                    )),
            );

        if let Some(markdown) = &self.flagged.explanation {
            card = card.child(explanation_panel(markdown.clone()));
        }

        if !self.flagged.alternatives.is_empty() {
            card = card
                .child(
                    label()
                        .text("Or use one of these instead:")
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                )
                .children(self.flagged.alternatives.iter().enumerate().map(
                    |(index, alternative)| {
                        AlternativeChoice {
                            alternative: alternative.clone(),
                            package_id: package_id.clone(),
                            index,
                            selected: choice == FlaggedChoice::Replace(index),
                            mc_version: self.mc_version.clone(),
                            choices: self.choices,
                        }
                        .into_element()
                    },
                ));
        }

        card
    }
}

fn choice_button(text: &'static str, selected: bool, on_press: impl FnMut() + 'static) -> Button {
    let mut on_press = on_press;
    let button = Button::new().small().on_press(move |_| on_press());
    if selected {
        button.primary().text(text)
    } else {
        button.secondary().text(text)
    }
}

#[derive(PartialEq)]
struct AlternativeChoice {
    alternative: ResolvedAlternative,
    package_id: String,
    index: usize,
    selected: bool,
    mc_version: String,
    choices: Choices,
}

impl Component for AlternativeChoice {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.alternative.icon_url.clone(), 256);
        let icon = remote_icon(self.alternative.icon_url.as_deref(), &icon_query, ALT_ICON);
        let installable = self.alternative.version_id.is_some();
        let version = self
            .alternative
            .version_number
            .clone()
            .unwrap_or_else(|| format!("No version for {}", self.mc_version));

        let mut choices = self.choices;
        let package_id = self.package_id.clone();
        let picked = FlaggedChoice::Replace(self.index);
        let button = Button::new()
            .small()
            .enabled(installable)
            .on_press(move |_| {
                choices.write().insert(package_id.clone(), picked);
            });
        let button = if self.selected {
            button
                .primary()
                .child(Icon::new(IconType::Check).size(14.))
                .text("Selected")
        } else {
            button.secondary().text("Use instead")
        };

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(ALT_ROW_H))
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(0., 8.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(CARD_BG)
            .border(border_all_color(1., colors::component_border()))
            .child(icon)
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.alternative.name.clone())
                            .font_size(12.5)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(version)
                            .font_size(11.)
                            .max_lines(1)
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(button)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_categories_are_left_out_of_the_content_line() {
        let summary = ModpackSummary {
            mods: 336,
            shaders: 1,
            ..ModpackSummary::default()
        };
        assert_eq!(content_line(&summary), "336 mods, 1 shader");
        assert_eq!(
            content_line(&ModpackSummary::default()),
            "Configs and settings only"
        );
    }
}
