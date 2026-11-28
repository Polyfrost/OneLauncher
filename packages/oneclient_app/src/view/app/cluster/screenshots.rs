use std::path::PathBuf;

use freya::prelude::*;
use oneclient_core::ScreenshotInfo;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Button, ContextMenu, GRID_GAP, Icon, IconType, LocalImage, OverlayPopup, ROW_GAP, ROW_ICON,
    ROW_RADIUS, ScreenshotViewer, joined_line, meta_size, row_line, row_surface, row_title,
    screenshot_context_menu,
};
use crate::hooks::{
    ScreenshotAction, Selection, query_is_loading, try_cluster_screenshots,
    use_cluster_screenshots, use_dispatch, use_screenshot_action, use_screenshot_folder_watch,
    use_selection,
};
use crate::i18n::{t, tf};
use crate::layout::cluster_content;
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, columns_for, flow_grid, fmt_date};
use crate::utils::{format_res, format_size};

use super::package_manager::instance_only_notice;

use super::cluster_not_found;
use super::folder_list::{
    CONTROL_H, SELECTION_COUNT_W, compact_selection, content_area, content_empty, content_loading,
    content_scroll, content_toolbar, layout_toggle, matches_search, open_folder_action,
    open_folder_button, scope_chip, selection_icon, toolbar_secondary, use_content_view,
};
use crate::hooks::use_cluster;

const THUMB_EDGE: u32 = 480;
/// Narrowest screenshot tile; tiles grow to fill the row
const TILE_MIN_W: f32 = 240.;
const TILE_MAX_COLS: usize = 6;
/// List thumbnails keep the 16:9 shape of the tiles at the row's icon height
const LIST_THUMB_W: f32 = ROW_ICON * 16. / 9.;

/// Preview height of a 16:9 tile when `cols` tiles share `width`
fn tile_preview_height(width: f32, cols: usize, gap: f32) -> f32 {
    let cols = cols.max(1) as f32;
    let col_w = ((width - gap * (cols - 1.)) / cols).max(0.);
    col_w * 9. / 16.
}

#[derive(PartialEq)]
pub struct ClusterScreenshots {
    pub cluster_id: i64,
}

