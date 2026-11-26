use std::collections::HashMap;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::WorldInfo;

use crate::components::{CardLayout, ContextMenu, IconType};
use crate::hooks::{
    delete_world, query_is_loading, spawn_world_task, try_cluster_worlds, try_world_size,
    use_cluster, use_cluster_worlds, use_datapack_world, use_dispatch, use_saves_folder_watch,
    use_world_size,
};
use crate::i18n::{t, tf};
use crate::layout::cluster_content;
use crate::routes::Route;
use crate::ui::fmt_date;

use super::cluster_not_found;
use super::folder_list::{
    CardIcon, FolderCard, confirm_dialog, content_box, content_empty, content_loading,
    content_toolbar, layout_toggle, matches_search, notify_in_use, open_folder_action,
    open_folder_button, scope_chip, supports_datapacks, use_content_view, use_game_folder_in_use,
};

#[derive(PartialEq)]
pub struct ClusterWorlds {
    pub cluster_id: i64,
}

fn open_datapacks(cluster_id: i64, world: String, mut remembered: State<HashMap<i64, String>>) {
    remembered.write().insert(cluster_id, world);
    let _ = RouterContext::get().push(Route::ClusterDataPacks { cluster_id });
}

impl Component for ClusterWorlds {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let cluster = use_cluster(cluster_id);
        let saves = cluster
            .as_ref()
            .and_then(|c| c.game_dir().ok())
            .map(|d| d.join("saves"));

        let query = use_cluster_worlds(cluster_id);
        use_saves_folder_watch(saves.clone(), query);
        let dispatch = use_dispatch();
        let in_use = use_game_folder_in_use(cluster.as_ref());
        let remembered = use_datapack_world();
        let search = use_state(String::new);
        let layout = use_content_view().layout;
        let mut menu = use_state(|| None::<(f32, f32, WorldInfo)>);
        let mut pending_delete = use_state(|| None::<String>);

        let Some(cluster) = cluster else {
            return cluster_not_found();
        };
        let shared = !cluster.uses_dedicated_dir();
        let datapacks = supports_datapacks(&cluster.mc_version);

        let all = try_cluster_worlds(&query).unwrap_or_default();
        let needle = search.read().trim().to_lowercase();
        let worlds: Vec<WorldInfo> = all
            .iter()
            .filter(|w| matches_search(&needle, &[&w.folder_name]))
            .cloned()
            .collect();
        let card_layout = CardLayout::from(*layout.read());

        let row = {
            let worlds = worlds.clone();
            move |i: usize| {
                let info = worlds[i].clone();
                let press_world = info.folder_name.clone();
                let menu_info = info.clone();
                WorldCard {
                    cluster_id,
                    info,
                    layout: card_layout,
                    on_press: datapacks.then(|| {
                        (move |()| open_datapacks(cluster_id, press_world.clone(), remembered))
                            .into()
                    }),
                    on_context: (move |(x, y)| menu.set(Some((x, y, menu_info.clone())))).into(),
                }
                .into_element()
            }
        };

        let empty = if query_is_loading(&query) {
            content_loading()
        } else if all.is_empty() {
            content_empty(
                IconType::Globe01,
                t("oneclient.cluster.worlds.empty.title"),
                Some(t("oneclient.cluster.worlds.empty.hint").to_string()),
                vec![open_folder_button(saves.clone())],
            )
        } else {
            content_empty(
                IconType::SearchMd,
                t("oneclient.cluster.worlds.search_empty.title"),
                Some(t("oneclient.cluster.package_manager.search_empty.hint").to_string()),
                Vec::new(),
            )
        };

        // A shared game folder is a standing fact, so it is a chip rather than a banner
        let mut controls = Vec::new();
        if shared {
            controls.push(scope_chip(
                true,
                t("oneclient.cluster.worlds.shared_notice").to_string(),
            ));
        }
        controls.push(open_folder_action(
            saves,
            t("oneclient.cluster.folder.tip.worlds").to_string(),
        ));

        let menu_overlay = menu.read().clone().map(|(x, y, info)| {
            let open_world = info.folder_name.clone();
            let target_world = info.folder_name.clone();
            let path = info.path.clone();
            let mut context = ContextMenu::new(x, y).title(info.folder_name.clone());
            if datapacks {
                context = context.action(
                    IconType::Database01,
                    t("oneclient.cluster.worlds.menu.datapacks"),
                    move |()| open_datapacks(cluster_id, open_world.clone(), remembered),
                );
            }
            context
                .action(
                    IconType::Folder,
                    t("oneclient.cluster.datapacks.menu.open_folder"),
                    move |()| crate::platform::open_path(&path.to_string_lossy()),
                )
                .separator()
                .danger_action(IconType::Trash01, t("oneclient.common.delete"), {
                    let dispatch = dispatch.clone();
                    move |()| {
                        if in_use {
                            notify_in_use(&dispatch, t("oneclient.cluster.worlds.in_use"));
                        } else {
                            pending_delete.set(Some(target_world.clone()));
                        }
                    }
                })
                .on_close(move |_| menu.set(None))
                .into_element()
        });

        let confirm_overlay = pending_delete.read().clone().map(|world| {
            let target_world = world.clone();
            let body = if shared {
                t("oneclient.cluster.worlds.delete.body.shared")
            } else {
                t("oneclient.cluster.worlds.delete.body")
            };
            confirm_dialog(
                tf("oneclient.cluster.datapacks.delete.title", &[&world]),
                body.to_string(),
                move || pending_delete.set(None),
                {
                    let dispatch = dispatch.clone();
                    move || {
                        pending_delete.set(None);
                        if in_use {
                            notify_in_use(&dispatch, t("oneclient.cluster.worlds.in_use"));
                            return;
                        }
                        spawn_world_task(
                            dispatch.clone(),
                            t("oneclient.cluster.worlds.delete_failed"),
                            delete_world(cluster_id, target_world.clone()),
                        );
                    }
                },
            )
        });

        cluster_content()
            .child(content_toolbar(
                Some(search),
                vec![layout_toggle(layout)],
                controls,
            ))
            .child(content_box(
                worlds.len(),
                card_layout,
                |_| false,
                row,
                Some(empty),
                Vec::new(),
            ))
            .maybe_child(menu_overlay)
            .maybe_child(confirm_overlay)
            .into_element()
    }
}

#[derive(PartialEq)]
struct WorldCard {
    cluster_id: i64,
    info: WorldInfo,
    layout: CardLayout,
    on_press: Option<EventHandler<()>>,
    on_context: EventHandler<(f32, f32)>,
}

/// A world in the shared content row and card, sized once its folder has been measured
impl Component for WorldCard {
    fn render(&self) -> impl IntoElement {
        let info = &self.info;
        let size = try_world_size(&use_world_size(self.cluster_id, info.folder_name.clone()));

        FolderCard {
            icon: info
                .icon
                .clone()
                .map_or(CardIcon::Symbol(IconType::Globe01), CardIcon::Image),
            title: info.folder_name.clone(),
            meta: tf(
                "oneclient.cluster.worlds.last_played",
                &[&fmt_date(info.last_played)],
            ),
            description: None,
            size: size.unwrap_or_default(),
            layout: self.layout,
            on_press: self.on_press.clone(),
            on_context: self.on_context.clone(),
        }
    }
}
