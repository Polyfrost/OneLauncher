use freya::prelude::*;
use oneclient_content::packages::ProviderId;

use crate::components::{Button, CARD_BG, Dropdown, Icon, IconType, OverlayPopup};
use crate::hooks::{
    query_is_loading, try_cluster_worlds, use_cluster_worlds, use_datapack_world, use_dispatch,
};
use crate::theme::colors;
use crate::ui::border_all_color;

const DIALOG_W: f32 = 400.;

#[derive(PartialEq)]
pub(crate) struct WorldInstallPrompt {
    pub cluster_id: i64,
    pub provider: ProviderId,
    pub project_id: String,
    pub pending: State<Option<String>>,
}

impl Component for WorldInstallPrompt {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let provider = self.provider;
        let project_id = self.project_id.clone();
        let mut pending = self.pending;

        let dispatch = use_dispatch();
        let mut remembered = use_datapack_world();
        let worlds_query = use_cluster_worlds(cluster_id);
        let names: Vec<String> = try_cluster_worlds(&worlds_query)
            .unwrap_or_default()
            .into_iter()
            .map(|w| w.folder_name)
            .collect();

        let mut picked = use_state(|| None::<String>);
        let world = [
            picked.read().clone(),
            remembered.peek().get(&cluster_id).cloned(),
        ]
        .into_iter()
        .flatten()
        .find(|name| names.contains(name))
        .or_else(|| names.first().cloned());

        let control = match &world {
            Some(current) => {
                let options = names.clone();
                Dropdown::new(current.clone(), names.clone())
                    .width(Size::fill())
                    .height(Size::px(32.))
                    .leading(
                        Icon::new(IconType::Globe01)
                            .size(14.)
                            .color(colors::fg_secondary()),
                    )
                    .on_select(move |idx: usize| {
                        if let Some(name) = options.get(idx) {
                            picked.set(Some(name.clone()));
                        }
                    })
                    .into_element()
            }
            None => label()
                .text(if query_is_loading(&worlds_query) {
                    "Loading worlds..."
                } else {
                    "This version has no worlds yet. Create one in game first."
                })
                .font_size(12.)
                .color(colors::fg_secondary())
                .into_element(),
        };

        let install_world = world.clone();
        let install = move |_| {
            let (Some(world), Some(version_id)) = (install_world.clone(), pending.read().clone())
            else {
                return;
            };
            remembered.write().insert(cluster_id, world.clone());
            dispatch.install_package(
                cluster_id,
                provider,
                project_id.clone(),
                version_id,
                Some(world),
            );
            pending.set(None);
        };

        OverlayPopup::new()
            .on_close(move |_| pending.set(None))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(DIALOG_W))
                            .max_width(Size::window_percent(90.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(CARD_BG)
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text("Install into which world?")
                                    .font_size(16.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text("Data packs only apply to the world they are added to.")
                                    .font_size(12.)
                                    .color(colors::fg_secondary()),
                            )
                            .child(control)
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .on_press(move |_| pending.set(None))
                                            .text("Cancel"),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .enabled(world.is_some())
                                            .on_press(install)
                                            .child(Icon::new(IconType::Download01).size(14.))
                                            .text("Install"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
