use freya::prelude::*;
use oneclient_core::clusters::ExportMode;

use super::instance::{WideCard, wide_card};
use crate::components::{Button, IconType, OverlayPopup};
use crate::hooks::use_dispatch;
use crate::i18n::{t, tf};
use crate::theme::colors;
use crate::ui::border_all_color;

const DIALOG_WIDTH: f32 = 520.;

#[derive(PartialEq)]
pub struct ExportInstanceModal {
    cluster_id: i64,
    name: String,
    on_close: EventHandler<()>,
}

impl ExportInstanceModal {
    pub fn new(cluster_id: i64, name: String, on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            cluster_id,
            name,
            on_close: on_close.into(),
        }
    }
}

impl Component for ExportInstanceModal {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mut mode = use_state(|| ExportMode::OneClient);
        let chosen = *mode.read();
        let cluster_id = self.cluster_id;

        let close_scrim = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_export = self.on_close.clone();

        let cards = [
            (
                ExportMode::OneClient,
                IconType::IconLogo,
                "OneClient",
                Some(t("oneclient.clusters.export.oneclient.badge")),
                t("oneclient.clusters.export.oneclient.blurb"),
                t("oneclient.clusters.export.oneclient.meta"),
            ),
            (
                ExportMode::Compatibility,
                IconType::Globe01,
                t("oneclient.clusters.export.compatibility.title"),
                None,
                t("oneclient.clusters.export.compatibility.blurb"),
                t("oneclient.clusters.export.compatibility.meta"),
            ),
        ];

        OverlayPopup::new()
            .on_close(move |()| close_scrim.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(DIALOG_WIDTH))
                            .max_width(Size::window_percent(92.))
                            .spacing(16.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(16.))
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text(tf("oneclient.clusters.export.title", &[&self.name]))
                                    .font_size(18.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(t("oneclient.clusters.export.description"))
                                    .font_size(13.)
                                    .color(colors::fg_secondary()),
                            )
                            .child(rect().vertical().width(Size::fill()).spacing(10.).children(
                                cards.into_iter().map(
                                    |(option, icon, title, badge, blurb, meta)| {
                                        wide_card(WideCard {
                                            icon,
                                            title: title.to_string(),
                                            badge: badge.map(str::to_string),
                                            blurb: blurb.to_string(),
                                            meta: meta.to_string(),
                                            selected: chosen == option,
                                            on_press: (move |()| mode.set(option)).into(),
                                        })
                                    },
                                ),
                            ))
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .ghost()
                                            .on_press(move |_| close_cancel.call(()))
                                            .text(t("oneclient.common.cancel")),
                                    )
                                    .child(
                                        Button::new()
                                            .on_press(move |_| {
                                                dispatch.export_cluster(cluster_id, chosen);
                                                close_export.call(());
                                            })
                                            .text(t("oneclient.clusters.export.confirm")),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
