use std::path::PathBuf;

use freya::prelude::*;
use oneclient_core::ScreenshotInfo;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Button, ContextMenu, Icon, IconType, LocalImage, OverlayPopup, ScreenshotViewer, ScrollArea,
    Segment, SegmentedControl, open_folder_button, screenshot_context_menu,
};
use crate::hooks::{
    ScreenshotAction, Selection, query_is_loading, try_cluster_screenshots,
    use_cluster_screenshots, use_dispatch, use_screenshot_action, use_screenshot_folder_watch,
    use_selection, use_view_state,
};
use crate::layout::cluster_content;
use crate::theme::colors;
use crate::ui::{border_all_color, flow_grid, fmt_date, grid_columns_for_width};
use crate::utils::{format_res, format_size};

use super::cluster_not_found;
use crate::hooks::use_cluster;

const THUMB_EDGE: u32 = 480;
const MAX_COL_W: f32 = 400.;
const GRID_GAP: f32 = 16.;
const TILE_PREVIEW_H: f32 = 168.;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const CARD_NAME: Color = Color::from_rgb(213, 219, 255);

#[derive(PartialEq)]
pub struct ClusterScreenshots {
    pub cluster_id: i64,
}

impl Component for ClusterScreenshots {
    fn render(&self) -> impl IntoElement {
        let Some(cluster) = use_cluster(self.cluster_id) else {
            return cluster_not_found();
        };
        let folder = cluster.game_dir().ok().map(|d| d.join("screenshots"));

        let query = use_cluster_screenshots(self.cluster_id);
        // Otherwise a screenshot taken in game only shows up on the next visit
        use_screenshot_folder_watch(folder.clone(), query);
        let action = use_screenshot_action();

        let shots = try_cluster_screenshots(&query).unwrap_or_default();

        let view_mode = use_view_state("cluster.screenshots").layout;
        let menu_dispatch = use_dispatch();

        let selection = use_selection::<PathBuf>();
        let mut viewing = use_state(|| None::<usize>);
        let mut confirm_delete = use_state(|| false);
        let grid_width = use_state(|| 0f32);
        let mut menu = use_state(|| None::<(f32, f32, PathBuf)>);

        let order: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
        let selected = selection.selected_in(&order);
        let editing = selection.is_active();

        let toolbar = toolbar_row(
            &shots,
            folder,
            view_mode,
            editing,
            selection,
            selected.len(),
            confirm_delete,
        );

        let content: Element = if shots.is_empty() {
            empty_state(query_is_loading(&query)).into_element()
        } else {
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

                items.push(match *view_mode.read() {
                    ViewLayout::Grid => ScreenshotTile {
                        info,
                        selected: is_selected,
                        edit_mode: editing,
                        on_activate: on_activate.into(),
                        on_context,
                    }
                    .into_element(),

                    ViewLayout::List => ScreenshotRow {
                        info,
                        selected: is_selected,
                        edit_mode: editing,
                        on_activate: on_activate.into(),
                        on_context,
                    }
                    .into_element(),
                });
            }

            match *view_mode.read() {
                ViewLayout::Grid => {
                    let cols = grid_columns_for_width(*grid_width.read(), MAX_COL_W, GRID_GAP);
                    flow_grid(items, cols, grid_width, GRID_GAP)
                }
                ViewLayout::List => rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(6.)
                    .children(items)
                    .into_element(),
            }
        };

