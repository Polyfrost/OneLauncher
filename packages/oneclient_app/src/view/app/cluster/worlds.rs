use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::WorldInfo;

use crate::components::{
    CARD_BG, CARD_NAME, CardLayout, ContextMenu, IconType, kebab_button, meta_size, meta_text,
    on_secondary,
};
use crate::hooks::{
    delete_world, query_is_loading, settled_or_loading, spawn_world_task, try_cluster_worlds,
    try_world_size, use_cluster, use_cluster_worlds, use_clusters, use_dispatch, use_game_snapshot,
    use_saves_folder_watch, use_view_state, use_world_size,
};
use crate::layout::cluster_content;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::{border_all_color, fmt_date};
use crate::utils::format_size;

use super::cluster_not_found;
use super::folder_list::{
    CardIcon, RowHeights, card_icon, confirm_dialog, content_box, folder_button, layout_toggle,
    matches_search, search_input, supports_datapacks, toolbar_panel,
};
use super::package_manager::{empty_hint, empty_shell, empty_title};

const LIST_ICON: f32 = 44.;
const GRID_ICON: f32 = 40.;
const LIST_PAD: f32 = 10.;
const GRID_PAD: f32 = 14.;

const SHARED_NOTICE: &str = "This version uses the shared game folder, so these worlds also appear in every other version that uses it.";

const WORLD_ROWS: RowHeights = RowHeights {
    list: LIST_ICON + 2. * LIST_PAD,
    grid: GRID_ICON + 2. * GRID_PAD,
};

#[derive(PartialEq)]
pub struct ClusterWorlds {
    pub cluster_id: i64,
}

fn notify_in_use(dispatch: &crate::Actions) {
    dispatch
        .notify("Close Minecraft first")
        .body("Worlds can't be deleted while a version using this game folder is running.")
        .error()
        .send();
}

fn open_datapacks(cluster_id: i64, world: String) {
    let _ = RouterContext::get().push(Route::ClusterDataPacks { cluster_id, world });
}

impl Component for ClusterWorlds {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let Some(cluster) = use_cluster(cluster_id) else {
            return cluster_not_found();
        };
        let saves = cluster.game_dir().ok().map(|d| d.join("saves"));
        let shared = !cluster.uses_dedicated_dir();
        let datapacks = supports_datapacks(&cluster.mc_version);