impl Component for ClusterScreenshots {
    fn render(&self) -> impl IntoElement {
        let cluster = use_cluster(self.cluster_id);
        let folder = cluster
            .as_ref()
            .and_then(|c| c.game_dir().ok())
            .map(|d| d.join("screenshots"));

        let query = use_cluster_screenshots(self.cluster_id);
        // Otherwise a screenshot taken in game only shows up on the next visit
        use_screenshot_folder_watch(folder.clone(), query);
        let action = use_screenshot_action();

        let all = try_cluster_screenshots(&query).unwrap_or_default();

        let view_mode = use_content_view().layout;
        let menu_dispatch = use_dispatch();
        let search = use_state(String::new);

        let selection = use_selection::<PathBuf>();
        let mut viewing = use_state(|| None::<usize>);
        let mut confirm_delete = use_state(|| false);
        let grid_width = use_state(|| 0f32);
        let mut menu = use_state(|| None::<(f32, f32, PathBuf)>);

        let Some(cluster) = cluster else {
            return cluster_not_found();
        };

        let needle = search.read().trim().to_lowercase();
        let shots: Vec<ScreenshotInfo> = all
            .iter()
            .filter(|shot| matches_search(&needle, &[&shot.name]))
            .cloned()
            .collect();
        let order: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
        let selected = selection.selected_in(&order);
        let editing = selection.is_active();

        let actions = toolbar_actions(
            &shots,
            !cluster.uses_dedicated_dir(),
            folder.clone(),
            editing,
            selection,
            selected.len(),
            confirm_delete,
        );
        let toolbar = content_toolbar(Some(search), vec![layout_toggle(view_mode)], actions);

        let content: Element = if shots.is_empty() {
            if query_is_loading(&query) {
                content_loading()
            } else if all.is_empty() {
                content_empty(
                    IconType::Eye,
                    t("oneclient.cluster.screenshots.no_images"),
                    Some(t("oneclient.cluster.screenshots.empty").to_string()),
                    vec![open_folder_button(folder)],
                )
            } else {
                content_empty(
                    IconType::SearchMd,
                    tf(
                        "oneclient.cluster.package_manager.search_empty.title",
                        &[&t("oneclient.cluster.screenshots.title").to_lowercase()],
                    ),
                    Some(t("oneclient.cluster.package_manager.search_empty.hint").to_string()),
                    Vec::new(),
                )
            }
        } else {
            let grid = *view_mode.read() == ViewLayout::Grid;
            let cols = columns_for(*grid_width.read(), TILE_MIN_W, TILE_MAX_COLS, GRID_GAP);
            let preview_h = tile_preview_height(*grid_width.read(), cols, GRID_GAP);
            let mut items: Vec<Element> = Vec::new();
            for (idx, info) in shots.iter().enumerate() {
                let info = info.clone();
                let is_selected = selected.contains(&info.path);

                let activate_path = info.path.clone();
                let activate_order = order.clone();
                let on_activate = move |_| {
                    if editing || selection.modifier_held() {
                        selection.click(activate_path.clone(), &activate_order);
                    } else {
                        viewing.set(Some(idx));
                    }
                };

                let ctx_path = info.path.clone();
                let on_context: EventHandler<(f32, f32)> = (move |(x, y)| {
                    menu.set(Some((x, y, ctx_path.clone())));
                })
                .into();

                items.push(if grid {
                    ScreenshotTile {
                        info,
                        selected: is_selected,
                        preview_h,
                        on_activate: on_activate.into(),
                        on_context,
                    }
                    .into_element()
                } else {
                    ScreenshotRow {
                        info,
                        selected: is_selected,
                        on_activate: on_activate.into(),
                        on_context,
                    }
                    .into_element()
                });
            }

            let list = if grid {
                flow_grid(items, cols, grid_width, GRID_GAP)
            } else {
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(ROW_GAP)
                    .children(items)
                    .into_element()
            };
            content_scroll().children(vec![list]).into_element()
        };

        let confirm_overlay = confirm_delete.read().then(|| {
            let paths = selected.clone();
            confirm_panel(paths.len(), confirm_delete, move || {
                for path in paths.iter().cloned() {
                    action.mutate(ScreenshotAction::Delete { path });
                }
                selection.exit();
                confirm_delete.clone().set(false);
            })
        });

        let menu_overlay = menu.read().clone().map(|(x, y, path)| {
            let menu_for = if !editing {
                let select_path = path.clone();
                screenshot_context_menu(
                    x,
                    y,
                    path,
                    action,
                    menu_dispatch.clone(),
                    |()| {},
                    Some((move |()| selection.toggle(select_path.clone())).into()),
                )
            } else {
                let all = order.clone();
                let every = !selected.is_empty() && selected.len() == order.len();
                let (icon, text) = if every {
                    (
                        IconType::XClose,
                        t("oneclient.cluster.package_manager.select.unselect_all"),
                    )
                } else {
                    (
                        IconType::Check,
                        t("oneclient.cluster.package_manager.select.select_all"),
                    )
                };
                let own = if selected.contains(&path) {
                    (
                        IconType::XClose,
                        t("oneclient.cluster.package_manager.select.unselect"),
                    )
                } else {
                    (
                        IconType::Check,
                        t("oneclient.cluster.package_manager.select.select"),
                    )
                };
                let mut multi = ContextMenu::new(x, y)
                    .title(tf(
                        "oneclient.cluster.package_manager.select.count",
                        &[&selected.len()],
                    ))
                    .action(own.0, own.1, move |()| selection.toggle(path.clone()))
                    .separator()
                    .action(icon, text, move |()| selection.toggle_all(&all));
                if !selected.is_empty() && !every {
                    multi = multi.action(
                        IconType::XClose,
                        t("oneclient.cluster.package_manager.select.clear"),
                        move |()| selection.clear(),
                    );
                }
                if !selected.is_empty() {
                    multi = multi.separator().danger_action(
                        IconType::Trash01,
                        tf(
                            "oneclient.cluster.package_manager.select.delete_count",
                            &[&selected.len()],
                        ),
                        move |()| confirm_delete.set(true),
                    );
                }
                multi
            };
            menu_for.on_close(move |_| menu.set(None)).into_element()
        });

        cluster_content()
            .child(
                selection
                    .track_modifiers(rect())
                    .vertical()
                    .width(Size::fill())
                    .height(Size::fill())
                    .child(toolbar)
                    .child(content_area(Vec::new()).child(content)),
            )
            .maybe_child((*viewing.read()).map(|start| {
                ScreenshotViewer::new(shots.clone(), start, move |_| viewing.clone().set(None))
                    .into_element()
            }))
            .maybe_child(confirm_overlay)
            .maybe_child(menu_overlay)
            .into_element()
    }
}