        let body = ScrollArea::new()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .scrollbar_gutter(true)
            .children(vec![content]);

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
                    (IconType::XClose, "Unselect all")
                } else {
                    (IconType::Check, "Select all")
                };
                let own = if selected.contains(&path) {
                    (IconType::XClose, "Unselect")
                } else {
                    (IconType::Check, "Select")
                };
                let mut multi = ContextMenu::new(x, y)
                    .title(format!("{} selected", selected.len()))
                    .action(own.0, own.1, move |()| selection.toggle(path.clone()))
                    .separator()
                    .action(icon, text, move |()| selection.toggle_all(&all));
                if !selected.is_empty() && !every {
                    multi = multi.action(IconType::XClose, "Clear selection", move |()| {
                        selection.clear()
                    });
                }
                if !selected.is_empty() {
                    multi = multi.separator().danger_action(
                        IconType::Trash01,
                        format!("Delete {}", selected.len()),
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
                    .spacing(16.)
                    .child(toolbar)
                    .child(body),
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

fn toolbar_row(
    shots: &[ScreenshotInfo],
    folder: Option<PathBuf>,
    view_mode: State<ViewLayout>,
    editing: bool,
    selection: Selection<PathBuf>,
    count: usize,
    mut confirm_delete: State<bool>,
) -> impl IntoElement {
    let all_paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();

    let mut right = rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(10.)
        .maybe_child(
            (!editing)
                .then_some(folder)
                .flatten()
                .map(open_folder_button),
        );

    if editing {
        let every = count > 0 && count == all_paths.len();
        right = right
            .child(
                label()
                    .text(format!("{count} selected"))
                    .font_size(12.)
                    .color(colors::fg_secondary()),
            )
            .child(
                Button::new()
                    .secondary()
                    .small()
                    .on_press(move |_| selection.toggle_all(&all_paths))
                    .text(if every { "Unselect all" } else { "Select all" }),
            )
            .maybe_child((count > 0 && !every).then(|| {
                Button::new()
                    .ghost()
                    .small()
                    .enabled(count > 0)
                    .on_press(move |_| selection.clear())
                    .text("Deselect all")
                    .into_element()
            }))
            .child(
                Button::new()
                    .danger()
                    .small()
                    .enabled(count > 0)
                    .on_press(move |_| confirm_delete.set(true))
                    .child(Icon::new(IconType::Trash01).size(14.))
                    .text(format!("Delete ({count})")),
            )
            .child(
                Button::new()
                    .ghost()
                    .small()
                    .on_press(move |_| selection.exit())
                    .text("Cancel"),
            );
    } else {
        right = right
            .child(
                SegmentedControl::new(view_mode)
                    .equal_width(40.)
                    .segment(Segment::new(ViewLayout::Grid).icon(IconType::DotsGrid))
                    .segment(Segment::new(ViewLayout::List).icon(IconType::ParagraphWrap)),
            )
            .child(
                Button::new()
                    .secondary()
                    .small()
                    .enabled(!shots.is_empty())
                    .on_press(move |_| selection.enter())
                    .child(Icon::new(IconType::Pencil01).size(14.))
                    .text("Select"),
            );
    }

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .child(
            rect().width(Size::flex(1.0)).child(
                label()
                    .text("Screenshots")
                    .font_size(20.)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(colors::fg_primary()),
            ),
        )
        .child(right)
        .into_element()
}

fn empty_state(loading: bool) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .center()
        .child(
            label()
                .text(if loading {
                    "Loading screenshots..."
                } else {
                    "No screenshots yet. Press F2 in-game to capture one."
                })
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .into_element()
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
                        .corner_radius(CornerRadius::new_all(14.))
                        .background(Color::from_rgb(26, 34, 41))
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            label()
                                .text(format!(
                                    "Move {count} screenshot{} to trash?",
                                    if count == 1 { "" } else { "s" }
                                ))
                                .font_size(16.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text("They can be restored from your system trash.")
                                .font_size(12.)
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
                                        .text("Cancel"),
                                )
                                .child(
                                    Button::new()
                                        .danger()
                                        .on_press(move |_| on_confirm())
                                        .child(Icon::new(IconType::Trash01).size(14.))
                                        .text("Delete"),
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
            .corner_radius(CornerRadius::new_all(5.))
            .background(Color::from_argb(170, 0, 0, 0))
            .layer(Layer::Relative(3))
            .child(label().text(text).font_size(9.).color(Color::WHITE))
            .into_element()
    })
}

fn preview_box(path: PathBuf, height: f32, res: Option<(u32, u32)>) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(height))
        .overflow(Overflow::Clip)
        .corner_radius(CornerRadius::new_all(10.))
        .background(colors::component_bg())
        .child(LocalImage::new(path, THUMB_EDGE, true).skeleton(true))
        .maybe_child(res_badge(res))
        .margin(1.)
        .into_element()
}

