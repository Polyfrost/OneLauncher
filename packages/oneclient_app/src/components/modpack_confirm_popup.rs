use std::collections::HashMap;

use freya::prelude::*;
use oneclient_content::modpacks::{ModpackSummary, UnlistedMod};
use oneclient_content::packages::ResolvedAlternative;
use oneclient_core::clusters::{ExistingPack, FlaggedModpack, FlaggedPackMod};

use super::flagged_install_popup::explanation_panel;
use crate::components::{
    Button, Icon, IconType, OverlayPopup, ScrollArea, caution_card, remote_icon, warning_card,
    warning_list,
};
use crate::hooks::{use_cached_image, use_dispatch, use_notifications_snapshot};
use crate::i18n::{t, tf};
use crate::notifications::{
    ExistingPackView, FlaggedChoice, ModpackConfirm, ModpackImportView, SharedPreview,
};
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::utils::{count_label, format_size};

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
        let updates_in_place = self
            .confirm
            .import
            .as_ref()
            .is_some_and(|import| import.previous_version.is_some());
        let existing = use_state(|| {
            if updates_in_place {
                ExistingPack::Replace
            } else {
                ExistingPack::KeepBoth
            }
        });
        let header_h = use_state(|| 0f32);
        let footer_h = use_state(|| 0f32);
        let content_h = use_state(|| 0f32);

        let window_h = Platform::get().current_window().root_size.read().height;
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
            existing,
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

fn content_line(summary: &ModpackSummary) -> String {
    let parts: Vec<String> = [
        (
            summary.mods,
            t("oneclient.components.modpack_confirm.content.mods.one"),
            t("oneclient.components.modpack_confirm.content.mods.other"),
        ),
        (
            summary.resource_packs,
            t("oneclient.components.modpack_confirm.content.resource_packs.one"),
            t("oneclient.components.modpack_confirm.content.resource_packs.other"),
        ),
        (
            summary.shaders,
            t("oneclient.components.modpack_confirm.content.shaders.one"),
            t("oneclient.components.modpack_confirm.content.shaders.other"),
        ),
        (
            summary.other_files,
            t("oneclient.components.modpack_confirm.content.other_files.one"),
            t("oneclient.components.modpack_confirm.content.other_files.other"),
        ),
    ]
    .into_iter()
    .filter(|(count, _, _)| *count > 0)
    .map(|(count, one, many)| count_label(count, one, many))
    .collect();

    if parts.is_empty() {
        t("oneclient.components.modpack_confirm.content.configs_only").to_string()
    } else {
        parts.join(", ")
    }
}

