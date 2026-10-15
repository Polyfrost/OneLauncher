use freya::prelude::*;
use oneclient_content::modpacks::ModpackSummary;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::{use_dispatch, use_notifications_snapshot};
use crate::notifications::ModpackConfirm;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::format_size;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 500.;
const DIALOG_PAD: f32 = 22.;
const FACT_LABEL_W: f32 = 110.;

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
                    .child(dialog(&confirm, dispatch)),
            )
            .into_element()
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

fn dialog(confirm: &ModpackConfirm, dispatch: crate::Actions) -> impl IntoElement {
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

    let cancel = dispatch.clone();
    let add = dispatch;

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
                .spacing(16.)
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(3.)
                        .child(
                            label()
                                .text(format!("Add {}?", confirm.pack_name))
                                .font_size(17.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .max_lines(2)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(format!(
                                    "From {}. It is added as a new instance with its own folder.",
                                    confirm.source
                                ))
                                .font_size(12.5)
                                .max_lines(2)
                                .color(colors::fg_secondary()),
                        ),
                )
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
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .cross_align(Alignment::Center)
                        .main_align(Alignment::End)
                        .spacing(8.)
                        .child(
                            Button::new()
                                .ghost()
                                .on_press(move |_| cancel.cancel_modpack())
                                .text("Cancel"),
                        )
                        .child(
                            Button::new()
                                .primary()
                                .on_press(move |_| add.confirm_modpack())
                                .child(Icon::new(IconType::Plus).size(15.))
                                .text("Add"),
                        ),
                ),
        )
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
