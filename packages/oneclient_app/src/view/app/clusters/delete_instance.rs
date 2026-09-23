use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::{Button, OverlayPopup};
use crate::hooks::{ClusterAction, use_active_cluster_id, use_cluster_mutation, use_game_snapshot};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;

const DIALOG_WIDTH: f32 = 420.;

#[derive(PartialEq)]
pub struct DeleteInstanceModal {
    cluster_id: i64,
    name: String,
    on_close: EventHandler<()>,
}

impl DeleteInstanceModal {
    pub fn new(cluster_id: i64, name: String, on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            cluster_id,
            name,
            on_close: on_close.into(),
        }
    }
}

impl Component for DeleteInstanceModal {
    fn render(&self) -> impl IntoElement {
        let mutation = use_cluster_mutation();
        let mut active_id = use_active_cluster_id();
        let game = use_game_snapshot();
        let cluster_id = self.cluster_id;
        let running = game.is_running(cluster_id);

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
                                    .text(format!(
                                        "{} and its files will be deleted, including its worlds, settings and installed content. This cannot be undone.",
                                        self.name
                                    ))
                                    .font_size(13.)
                                    .color(colors::fg_secondary()),
                            )
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
                                                let _ = RouterContext::get().push(Route::Home {});
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
