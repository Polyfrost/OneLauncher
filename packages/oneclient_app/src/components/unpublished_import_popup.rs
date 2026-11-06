use freya::prelude::*;
use oneclient_core::UnpublishedFile;

use crate::components::{
    Button, OVERLAY_BASE_LEVEL, OverlayPopup, caution_card, warning_card, warning_list,
};
use crate::hooks::{use_dispatch, use_notifications_snapshot};
use crate::i18n::{t, tf};
use crate::notifications::{UnpublishedImport, UnpublishedInstance};
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::utils::count_label;

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
            .overlay_level(OVERLAY_BASE_LEVEL + 1)
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

pub(crate) fn unpublished_rows(
    mods: &[UnpublishedFile],
) -> impl Iterator<Item = (String, Option<String>)> + '_ {
    mods.iter().map(|file| {
        let note = if file.enabled {
            t("oneclient.components.unpublished_import.local_file")
        } else {
            t("oneclient.components.unpublished_import.disabled")
        };
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
    let title = match (import.instances.as_slice(), total) {
        ([only], 1) => tf(
            "oneclient.components.unpublished_import.title.named",
            &[&only.name],
        ),
        _ => count_label(
            total,
            t("oneclient.components.unpublished_import.title.one"),
            t("oneclient.components.unpublished_import.title.other"),
        ),
    };
    let subtitle = if import
        .instances
        .iter()
        .any(|instance| !instance.mods.is_empty())
    {
        count_label(
            total,
            t("oneclient.components.unpublished_import.subtitle.unpublished.one"),
            t("oneclient.components.unpublished_import.subtitle.unpublished.other"),
        )
    } else {
        count_label(
            total,
            t("oneclient.components.unpublished_import.subtitle.unchecked.one"),
            t("oneclient.components.unpublished_import.subtitle.unchecked.other"),
        )
    };

    let others = (import.others > 0).then(|| {
        Button::new()
            .secondary()
            .on_press(move |_| on_import_others())
            .text(count_label(
                import.others,
                t("oneclient.components.unpublished_import.import_others.one"),
                t("oneclient.components.unpublished_import.import_others.other"),
            ))
            .into_element()
    });
    let all_text = if import.others > 0 {
        t("oneclient.components.unpublished_import.import_all")
    } else {
        t("oneclient.components.unpublished_import.import_anyway")
    };

    rect()
        .vertical()
        .width(Size::px(DIALOG_W))
        .max_width(Size::window_percent(95.))
        .spacing(SECTION_GAP)
        .padding(Gaps::new_all(DIALOG_PAD))
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
                .spacing(3.)
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
                        .text(t("oneclient.common.cancel")),
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
    if instance.unconfirmed > 0 {
        let title = if instance.unconfirmed == 1 {
            tf(
                "oneclient.components.unpublished_import.unconfirmed.title.one",
                &[&instance.name, &instance.unconfirmed],
            )
        } else {
            tf(
                "oneclient.components.unpublished_import.unconfirmed.title.other",
                &[&instance.name, &instance.unconfirmed],
            )
        };
        return caution_card(
            title,
            tf(
                "oneclient.components.unpublished_import.unconfirmed.body",
                &[&instance.game],
            ),
        )
        .into_element();
    }
    let title = if instance.mods.len() == 1 {
        tf(
            "oneclient.components.unpublished_import.unpublished.title.one",
            &[&instance.name, &instance.mods.len()],
        )
    } else {
        tf(
            "oneclient.components.unpublished_import.unpublished.title.other",
            &[&instance.name, &instance.mods.len()],
        )
    };
    warning_card(
        title,
        tf(
            "oneclient.components.unpublished_import.unpublished.body",
            &[&instance.game],
        ),
    )
    .child(warning_list(unpublished_rows(&instance.mods)))
    .into_element()
}