fn toolbar_actions(
    shots: &[ScreenshotInfo],
    shared: bool,
    folder: Option<PathBuf>,
    editing: bool,
    selection: Selection<PathBuf>,
    count: usize,
    mut confirm_delete: State<bool>,
) -> Vec<Element> {
    let all_paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();

    if !editing {
        let noun = t("oneclient.cluster.screenshots.title").to_lowercase();
        let tip = if shared {
            t("oneclient.cluster.folder.tip.shared").to_string()
        } else {
            instance_only_notice(&noun)
        };
        let mut actions = vec![
            scope_chip(shared, tip),
            open_folder_action(
                folder,
                t("oneclient.layout.cluster.screenshots_folder").to_string(),
            ),
        ];
        if !shots.is_empty() {
            actions.push(
                toolbar_secondary(
                    IconType::Pencil01,
                    t("oneclient.cluster.package_manager.select.select"),
                )
                .on_press(move |_| selection.enter())
                .into_element(),
            );
        }
        return actions;
    }

    let every = count > 0 && count == all_paths.len();
    if compact_selection() {
        return vec![
            label()
                .text(tf(
                    "oneclient.cluster.package_manager.select.count",
                    &[&count],
                ))
                .width(Size::px(SELECTION_COUNT_W))
                .font_size(text_size(12.))
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(colors::fg_secondary())
                .into_element(),
            selection_icon(
                if every {
                    IconType::XClose
                } else {
                    IconType::Check
                },
                if every {
                    t("oneclient.cluster.package_manager.select.unselect_all")
                } else {
                    t("oneclient.cluster.package_manager.select.select_all")
                },
            )
            .on_press(move |_| selection.toggle_all(&all_paths))
            .into_element(),
            selection_icon(
                IconType::Minus,
                t("oneclient.cluster.screenshots.deselect_all"),
            )
            .ghost()
            .enabled(count > 0 && !every)
            .on_press(move |_| selection.clear())
            .into_element(),
            selection_icon(
                IconType::Trash01,
                tf("oneclient.cluster.package_manager.select.delete", &[&count]),
            )
            .danger()
            .enabled(count > 0)
            .on_press(move |_| confirm_delete.set(true))
            .into_element(),
            selection_icon(IconType::XClose, t("oneclient.common.cancel"))
                .ghost()
                .on_press(move |_| selection.exit())
                .into_element(),
        ];
    }
    let ghost = |text: &'static str| {
        Button::new()
            .ghost()
            .height(Size::px(CONTROL_H))
            .font_size(text_size(12.))
            .text(text)
    };
    let mut actions = vec![
        label()
            .text(tf(
                "oneclient.cluster.package_manager.select.count",
                &[&count],
            ))
            .width(Size::px(90.))
            .font_size(text_size(12.))
            .max_lines(1)
            .text_overflow(TextOverflow::Ellipsis)
            .color(colors::fg_secondary())
            .into_element(),
        Button::new()
            .secondary()
            .height(Size::px(CONTROL_H))
            .width(Size::px(103.))
            .font_size(text_size(12.))
            .on_press(move |_| selection.toggle_all(&all_paths))
            .text(if every {
                t("oneclient.cluster.package_manager.select.unselect_all")
            } else {
                t("oneclient.cluster.package_manager.select.select_all")
            })
            .into_element(),
        ghost(t("oneclient.cluster.screenshots.deselect_all"))
            .enabled(count > 0 && !every)
            .on_press(move |_| selection.clear())
            .into_element(),
    ];
    actions.push(
        Button::new()
            .danger()
            .height(Size::px(CONTROL_H))
            .width(Size::px(114.))
            .font_size(text_size(12.))
            .enabled(count > 0)
            .on_press(move |_| confirm_delete.set(true))
            .child(Icon::new(IconType::Trash01).size(15.))
            .text(tf(
                "oneclient.cluster.package_manager.select.delete",
                &[&count],
            ))
            .into_element(),
    );
    actions.push(
        ghost(t("oneclient.common.cancel"))
            .on_press(move |_| selection.exit())
            .into_element(),
    );
    actions
}

