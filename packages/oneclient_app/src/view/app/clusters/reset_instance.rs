use freya::prelude::*;

use crate::components::{Button, OverlayPopup};
use crate::hooks::{use_cluster, use_game_snapshot};
use crate::theme::colors;
use crate::ui::border_all_color;

const DIALOG_WIDTH: f32 = 420.;

#[derive(PartialEq)]
pub struct ResetInstanceModal {
    cluster_id: i64,
    name: String,
    on_close: EventHandler<()>,
    on_reset: EventHandler<()>,
}

impl ResetInstanceModal {
    pub fn new(
        cluster_id: i64,
        name: String,
        on_close: impl Into<EventHandler<()>>,
        on_reset: impl Into<EventHandler<()>>,
    ) -> Self {
        Self {
            cluster_id,
            name,
            on_close: on_close.into(),
            on_reset: on_reset.into(),
        }
    }
}

impl Component for ResetInstanceModal {
    fn render(&self) -> impl IntoElement {
        let game = use_game_snapshot();
        let cluster_id = self.cluster_id;
        let running = game.is_running(cluster_id);
        let cluster = use_cluster(cluster_id);
        let shared = cluster.as_ref().is_some_and(|c| !c.uses_dedicated_dir());
        let modpack = cluster
            .as_ref()
            .is_some_and(|c| c.linked_modpack_hash.is_some());

        let what = if shared {
            "its mods and settings are reset"
        } else {
            "its mods, settings and config files are reset"
        };
        let reinstall = if modpack {
            " and the modpack is installed again"
        } else {
            ""
        };
        let warning = format!(
            "{} goes back to how it was when it was set up: {what}{reinstall}. \
             Worlds, screenshots and logs are kept, and everything removed is moved to a backup folder.",
            self.name
        );

        let close_scrim = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_reset = self.on_close.clone();
        let on_reset = self.on_reset.clone();

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
                                    .text("Reset instance")
                                    .font_size(18.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(warning)
                                    .font_size(13.)
                                    .color(colors::fg_secondary()),
                            )
                            .maybe_child(shared.then(|| {
                                label()
                                    .text("This version uses the shared game folder, so the config files there are kept. Other versions use them too.")
                                    .font_size(12.)
                                    .color(colors::fg_secondary())
                                    .into_element()
                            }))
                            .maybe_child(running.then(|| {
                                label()
                                    .text("Close the game before resetting this instance.")
                                    .font_size(12.)
                                    .color(colors::fg_secondary())
                                    .into_element()
                            }))
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
                                            .text("Cancel"),
                                    )
                                    .child(
                                        Button::new()
                                            .danger()
                                            .enabled(!running)
                                            .on_press(move |_| {
                                                on_reset.call(());
                                                close_reset.call(());
                                            })
                                            .text("Reset"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
