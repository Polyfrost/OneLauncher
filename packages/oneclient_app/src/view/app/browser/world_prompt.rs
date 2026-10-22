use freya::prelude::*;
use oneclient_content::packages::ProviderId;

use crate::components::{Button, Dropdown, Icon, IconType};
use crate::hooks::{
    query_is_loading, try_cluster_worlds, use_cluster_worlds, use_datapack_world, use_dispatch,
};
use crate::theme::colors;
use crate::view::app::cluster::dialog;

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

        let world = remembered
            .read()
            .get(&cluster_id)
            .filter(|name| names.contains(name))
            .cloned()
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
                            remembered.write().insert(cluster_id, name.clone());
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

        dialog(
            "Install into which world?".to_string(),
            "Data packs only apply to the world they are added to.".to_string(),
            Some(control),
            move || pending.set(None),
            [
                Button::new()
                    .secondary()
                    .on_press(move |_| pending.set(None))
                    .text("Cancel")
                    .into_element(),
                Button::new()
                    .primary()
                    .enabled(world.is_some())
                    .on_press(install)
                    .child(Icon::new(IconType::Download01).size(14.))
                    .text("Install")
                    .into_element(),
            ],
        )
    }
}
