use freya::prelude::*;
use oneclient_core::UnpublishedFile;

use crate::components::{Button, OverlayPopup, caution_card, warning_card, warning_list};
use crate::hooks::{use_dispatch, use_notifications_snapshot};
use crate::notifications::{UnpublishedImport, UnpublishedInstance};
use crate::theme::colors;
use crate::ui::border_all_color;

const DIALOG_W: f32 = 500.;
const DIALOG_PAD: f32 = 22.;
const SECTION_GAP: f32 = 16.;
const BODY_MAX_H: f32 = 380.;

#[derive(PartialEq)]
pub struct UnpublishedImportPopup;

impl Component for UnpublishedImportPopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();

        let Some(import) = snapshot.unpublished_import else {
            return rect().into_element();
        };

        let close = dispatch.clone();
        let cancel = dispatch.clone();
        let others = dispatch.clone();
        OverlayPopup::new()
            .on_close(move |_| close.cancel_unpublished_import())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(dialog(
                        &import,
                        move || cancel.cancel_unpublished_import(),
                        move || others.confirm_unpublished_import(false),
                        move || dispatch.confirm_unpublished_import(true),
                    )),
            )
            .into_element()
    }
}

fn count_label(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

pub(crate) fn unpublished_rows(
    mods: &[UnpublishedFile],
) -> impl Iterator<Item = (String, Option<String>)> + '_ {
    mods.iter().map(|file| {
        let note = if file.enabled { "Local file" } else { "Disabled" };
        (file.name.clone(), Some(note.to_string()))
    })
}

fn dialog(
    import: &UnpublishedImport,
    mut on_cancel: impl FnMut() + 'static,
    mut on_import_others: impl FnMut() + 'static,
    mut on_import_all: impl FnMut() + 'static,
) -> impl IntoElement {
    let total = import.instances.len() + import.others;
    let (title, whose) = match (import.instances.as_slice(), total) {
        ([only], 1) => (format!("Import {}?", only.name), "its"),
        _ => (
            format!("Import {}?", count_label(total, "instance", "instances")),
            "their",
        ),
    };
    let subtitle = if import.instances.iter().any(|instance| !instance.mods.is_empty()) {
        format!("Some of {whose} mods are not on Modrinth or CurseForge. Check them before you import.")
    } else {
        format!("OneClient could not check {whose} mods. Check them before you import.")
    };

    let others = (import.others > 0).then(|| {
        let text = if import.others == 1 {
            "Only import the other one".to_string()
        } else {
            format!("Only import the other {}", import.others)
        };
        Button::new()
            .secondary()
            .on_press(move |_| on_import_others())
            .text(text)
            .into_element()
    });
    let all_text = if import.others > 0 {
        "Import all"
    } else {
        "Import anyway"
    };

    rect()
        .vertical()
        .width(Size::px(DIALOG_W))
        .max_width(Size::window_percent(95.))
        .spacing(SECTION_GAP)
        .padding(Gaps::new_all(DIALOG_PAD))
        .corner_radius(CornerRadius::new_all(16.))
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
                .spacing(3.)
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
            ScrollView::new()
                .width(Size::fill())
                .height(Size::Inner)
                .max_height(Size::px(BODY_MAX_H))
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(SECTION_GAP)
                        .children(import.instances.iter().map(instance_card)),
                ),
        )
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
                        .on_press(move |_| on_cancel())
                        .text("Cancel"),
                )
                .maybe_child(others)
                .child(
                    Button::new()
                        .danger()
                        .on_press(move |_| on_import_all())
                        .text(all_text),
                ),
        )
}

fn instance_card(instance: &UnpublishedInstance) -> Element {
    if !instance.checked {
        return caution_card(
            format!("{}: couldn't check its mods", instance.name),
            format!(
                "{}. OneClient could not reach Modrinth or CurseForge, so it could not tell whether these mods are published there. Only import it if you trust where it came from.",
                instance.game
            ),
        )
        .into_element();
    }
    warning_card(
        format!(
            "{}: {} not on Modrinth or CurseForge",
            instance.name,
            count_label(instance.mods.len(), "mod", "mods")
        ),
        format!(
            "{}. Nobody has checked these mods, and they could contain malware. Only import instances from people you trust.",
            instance.game
        ),
    )
    .child(warning_list(unpublished_rows(&instance.mods)))
    .into_element()
}