        let query = use_cluster_worlds(cluster_id);
        use_saves_folder_watch(saves.clone(), query);
        let dispatch = use_dispatch();
        let game = use_game_snapshot();
        let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
        let in_use = game.is_active(cluster_id)
            || (shared
                && clusters
                    .iter()
                    .any(|c| game.is_active(c.id) && !c.uses_dedicated_dir()));
        let search = use_state(String::new);
        let layout = use_view_state("cluster.worlds").layout;
        let mut menu = use_state(|| None::<(f32, f32, WorldInfo)>);
        let mut pending_delete = use_state(|| None::<String>);

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
                        (move |()| open_datapacks(cluster_id, press_world.clone())).into()
                    }),
                    on_context: (move |(x, y)| menu.set(Some((x, y, menu_info.clone())))).into(),
                }
                .into_element()
            }
        };

        let empty = if query_is_loading(&query) {
            None
        } else if all.is_empty() {
            Some(
                empty_shell(IconType::Globe01)
                    .child(empty_title("No worlds yet."))
                    .child(empty_hint(
                        "Worlds show up here once you create one in game.",
                    ))
                    .into_element(),
            )
        } else {
            Some(
                empty_shell(IconType::SearchMd)
                    .child(empty_title("No worlds match your search."))
                    .child(empty_hint("Try a different term."))
                    .into_element(),
            )
        };

        let mut controls = vec![search_input(search)];
        controls.extend(saves.map(folder_button));
        controls.push(layout_toggle(layout));

        let menu_overlay = menu.read().clone().map(|(x, y, info)| {
            let open_world = info.folder_name.clone();
            let target_world = info.folder_name.clone();
            let path = info.path.clone();
            let mut context = ContextMenu::new(x, y).title(info.folder_name.clone());
            if datapacks {
                context = context.action(IconType::Database01, "Data packs", move |()| {
                    open_datapacks(cluster_id, open_world.clone())
                });
            }
            context
                .action(IconType::Folder, "Open folder", move |()| {
                    crate::platform::open_path(&path.to_string_lossy())
                })
                .separator()
                .danger_action(IconType::Trash01, "Delete", {
                    let dispatch = dispatch.clone();
                    move |()| {
                        if in_use {
                            notify_in_use(&dispatch);
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
                "This version uses the shared game folder, so the world is removed from every version that uses it. It can be restored from your system trash."
            } else {
                "It can be restored from your system trash."
            };
            confirm_dialog(
                format!("Move \"{world}\" to trash?"),
                body.to_string(),
                move || pending_delete.set(None),
                {
                    let dispatch = dispatch.clone();
                    move || {
                        pending_delete.set(None);
                        if in_use {
                            notify_in_use(&dispatch);
                            return;
                        }
                        spawn_world_task(
                            dispatch.clone(),
                            "Couldn't delete world",
                            delete_world(cluster_id, target_world.clone()),
                        );
                    }
                },
            )
        });

        cluster_content()
            .child(toolbar_panel(None, controls))
            .child(content_box(
                worlds.len(),
                card_layout,
                WORLD_ROWS,
                row,
                empty,
                shared
                    .then(|| SHARED_NOTICE.to_string())
                    .into_iter()
                    .collect(),
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

impl Component for WorldCard {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let info = &self.info;
        let grid = self.layout == CardLayout::Grid;
        let size = try_world_size(&use_world_size(self.cluster_id, info.folder_name.clone()));

        let icon = card_icon(
            &info
                .icon
                .clone()
                .map_or(CardIcon::Symbol(IconType::Globe01), CardIcon::Image),
            if grid { GRID_ICON } else { LIST_ICON },
        );
        let last_played = format!("Last played {}", fmt_date(info.last_played));
        let on_press = self.on_press.clone();
        let pressable = on_press.is_some();

        let text = if grid {
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    label()
                        .text(info.folder_name.clone())
                        .font_size(14.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .width(Size::fill())
                        .color(Color::WHITE),
                )
                .child(meta_text(
                    match size {
                        Some(size) => format!("{last_played} \u{b7} {}", format_size(size)),
                        None => last_played.clone(),
                    },
                    CARD_NAME.with_a(127),
                ))
        } else {
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    label()
                        .text(info.folder_name.clone())
                        .font_size(15.)
                        .font_weight(FontWeight::MEDIUM)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .width(Size::fill())
                        .color(CARD_NAME),
                )
                .child(
                    label()
                        .text(last_played)
                        .font_size(11.)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .width(Size::fill())
                        .color(colors::fg_secondary()),
                )
        };

        let hovering = *hovered.read();
        let card = rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .content(Content::Flex)
            .maybe(pressable, |el| el.cursor(CursorIcon::Pointer))
            .on_press(move |_| {
                if let Some(handler) = &on_press {
                    handler.call(());
                }
            })
            .on_secondary_down(on_secondary(Some(self.on_context.clone())));

        if grid {
            card.height(Size::px(WORLD_ROWS.grid))
                .spacing(11.)
                .padding(Gaps::new_all(GRID_PAD))
                .corner_radius(CornerRadius::new_all(6.))
                .background(if hovering {
                    colors::component_bg_hover()
                } else {
                    colors::component_bg()
                })
                .border(border_all_color(
                    1.,
                    if hovering {
                        colors::component_border_hover()
                    } else {
                        colors::component_border()
                    },
                ))
                .overflow(Overflow::Clip)
                .on_pointer_enter(move |_| hovered.set(true))
                .on_pointer_leave(move |_| hovered.set(false))
                .child(icon)
                .child(text)
                .child(kebab_button(self.on_context.clone()))
        } else {
            card.height(Size::px(WORLD_ROWS.list))
                .spacing(12.)
                .padding(Gaps::new_all(LIST_PAD))
                .corner_radius(CornerRadius::new_all(8.))
                .background(CARD_BG)
                .child(icon)
                .child(text)
                .child(meta_size(size.unwrap_or_default()))
                .child(kebab_button(self.on_context.clone()))
        }
    }
}