#[derive(PartialEq)]
struct ScreenshotTile {
    info: ScreenshotInfo,
    selected: bool,
    edit_mode: bool,
    on_activate: EventHandler<()>,
    on_context: EventHandler<(f32, f32)>,
}

impl Component for ScreenshotTile {
    fn render(&self) -> impl IntoElement {
        let info = self.info.clone();
        let mut hovered = use_state(|| false);

        let selected = self.selected;
        let on_activate = self.on_activate.clone();
        let on_context = self.on_context.clone();

        let border_color = if selected {
            colors::brand()
        } else if *hovered.read() {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .max_width(Size::px(MAX_COL_W))
            .corner_radius(CornerRadius::new_all(10.))
            .background(CARD_BG)
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
            .child(preview_box(
                info.path.clone(),
                TILE_PREVIEW_H,
                info.resolution,
            ))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .padding(Gaps::new_all(12.))
                    .spacing(6.)
                    .child(
                        label()
                            .text(info.name.clone())
                            .font_size(16.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(CARD_NAME),
                    )
                    .child(
                        label()
                            .text(format!(
                                "{} • {}",
                                format_size(info.size_bytes),
                                fmt_date(info.created)
                            ))
                            .font_size(11.)
                            .color(colors::fg_secondary()),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct ScreenshotRow {
    info: ScreenshotInfo,
    selected: bool,
    edit_mode: bool,
    on_activate: EventHandler<()>,
    on_context: EventHandler<(f32, f32)>,
}

impl Component for ScreenshotRow {
    fn render(&self) -> impl IntoElement {
        let info = self.info.clone();
        let mut hovered = use_state(|| false);

        let selected = self.selected;
        let on_activate = self.on_activate.clone();
        let on_context = self.on_context.clone();

        let thumb = rect()
            .width(Size::px(64.))
            .height(Size::px(40.))
            .corner_radius(CornerRadius::new_all(6.))
            .overflow(Overflow::Clip)
            .background(colors::component_bg())
            .child(LocalImage::new(info.path.clone(), THUMB_EDGE, true).skeleton(true));

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_symmetric(8., 10.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(if *hovered.read() {
                colors::component_bg_hover()
            } else {
                CARD_BG
            })
            .border(border_all_color(
                if selected { 2. } else { 1. },
                if selected {
                    colors::brand()
                } else {
                    colors::component_border()
                },
            ))
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| on_activate.call(()))
            .on_secondary_down(move |e: Event<PressEventData>| {
                if let PressEventData::Mouse(m) = e.data() {
                    on_context.call((m.global_location.x as f32, m.global_location.y as f32));
                }
            })
            .child(thumb)
            .child(
                rect().width(Size::flex(1.0)).child(
                    label()
                        .text(info.name.clone())
                        .font_size(13.)
                        .max_lines(1)
                        .width(Size::fill())
                        .color(CARD_NAME),
                ),
            )
            .maybe_child(info.resolution.map(|res| {
                label()
                    .text(format_res(res))
                    .font_size(11.)
                    .color(colors::fg_secondary())
                    .into_element()
            }))
            .child(
                label()
                    .text(format_size(info.size_bytes))
                    .font_size(11.)
                    .color(colors::fg_secondary()),
            )
            .child(
                label()
                    .text(fmt_date(info.created))
                    .font_size(11.)
                    .color(colors::fg_secondary()),
            )
    }
}
