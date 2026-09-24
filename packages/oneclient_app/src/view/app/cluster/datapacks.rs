use std::path::PathBuf;

use freya::prelude::*;
use oneclient_core::DataPackInfo;

use crate::Actions;
use crate::components::{Button, CardLayout, ContextMenu, Dropdown, Icon, IconType};
use crate::hooks::{
    add_world_datapacks, delete_world_datapack, query_is_loading, spawn_world_task,
    try_cluster_worlds, try_world_datapacks, use_cluster, use_cluster_worlds, use_dispatch,
    use_view_state, use_world_datapacks,
};
use crate::layout::cluster_content;
use crate::theme::colors;
use crate::ui::fmt_date;

use super::cluster_not_found;
use super::folder_list::{
    CardIcon, FolderCard, PACKAGE_ROWS, confirm_dialog, content_box, folder_button, layout_toggle,
    matches_search, search_input, toolbar_action, toolbar_panel,
};
use super::package_manager::{empty_hint, empty_shell, empty_title};

const WORLD_PICKER_W: f32 = 220.;

#[derive(PartialEq)]
pub struct ClusterDataPacks {
    pub cluster_id: i64,
    pub world: String,
}

fn pick_and_add(cluster_id: i64, world: String, dispatch: Actions) {
    spawn(async move {
        let Some(handles) = rfd::AsyncFileDialog::new()
            .set_title("Select data packs to add")
            .add_filter("Data pack", &["zip"])
            .pick_files()
            .await
        else {
            return;
        };
        let files = handles.iter().map(|h| h.path().to_path_buf()).collect();
        spawn_world_task(
            dispatch,
            "Couldn't add data packs",
            add_world_datapacks(cluster_id, world, files),
        );
    });
}

impl Component for ClusterDataPacks {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let requested_world = self.world.clone();
        let cluster = use_cluster(cluster_id);

        let worlds_query = use_cluster_worlds(cluster_id);
        let worlds = try_cluster_worlds(&worlds_query).unwrap_or_default();
        let names: Vec<String> = worlds.iter().map(|w| w.folder_name.clone()).collect();

        let mut picked = use_state(|| None::<String>);
        let current = [picked.read().clone(), Some(requested_world)]
            .into_iter()
            .flatten()
            .find(|name| names.contains(name))
            .or_else(|| names.first().cloned());

        let packs_query = use_world_datapacks(cluster_id, current.clone().unwrap_or_default());
        let dispatch = use_dispatch();
        let search = use_state(String::new);
        let layout = use_view_state("cluster.datapacks").layout;
        let mut menu = use_state(|| None::<(f32, f32, DataPackInfo)>);
        let mut pending_delete = use_state(|| None::<String>);

        let Some(cluster) = cluster else {
            return cluster_not_found();
        };
        let card_layout = CardLayout::from(*layout.read());

        let Some(world) = current else {
            let empty = (!query_is_loading(&worlds_query)).then(|| {
                empty_shell(IconType::Globe01)
                    .child(empty_title("No worlds yet."))
                    .child(empty_hint(
                        "Data packs live inside worlds, so create a world in game first.",
                    ))
                    .into_element()
            });
            return cluster_content()
                .child(toolbar_panel(None, vec![layout_toggle(layout)]))
                .child(content_box(
                    0,
                    card_layout,
                    PACKAGE_ROWS,
                    |_| rect().into_element(),
                    empty,
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
                    icon: CardIcon::Symbol(IconType::Database01),
                    title: info.file_name.clone(),
                    badge: if info.is_dir {
                        (IconType::Folder, "Folder".to_string())
                    } else {
                        (IconType::File02, "Zip".to_string())
                    },
                    subtitle: format!("Modified {}", fmt_date(info.modified)),
                    description: info.description.clone(),
                    size: info.size_bytes,
                    layout: card_layout,
                    on_context: (move |(x, y)| menu.set(Some((x, y, menu_info.clone())))).into(),
                }
                .into_element()
            }
        };

        let empty = if query_is_loading(&packs_query) {
            None
        } else if all.is_empty() {
            let add_world = world.clone();
            let add_dispatch = dispatch.clone();
            Some(
                empty_shell(IconType::FilePlus02)
                    .child(empty_title("No data packs in this world yet."))
                    .child(empty_hint(
                        "Data packs only apply to the world they are added to.",
                    ))
                    .child(rect().height(Size::px(6.)))
                    .child(
                        Button::new()
                            .primary()
                            .on_press(move |_| {
                                pick_and_add(cluster_id, add_world.clone(), add_dispatch.clone())
                            })
                            .child(Icon::new(IconType::FilePlus02).size(14.))
                            .text("Add from file"),
                    )
                    .into_element(),
            )
        } else {
            Some(
                empty_shell(IconType::SearchMd)
                    .child(empty_title("No data packs match your search."))
                    .child(empty_hint("Try a different term."))
                    .into_element(),
            )
        };

        let picker_names = names.clone();
        let picker = Dropdown::new(world.clone(), names)
            .width(Size::px(WORLD_PICKER_W))
            .height(Size::px(34.))
            .leading(
                Icon::new(IconType::Globe01)
                    .size(14.)
                    .color(colors::fg_secondary()),
            )
            .on_select(move |idx: usize| {
                if let Some(name) = picker_names.get(idx) {
                    picked.set(Some(name.clone()));
                }
            })
            .into_element();

        let add_world = world.clone();
        let add_dispatch = dispatch.clone();
        let mut controls = vec![search_input(search)];
        controls.extend(folder.clone().map(folder_button));
        controls.push(layout_toggle(layout));
        controls.push(
            toolbar_action(IconType::Plus, "Add Data Packs")
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
                .action(IconType::Folder, "Open folder", move |()| {
                    if let Some(dir) = &reveal {
                        crate::platform::open_path(&dir.to_string_lossy());
                    }
                })
                .separator()
                .danger_action(IconType::Trash01, "Delete", move |()| {
                    pending_delete.set(Some(delete_name.clone()))
                })
                .on_close(move |_| menu.set(None))
                .into_element()
        });

        let confirm_overlay = pending_delete.read().clone().map(|file_name| {
            let delete_world = world.clone();
            confirm_dialog(
                format!("Move \"{file_name}\" to trash?"),
                format!(
                    "It is removed from \"{world}\" only and can be restored from your system trash."
                ),
                move || pending_delete.set(None),
                {
                    let dispatch = dispatch.clone();
                    move || {
                        pending_delete.set(None);
                        spawn_world_task(
                            dispatch.clone(),
                            "Couldn't delete data pack",
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
            .child(toolbar_panel(Some(picker), controls))
            .child(content_box(
                packs.len(),
                card_layout,
                PACKAGE_ROWS,
                row,
                empty,
                Vec::new(),
            ))
            .maybe_child(menu_overlay)
            .maybe_child(confirm_overlay)
            .into_element()
    }
}