fn notes(summary: &ModpackSummary) -> Vec<(String, Color)> {
    let mut notes = Vec::new();
    if summary.optional > 0 {
        notes.push((
            count_label(
                summary.optional,
                t("oneclient.components.modpack_confirm.note.optional.one"),
                t("oneclient.components.modpack_confirm.note.optional.other"),
            ),
            colors::fg_secondary(),
        ));
    }
    if summary.manual > 0 {
        notes.push((
            count_label(
                summary.manual,
                t("oneclient.components.modpack_confirm.note.manual.one"),
                t("oneclient.components.modpack_confirm.note.manual.other"),
            ),
            colors::code_warn(),
        ));
    }
    if summary.unresolved > 0 {
        notes.push((
            count_label(
                summary.unresolved,
                t("oneclient.components.modpack_confirm.note.unresolved.one"),
                t("oneclient.components.modpack_confirm.note.unresolved.other"),
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
                .font_size(text_size(12.5))
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(value)
                .width(Size::flex(1.))
                .font_size(text_size(12.5))
                .max_lines(2)
                .color(colors::fg_primary()),
        )
        .into_element()
}

fn intro(confirm: &ModpackConfirm) -> (String, String) {
    match &confirm.import {
        None => (
            tf(
                "oneclient.components.modpack_confirm.intro.new.title",
                &[&confirm.pack_name],
            ),
            tf(
                "oneclient.components.modpack_confirm.intro.new.body",
                &[&confirm.source],
            ),
        ),
        Some(import) => {
            let detail = match &import.previous_version {
                Some(version) if !version.trim().is_empty() => tf(
                    "oneclient.components.modpack_confirm.intro.replace_version",
                    &[version, &import.cluster_name],
                ),
                Some(_) => tf(
                    "oneclient.components.modpack_confirm.intro.replace",
                    &[&import.cluster_name],
                ),
                None => tf(
                    "oneclient.components.modpack_confirm.intro.merge",
                    &[&import.cluster_name],
                ),
            };
            (
                tf(
                    "oneclient.components.modpack_confirm.intro.import.title",
                    &[&confirm.pack_name, &import.cluster_name],
                ),
                detail,
            )
        }
    }
}

fn dialog(
    confirm: &ModpackConfirm,
    choices: Choices,
    existing: State<ExistingPack>,
    dispatch: crate::Actions,
    measured: Measured,
) -> impl IntoElement {
    let summary = &confirm.summary;
    let version = if confirm.version.trim().is_empty() {
        t("oneclient.components.modpack_confirm.version_unknown").to_string()
    } else {
        confirm.version.clone()
    };

    let mut facts = vec![
        fact(
            t("oneclient.components.modpack_confirm.fact.instance"),
            confirm.instance_name.clone(),
        ),
        fact(
            t("oneclient.components.modpack_confirm.fact.version"),
            version,
        ),
        fact(
            "Minecraft",
            format!("{} · {}", confirm.mc_version, confirm.loader),
        ),
        fact(
            t("oneclient.components.modpack_confirm.fact.content"),
            content_line(summary),
        ),
    ];
    if summary.download_bytes > 0 {
        facts.push(fact(
            t("oneclient.components.modpack_confirm.fact.download"),
            tf(
                "oneclient.components.modpack_confirm.fact.download_size",
                &[&format_size(summary.download_bytes)],
            ),
        ));
    }

    let (title, subtitle) = intro(confirm);
    let cancel = dispatch.clone();
    let add = dispatch;
    let importing = confirm.import.is_some();
    let existing_section = confirm.import.as_ref().and_then(|import| {
        import
            .existing
            .as_ref()
            .map(|pack| existing_section(pack, existing).into_element())
    });
    let import_notes: Vec<Element> = confirm
        .import
        .iter()
        .flat_map(|import| import.notes.iter())
        .map(|(text, warn)| {
            label()
                .text(text.clone())
                .font_size(text_size(12.))
                .max_lines(4)
                .color(if *warn {
                    colors::code_warn()
                } else {
                    colors::fg_secondary()
                })
                .into_element()
        })
        .collect();
    let flagged_section = confirm
        .import
        .as_ref()
        .filter(|import| !import.flagged.is_empty())
        .map(|import| flagged_section(import, &confirm.mc_version, choices).into_element());
    let flagged_pack = confirm
        .flagged
        .as_ref()
        .map(|flagged| flagged_pack_section(&confirm.pack_name, flagged).into_element());
    let unlisted =
        (!confirm.unlisted.is_empty()).then(|| unlisted_section(&confirm.unlisted).into_element());
    let unverified =
        (summary.unverified > 0).then(|| unverified_card(summary.unverified).into_element());
    let unchecked = if confirm.mods_unchecked {
        Some(caution_card(
            t("oneclient.components.modpack_confirm.unchecked.title"),
            t("oneclient.components.modpack_confirm.unchecked.body"),
        ))
    } else if confirm.mods_unconfirmed > 0 {
        Some(caution_card(
            count_label(
                confirm.mods_unconfirmed,
                t("oneclient.components.modpack_confirm.unconfirmed.title.one"),
                t("oneclient.components.modpack_confirm.unconfirmed.title.other"),
            ),
            t("oneclient.components.modpack_confirm.unconfirmed.body"),
        ))
    } else {
        None
    }
    .map(IntoElement::into_element);
    let warned =
        flagged_pack.is_some() || unlisted.is_some() || unverified.is_some() || unchecked.is_some();
    let shared_section = confirm.shared.as_ref().map(shared_section);
    let add_button = Button::new().on_press(move |_| {
        if importing {
            add.confirm_modpack_import(choices.read().clone(), *existing.read());
        } else {
            add.confirm_modpack();
        }
    });
    let add_button = match (warned, importing) {
        (true, true) => add_button
            .danger()
            .text(t("oneclient.components.modpack_confirm.add_anyway")),
        (true, false) => add_button
            .danger()
            .text(t("oneclient.components.modpack_confirm.install_anyway")),
        (false, _) => add_button
            .primary()
            .child(Icon::new(IconType::Plus).size(15.))
            .text(t("oneclient.components.modpack_confirm.add")),
    };

    rect()
        .vertical()
        .width(Size::px(DIALOG_W))
        .height(Size::Inner)
        .max_width(Size::window_percent(95.))
        .max_height(Size::window_percent(85.))
        .overflow(Overflow::Clip)
        .corner_radius(rounded(16.))
        .background(colors::card())
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
                                .font_size(text_size(17.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .max_lines(2)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(subtitle)
                                .font_size(text_size(12.5))
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
                                .maybe_child(unlisted)
                                .maybe_child(unverified)
                                .maybe_child(unchecked)
                                .child(
                                    rect()
                                        .vertical()
                                        .width(Size::fill())
                                        .spacing(8.)
                                        .padding(Gaps::new_all(12.))
                                        .corner_radius(rounded(10.))
                                        .background(colors::component_bg())
                                        .border(border_all_color(1., colors::component_border()))
                                        .children(facts),
                                )
                                .children(
                                    confirm
                                        .notices
                                        .iter()
                                        .map(|notice| (notice.clone(), colors::code_warn()))
                                        .chain(notes(summary))
                                        .map(|(text, color)| {
                                            label()
                                                .text(text)
                                                .font_size(text_size(12.))
                                                .max_lines(3)
                                                .color(color)
                                                .into_element()
                                        }),
                                )
                                .maybe_child(shared_section)
                                .children(import_notes)
                                .maybe_child(existing_section)
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
                                .text(t("oneclient.common.cancel")),
                        )
                        .child(add_button),
                ),
        )
}

fn shared_section(preview: &SharedPreview) -> Element {
    let unverified = preview.files.iter().filter(|file| !file.verified).count();
    let mut lines = vec![
        label()
            .text(tf(
                "oneclient.components.modpack_confirm.shared.linked",
                &[&preview.code],
            ))
            .font_size(text_size(12.))
            .max_lines(4)
            .color(colors::fg_secondary())
            .into_element(),
    ];
    if unverified > 0 {
        lines.push(
            label()
                .text(count_label(
                    unverified,
                    t("oneclient.components.modpack_confirm.shared.unverified.one"),
                    t("oneclient.components.modpack_confirm.shared.unverified.other"),
                ))
                .font_size(text_size(12.))
                .max_lines(3)
                .color(colors::code_warn())
                .into_element(),
        );
    }
    if preview.settings > 0 {
        lines.push(
            label()
                .text(count_label(
                    preview.settings,
                    t("oneclient.components.modpack_confirm.shared.settings.one"),
                    t("oneclient.components.modpack_confirm.shared.settings.other"),
                ))
                .font_size(text_size(12.))
                .color(colors::fg_secondary())
                .into_element(),
        );
    }

    let rows: Vec<Element> = preview
        .files
        .iter()
        .map(|file| {
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(
                    label()
                        .text(file.name.clone())
                        .width(Size::flex(1.))
                        .font_size(text_size(12.5))
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(colors::fg_primary()),
                )
                .child(label().text(file.source).font_size(text_size(11.5)).color(
                    if file.verified {
                        colors::fg_secondary()
                    } else {
                        colors::code_warn()
                    },
                ))
                .into_element()
        })
        .collect();

    let mut section = rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .children(lines);
    if !rows.is_empty() {
        section = section.child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(6.)
                .padding(Gaps::new_all(12.))
                .corner_radius(rounded(10.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::component_border()))
                .child(
                    label()
                        .text(t("oneclient.components.modpack_confirm.shared.inside"))
                        .font_size(text_size(12.5))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .children(rows)
                .maybe_child(preview.truncated.then(|| {
                    label()
                        .text(t("oneclient.components.modpack_confirm.shared.more"))
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary())
                        .into_element()
                })),
        );
    }
    section.into_element()
}

fn existing_section(pack: &ExistingPackView, existing: State<ExistingPack>) -> impl IntoElement {
    let current = *existing.read();
    let mut replace = existing;
    let mut keep_both = existing;
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .padding(Gaps::new_all(12.))
        .corner_radius(rounded(10.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(
            label()
                .text(if pack.version.trim().is_empty() {
                    tf(
                        "oneclient.components.modpack_confirm.existing.body",
                        &[&pack.name],
                    )
                } else {
                    tf(
                        "oneclient.components.modpack_confirm.existing.body_version",
                        &[&pack.name, &pack.version],
                    )
                })
                .font_size(text_size(12.))
                .max_lines(4)
                .color(colors::fg_primary()),
        )
        .child(
            rect()
                .horizontal()
                .spacing(8.)
                .child(choice_button(
                    t("oneclient.components.modpack_confirm.existing.replace"),
                    current == ExistingPack::Replace,
                    move || replace.set(ExistingPack::Replace),
                ))
                .child(choice_button(
                    t("oneclient.components.modpack_confirm.existing.keep_both"),
                    current == ExistingPack::KeepBoth,
                    move || keep_both.set(ExistingPack::KeepBoth),
                )),
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
                        .text(t("oneclient.components.modpack_confirm.flagged_pack.title"))
                        .font_size(text_size(14.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(
            label()
                .text(tf(
                    "oneclient.components.modpack_confirm.flagged_pack.body",
                    &[&pack_name],
                ))
                .font_size(text_size(12.))
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

fn unlisted_section(mods: &[UnlistedMod]) -> impl IntoElement {
    warning_card(
        count_label(
            mods.len(),
            t("oneclient.components.modpack_confirm.unlisted.title.one"),
            t("oneclient.components.modpack_confirm.unlisted.title.other"),
        ),
        t("oneclient.components.modpack_confirm.unlisted.body"),
    )
    .child(warning_list(mods.iter().map(|unlisted| {
        let origin = if unlisted.bundled {
            t("oneclient.components.modpack_confirm.unlisted.bundled")
        } else {
            t("oneclient.components.modpack_confirm.unlisted.external")
        };
        (unlisted.name.clone(), Some(origin.to_string()))
    })))
}

fn unverified_card(count: usize) -> impl IntoElement {
    warning_card(
        count_label(
            count,
            t("oneclient.components.modpack_confirm.unverified.title.one"),
            t("oneclient.components.modpack_confirm.unverified.title.other"),
        ),
        count_label(
            count,
            t("oneclient.components.modpack_confirm.unverified.body.one"),
            t("oneclient.components.modpack_confirm.unverified.body.other"),
        ),
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
                        .text(count_label(
                            count,
                            t("oneclient.components.modpack_confirm.flagged.title.one"),
                            t("oneclient.components.modpack_confirm.flagged.title.other"),
                        ))
                        .font_size(text_size(14.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(
            label()
                .text(t("oneclient.components.modpack_confirm.flagged.body"))
                .font_size(text_size(12.))
                .max_lines(3)
                .color(colors::fg_secondary()),
        )
        .child(rect().vertical().width(Size::fill()).spacing(10.).children(
            import.flagged.iter().map(|flagged| {
                FlaggedCard {
                    flagged: flagged.clone(),
                    mc_version: mc_version.to_string(),
                    choices,
                }
                .into_element()
            }),
        ))
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
            .corner_radius(rounded(10.))
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
                            .font_size(text_size(13.))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .child(choice_button(
                        t("oneclient.components.modpack_confirm.flagged.skip"),
                        choice == FlaggedChoice::Skip,
                        move || choose(skip_id.clone(), FlaggedChoice::Skip),
                    ))
                    .child(choice_button(
                        t("oneclient.components.modpack_confirm.flagged.keep"),
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
                        .text(t(
                            "oneclient.components.modpack_confirm.flagged.alternatives",
                        ))
                        .font_size(text_size(12.))
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
        let version = self.alternative.version_number.clone().unwrap_or_else(|| {
            tf(
                "oneclient.components.flagged_install_popup.no_version",
                &[&self.mc_version],
            )
        });

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
                .text(t("oneclient.components.modpack_confirm.flagged.selected"))
        } else {
            button.secondary().text(t(
                "oneclient.components.modpack_confirm.flagged.use_instead",
            ))
        };

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(ALT_ROW_H))
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(0., 8.))
            .corner_radius(rounded(8.))
            .background(colors::card())
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
                            .font_size(text_size(12.5))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(version)
                            .font_size(text_size(11.))
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
