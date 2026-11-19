use freya::prelude::*;

use crate::components::{Button, OVERLAY_BASE_LEVEL, OverlayPopup};
use crate::hooks::use_server_hub;
use crate::i18n::{t, tf};
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

#[derive(PartialEq)]
pub struct ServerShutdownOverlay;

impl Component for ServerShutdownOverlay {
    fn render(&self) -> impl IntoElement {
        let hub = use_server_hub();
        if !hub.shutting_down {
            return rect().into_element();
        }

        OverlayPopup::new()
            .overlay_level(OVERLAY_BASE_LEVEL + 4)
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(420.))
                            .max_width(Size::window_percent(90.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(rounded(14.))
                            .background(colors::card())
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text(t("oneclient.servers.shutdown.title"))
                                    .font_size(text_size(16.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(tf(
                                        "oneclient.servers.shutdown.body",
                                        &[&hub.live_count()],
                                    ))
                                    .font_size(text_size(12.))
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                rect().width(Size::fill()).main_align(Alignment::End).child(
                                    Button::new()
                                        .danger()
                                        .on_press(|_| crate::servers::force_stop_all())
                                        .text(t("oneclient.servers.shutdown.force")),
                                ),
                            ),
                    ),
            )
            .into_element()
    }
}