fn confirm_panel(
    count: usize,
    confirm: State<bool>,
    mut on_confirm: impl FnMut() + 'static,
) -> impl IntoElement {
    let mut cancel = confirm;

    OverlayPopup::new()
        .on_close(move |_| confirm.clone().set(false))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .vertical()
                        .width(Size::px(400.))
                        .max_width(Size::window_percent(90.))
                        .spacing(14.)
                        .padding(Gaps::new_all(20.))
                        .corner_radius(rounded(ROW_RADIUS))
                        .background(colors::card())
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            label()
                                .text(if count == 1 {
                                    tf("oneclient.cluster.screenshots.delete.title.one", &[&count])
                                } else {
                                    tf(
                                        "oneclient.cluster.screenshots.delete.title.other",
                                        &[&count],
                                    )
                                })
                                .font_size(text_size(16.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(t("oneclient.cluster.screenshots.delete.body"))
                                .font_size(text_size(12.))
                                .color(colors::fg_secondary()),
                        )
                        .child(
                            rect()
                                .horizontal()
                                .width(Size::fill())
                                .main_align(Alignment::End)
                                .spacing(8.)
                                .child(
                                    Button::new()
                                        .secondary()
                                        .on_press(move |_| cancel.set(false))
                                        .text(t("oneclient.common.cancel")),
                                )
                                .child(
                                    Button::new()
                                        .danger()
                                        .on_press(move |_| on_confirm())
                                        .child(Icon::new(IconType::Trash01).size(14.))
                                        .text(t("oneclient.common.delete")),
                                ),
                        ),
                ),
        )
        .into_element()
}

fn res_badge(res: Option<(u32, u32)>) -> Option<Element> {
    res.map(format_res).map(|text| {
        rect()
            .position(Position::new_absolute().bottom(6.).right(6.))
            .padding(Gaps::new_symmetric(2., 6.))
            .corner_radius(rounded(5.))
            .background(Color::from_argb(170, 0, 0, 0))
            .layer(Layer::Relative(3))
            .child(
                label()
                    .text(text)
                    .font_size(text_size(10.))
                    .color(Color::WHITE),
            )
            .into_element()
    })
}

fn thumbnail(path: PathBuf) -> LocalImage {
    LocalImage::new(path, THUMB_EDGE, true)
        .skeleton(true)
        .retained(true)
}

