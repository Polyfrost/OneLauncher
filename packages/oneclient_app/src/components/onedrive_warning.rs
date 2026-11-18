use std::path::Path;

use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OVERLAY_BASE_LEVEL, OverlayPopup};
use crate::i18n::t;
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, path_block};

/// Asked before the data folder goes into OneDrive, which syncs every file the launcher writes
pub fn onedrive_warning(
    folder: &Path,
    on_back: impl Into<EventHandler<()>>,
    on_continue: impl Into<EventHandler<()>>,
) -> Element {
    let on_back = on_back.into();
    let on_continue = on_continue.into();
    let close = on_back.clone();

    let card = rect()
        .vertical()
        .width(Size::px(460.))
        .max_width(Size::window_percent(90.))
        .spacing(14.)
        .padding(Gaps::new_all(20.))
        .corner_radius(rounded(14.))
        .background(colors::card())
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(
                    Icon::new(IconType::AlertTriangle)
                        .size(20.)
                        .color(colors::code_warn()),
                )
                .child(
                    label()
                        .text(t("oneclient.components.onedrive_warning.title"))
                        .font_size(text_size(16.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(path_block(
            t("oneclient.components.onedrive_warning.folder"),
            folder,
        ))
        .child(
            label()
                .text(t("oneclient.components.onedrive_warning.body"))
                .font_size(text_size(12.))
                .max_lines(8)
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
                        .on_press(move |_| on_continue.call(()))
                        .text(t("oneclient.components.onedrive_warning.use_anyway")),
                )
                .child(
                    Button::new()
                        .primary()
                        .on_press(move |_| on_back.call(()))
                        .text(t("oneclient.components.onedrive_warning.choose_another")),
                ),
        );

    OverlayPopup::new()
        .overlay_level(OVERLAY_BASE_LEVEL + 3)
        .on_close(move |()| close.call(()))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(card),
        )
        .into_element()
}
