use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::{Button, OverlayPopup};
use crate::hooks::{
    ClusterAction, use_active_cluster_id, use_cluster, use_cluster_mutation, use_game_snapshot,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;

const DIALOG_WIDTH: f32 = 420.;

#[derive(PartialEq)]
pub struct DeleteInstanceModal {
    cluster_id: i64,
    name: String,
    on_close: EventHandler<()>,
    stay: bool,
}

impl DeleteInstanceModal {
    pub fn new(cluster_id: i64, name: String, on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            cluster_id,
            name,
            on_close: on_close.into(),
            stay: false,
        }
    }

    pub fn stay_on_page(mut self) -> Self {
        self.stay = true;
        self
    }
}

impl Component for DeleteInstanceModal {
    fn render(&self) -> impl IntoElement {
        let mutation = use_cluster_mutation();
        let mut active_id = use_active_cluster_id();
        let game = use_game_snapshot();
        let cluster_id = self.cluster_id;
        let stay = self.stay;
        let running = game.is_running(cluster_id);
        let cluster = use_cluster(cluster_id);
        let dedicated = cluster.as_ref().is_none_or(|c| c.uses_dedicated_dir());
        let provisioned = cluster.as_ref().is_some_and(|c| !c.user_created);
        let warning = if dedicated {
            format!(
                "{} and its files will be deleted, including its worlds, settings and installed content. This cannot be undone.",
                self.name
            )
        } else {
            format!(
                "{} and its files will be deleted, including its settings and installed content. Worlds and everything else in the shared game folder are kept. This cannot be undone.",
                self.name
            )
        };

        let close_scrim = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_delete = self.on_close.clone();

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
                                    .text("Delete instance")
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
                            .maybe_child(provisioned.then(|| {
                                label()
                                    .text("It won't be added back automatically. To play this version again, create a OneClient instance for it.")
                                    .font_size(12.)
                                    .color(colors::fg_secondary())
                                    .into_element()
                            }))
                            .maybe_child(running.then(|| {
                                label()
                                    .text("Close the game before deleting this instance.")
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
                                                mutation.mutate(ClusterAction::DeleteInstance {
                                                    cluster_id,
                                                });
                                                if *active_id.peek() == Some(cluster_id) {
                                                    *active_id.write() = None;
                                                }
                                                if !stay {
                                                    let _ = RouterContext::get().push(Route::Home {});
                                                }
                                                close_delete.call(());
                                            })
                                            .text("Delete"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