/// The brand outline a selected screenshot wears, the way selected package rows do
fn selection_outline(radius: f32) -> Element {
    rect()
        .position(Position::new_absolute().top(0.).left(0.))
        .width(Size::percent(100.))
        .height(Size::percent(100.))
        .layer(Layer::Relative(4))
        .corner_radius(rounded(radius))
        .background(colors::brand().with_a(28))
        .border(border_all_color(2., colors::brand()).alignment(BorderAlignment::Inner))
        .into_element()
}

#[derive(PartialEq)]
struct ScreenshotTile {
    info: ScreenshotInfo,
    selected: bool,
    preview_h: f32,
    on_activate: EventHandler<()>,
    on_context: EventHandler<(f32, f32)>,
}

impl Component for ScreenshotTile {
    fn render(&self) -> impl IntoElement {
        let info = self.info.clone();
        let mut hovered = use_state(|| false);

        let on_activate = self.on_activate.clone();
        let on_context = self.on_context.clone();

        let border_color = if *hovered.read() {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .corner_radius(rounded(ROW_RADIUS))
            .background(colors::card())
            .overflow(Overflow::Clip)
            .border(border_all_color(1., border_color).alignment(BorderAlignment::Inner))
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| on_activate.call(()))
            .on_secondary_down(move |e: Event<PressEventData>| {
                if let PressEventData::Mouse(m) = e.data() {
                    on_context.call((m.global_location.x as f32, m.global_location.y as f32));
                }
            })
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(self.preview_h))
                    .overflow(Overflow::Clip)
                    .background(colors::component_bg())
                    .child(thumbnail(info.path.clone()))
                    .maybe_child(res_badge(info.resolution)),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .padding(Gaps::new_symmetric(10., 12.))
                    .spacing(2.)
                    .child(row_title(info.name.clone(), colors::fg_primary()))
                    .child(row_line(
                        joined_line([
                            format_size(info.size_bytes).as_str(),
                            fmt_date(info.created).as_str(),
                        ]),
                        colors::fg_secondary(),
                    )),
            )
            .maybe_child(self.selected.then(|| selection_outline(ROW_RADIUS)))
    }
}

#[derive(PartialEq)]
struct ScreenshotRow {
    info: ScreenshotInfo,
    selected: bool,
    on_activate: EventHandler<()>,
    on_context: EventHandler<(f32, f32)>,
}

impl Component for ScreenshotRow {
    fn render(&self) -> impl IntoElement {
        let info = self.info.clone();
        let hovered = use_state(|| false);

        let on_activate = self.on_activate.clone();
        let on_context = self.on_context.clone();

        let thumb = rect()
            .width(Size::px(LIST_THUMB_W))
            .height(Size::px(ROW_ICON))
            .corner_radius(rounded(6.))
            .overflow(Overflow::Clip)
            .background(colors::component_bg())
            .child(thumbnail(info.path.clone()));
        let resolution = info.resolution.map(format_res).unwrap_or_default();

        rect()
            .width(Size::fill())
            .child(
                row_surface(hovered)
                    .cursor(CursorIcon::Pointer)
                    .on_press(move |_| on_activate.call(()))
                    .on_secondary_down(move |e: Event<PressEventData>| {
                        if let PressEventData::Mouse(m) = e.data() {
                            on_context
                                .call((m.global_location.x as f32, m.global_location.y as f32));
                        }
                    })
                    .child(thumb)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(4.)
                            .child(row_title(info.name.clone(), colors::fg_primary()))
                            .child(row_line(
                                joined_line([resolution.as_str(), fmt_date(info.created).as_str()]),
                                colors::fg_secondary(),
                            )),
                    )
                    .child(meta_size(info.size_bytes)),
            )
            .maybe_child(self.selected.then(|| selection_outline(ROW_RADIUS)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_keep_a_16_by_9_preview() {
        assert_eq!(tile_preview_height(160., 1, 12.), 90.);
        // Two 320 px columns with a 12 px gap between them
        assert_eq!(tile_preview_height(652., 2, 12.), 180.);
        assert_eq!(tile_preview_height(0., 3, 12.), 0.);
    }
}
