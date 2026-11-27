use std::collections::HashMap;
use std::path::PathBuf;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::{DataPackInfo, PackIcon};

use crate::components::{CardLayout, ContextMenu, Dropdown, Icon, IconType};
use crate::hooks::{
    add_world_datapacks, delete_world_datapack, query_is_loading, spawn_world_task,
    try_cluster_worlds, try_world_datapacks, use_cluster, use_cluster_worlds, use_datapack_world,
    use_dispatch, use_world_datapacks,
};
use crate::i18n::{t, tf};
use crate::layout::cluster_content;
use crate::theme::colors;
use crate::{Actions, Route};

use super::cluster_not_found;
use super::folder_list::{
    CONTROL_H, CardIcon, FolderCard, confirm_dialog, content_box, content_empty, content_loading,
    content_toolbar, layout_toggle, matches_search, notify_in_use, open_folder_action,
    open_folder_button, toolbar_action, toolbar_secondary, use_content_view,
    use_game_folder_in_use,
};
use super::package_manager::copy_list_button;

fn copy_datapack_list(dispatch: crate::Actions, mut names: Vec<String>) -> Element {
    names.sort_by_key(|name| name.to_lowercase());
    copy_list_button(
        dispatch,
        names.len(),
        t("oneclient.cluster.datapacks.noun.one"),
        t("oneclient.cluster.datapacks.noun.other"),
        move || {
            names
                .iter()
                .map(|name| format!("- {name}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
    )
}

const WORLD_PICKER_W: f32 = 180.;

#[derive(PartialEq)]
pub struct ClusterDataPacks {
    pub cluster_id: i64,
}

fn pick_and_add(cluster_id: i64, world: String, dispatch: Actions) {
    spawn(async move {
        let Some(handles) = rfd::AsyncFileDialog::new()
            .set_title(t("oneclient.cluster.datapacks.picker.title"))
            .add_filter(t("oneclient.cluster.datapacks.picker.filter"), &["zip"])
            .pick_files()
            .await
        else {
            return;
        };
        let files = handles.iter().map(|h| h.path().to_path_buf()).collect();
        spawn_world_task(
            dispatch,
            t("oneclient.cluster.datapacks.add_failed"),
            add_world_datapacks(cluster_id, world, files),
        );
    });
}

fn browse(cluster_id: i64, world: String, mut remembered: State<HashMap<i64, String>>) {
    remembered.write().insert(cluster_id, world);
    let _ = RouterContext::get().push(Route::Browser {
        cluster_id,
        package_type: "datapack".to_string(),
        pick_cluster: false,
        in_instance: true,
    });
}

impl Component for ClusterDataPacks {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let cluster = use_cluster(cluster_id);

        let worlds_query = use_cluster_worlds(cluster_id);
        let worlds = try_cluster_worlds(&worlds_query).unwrap_or_default();
        let names: Vec<String> = worlds.iter().map(|w| w.folder_name.clone()).collect();

        let mut remembered = use_datapack_world();
        let current = remembered
            .read()
            .get(&cluster_id)
            .filter(|name| names.contains(name))
            .cloned()
            .or_else(|| names.first().cloned());

        let packs_query = use_world_datapacks(cluster_id, current.clone().unwrap_or_default());
        let dispatch = use_dispatch();
        let in_use = use_game_folder_in_use(cluster.as_ref());
        let search = use_state(String::new);
        let layout = use_content_view().layout;
        let mut menu = use_state(|| None::<(f32, f32, DataPackInfo)>);
        let mut pending_delete = use_state(|| None::<String>);

        let Some(cluster) = cluster else {
            return cluster_not_found();
        };
        let card_layout = CardLayout::from(*layout.read());

        let Some(world) = current else {
            let empty = if query_is_loading(&worlds_query) {
                content_loading()
            } else {
                content_empty(
                    IconType::Database01,
                    t("oneclient.cluster.datapacks.no_worlds.title"),
                    Some(t("oneclient.cluster.datapacks.no_worlds.hint").to_string()),
                    vec![
                        toolbar_secondary(
                            IconType::Globe01,
                            t("oneclient.cluster.datapacks.no_worlds.worlds"),
                        )
                        .on_press(move |_| {
                            let _ =
                                RouterContext::get().replace(Route::ClusterWorlds { cluster_id });
                        })
                        .into_element(),
                    ],
                )
            };
            return cluster_content()
                .child(content_box(
                    0,
                    card_layout,
                    |_| false,
                    |_| rect().into_element(),
                    Some(empty),
                    Vec::new(),
                ))
                .into_element();
        };

        let folder: Option<PathBuf> = cluster
            .game_dir()
            .ok()
            .map(|d| d.join("saves").join(&world).join("datapacks"));

        let all = try_world_datapacks(&packs_query).unwrap_or_default();
        let needle = search.read().trim().to_lowercase();
        let packs: Vec<DataPackInfo> = all
            .iter()
            .filter(|p| {
                matches_search(
                    &needle,
                    &[&p.file_name, p.description.as_deref().unwrap_or_default()],
                )
            })
            .cloned()
            .collect();

        let row = {
            let packs = packs.clone();
            move |i: usize| {
                let info = packs[i].clone();
                let menu_info = info.clone();
                FolderCard {
                    icon: match info.icon.clone() {
                        Some(PackIcon::File(path)) => CardIcon::Image(path),
                        Some(PackIcon::Cached(url)) => CardIcon::Cached(url, IconType::Database01),
                        None => CardIcon::Symbol(IconType::Database01),
                    },
                    title: info.file_name.clone(),
                    meta: if info.is_dir {
                        t("oneclient.cluster.datapacks.badge.folder")
                    } else {
                        t("oneclient.cluster.datapacks.badge.zip")
                    }
                    .to_string(),
                    description: info.description.clone(),
                    size: info.size_bytes,
                    layout: card_layout,
                    on_press: None,
                    on_context: (move |(x, y)| menu.set(Some((x, y, menu_info.clone())))).into(),
                }
                .into_element()
            }
        };

        let empty = if query_is_loading(&packs_query) {
            content_loading()
        } else if all.is_empty() {
            let add_world = world.clone();
            let add_dispatch = dispatch.clone();
            let browse_world = add_world.clone();
            content_empty(
                IconType::Database01,
                t("oneclient.cluster.datapacks.empty.title"),
                Some(t("oneclient.cluster.datapacks.empty.hint").to_string()),
                vec![
                    toolbar_action(
                        IconType::SearchMd,
                        t("oneclient.cluster.package_manager.browse_content"),
                    )
                    .on_press(move |_| browse(cluster_id, browse_world.clone(), remembered))
                    .into_element(),
                    toolbar_secondary(
                        IconType::FilePlus02,
                        t("oneclient.cluster.package_manager.add_from_file"),
                    )
                    .on_press(move |_| {
                        pick_and_add(cluster_id, add_world.clone(), add_dispatch.clone())
                    })
                    .into_element(),
                    open_folder_button(folder.clone()),
                ],
            )
        } else {
            content_empty(
                IconType::SearchMd,
                t("oneclient.cluster.datapacks.search_empty.title"),
                Some(t("oneclient.cluster.package_manager.search_empty.hint").to_string()),
                Vec::new(),
            )
        };

        let picker_names = names.clone();
        let picker = Dropdown::new(world.clone(), names)
            .outlined()
            .solid_menu()
            .width(Size::px(WORLD_PICKER_W))
            .height(Size::px(CONTROL_H))
            .leading(
                Icon::new(IconType::Globe01)
                    .size(14.)
                    .color(colors::fg_secondary()),
            )
            .on_select(move |idx: usize| {
                if let Some(name) = picker_names.get(idx) {
                    remembered.write().insert(cluster_id, name.clone());
                }
            })
            .into_element();

        let add_world = world.clone();
        let add_dispatch = dispatch.clone();
        let mut controls = vec![copy_datapack_list(
            dispatch.clone(),
            all.iter().map(|p| p.file_name.clone()).collect(),
        )];
        controls.push(open_folder_action(
            folder.clone(),
            tf("oneclient.cluster.folder.tip.datapacks", &[&world]),
        ));
        let browse_world = world.clone();
        controls.push(
            toolbar_secondary(IconType::SearchMd, t("oneclient.common.browse"))
                .on_press(move |_| browse(cluster_id, browse_world.clone(), remembered))
                .into_element(),
        );
        controls.push(
            toolbar_action(IconType::Plus, t("oneclient.cluster.datapacks.add"))
                .on_press(move |_| {
                    pick_and_add(cluster_id, add_world.clone(), add_dispatch.clone())
                })
                .into_element(),
        );

        let menu_overlay = menu.read().clone().map(|(x, y, info)| {
            let delete_name = info.file_name.clone();
            let reveal = folder.clone();
            ContextMenu::new(x, y)
                .title(info.file_name.clone())
                .action(
                    IconType::Folder,
                    t("oneclient.cluster.datapacks.menu.open_folder"),
                    move |()| {
                        if let Some(dir) = &reveal {
                            crate::platform::open_path(&dir.to_string_lossy());
                        }
                    },
                )
                .separator()
                .danger_action(IconType::Trash01, t("oneclient.common.delete"), {
                    let dispatch = dispatch.clone();
                    move |()| {
                        if in_use {
                            notify_in_use(&dispatch, t("oneclient.cluster.datapacks.in_use"));
                        } else {
                            pending_delete.set(Some(delete_name.clone()));
                        }
                    }
                })
                .on_close(move |_| menu.set(None))
                .into_element()
        });

        let confirm_overlay = pending_delete.read().clone().map(|file_name| {
            let delete_world = world.clone();
            confirm_dialog(
                tf("oneclient.cluster.datapacks.delete.title", &[&file_name]),
                tf("oneclient.cluster.datapacks.delete.body", &[&world]),
                move || pending_delete.set(None),
                {
                    let dispatch = dispatch.clone();
                    move || {
                        pending_delete.set(None);
                        if in_use {
                            notify_in_use(&dispatch, t("oneclient.cluster.datapacks.in_use"));
                            return;
                        }
                        spawn_world_task(
                            dispatch.clone(),
                            t("oneclient.cluster.datapacks.delete_failed"),
                            delete_world_datapack(
                                cluster_id,
                                delete_world.clone(),
                                file_name.clone(),
                            ),
                        );
                    }
                },
            )
        });

        cluster_content()
            .child(content_toolbar(
                Some(search),
                vec![picker, layout_toggle(layout)],
                controls,
            ))
            .child(content_box(
                packs.len(),
                card_layout,
                |i| {
                    packs[i]
                        .description
                        .as_deref()
                        .is_some_and(|d| !d.trim().is_empty())
                },
                row,
                Some(empty),
                Vec::new(),
            ))
            .maybe_child(menu_overlay)
            .maybe_child(confirm_overlay)
            .into_element()
    }
}
