use std::collections::HashSet;
use std::path::PathBuf;

use freya::prelude::*;
use freya::query::QueryStateData;
use oneclient_core::ScreenshotInfo;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Button, ContextMenu, Icon, IconType, LocalImage, OverlayPopup, ScreenshotViewer, ScrollArea,
    open_folder_button,
    Segment, SegmentedControl,
};
use crate::hooks::{
    ScreenshotAction, try_cluster_screenshots, use_cluster_screenshots, use_dispatch,
    use_screenshot_action, use_view_state,
};
use crate::layout::cluster_content;
use crate::theme::colors;
use crate::ui::{border_all_color, fmt_date};
use crate::utils::{format_res, format_size};

use super::{cluster_not_found, load_cluster};

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
        let Some(cluster) = load_cluster(self.cluster_id) else {
            return cluster_not_found();
        };
        let folder = cluster.game_dir().ok().map(|d| d.join("screenshots"));

        let query = use_cluster_screenshots(self.cluster_id);
        let action = use_screenshot_action();

        let shots = try_cluster_screenshots(&query).unwrap_or_default();

        let view_mode = use_view_state("cluster.screenshots").layout;
        let menu_dispatch = use_dispatch();

        let mut edit_mode = use_state(|| false);
        let mut selected = use_state(HashSet::<PathBuf>::new);
        let mut viewing = use_state(|| None::<usize>);
        let confirm_delete = use_state(|| false);
        let grid_width = use_state(|| 0f32);
        let mut menu = use_state(|| None::<(f32, f32, PathBuf)>);

        let toolbar = toolbar_row(&shots, folder, view_mode, edit_mode, selected, confirm_delete);

        let content: Element = if shots.is_empty() {
            empty_state(matches!(
                &*query.read().state(),
                QueryStateData::Loading { res: None }
            ))
            .into_element()
        } else {
            let mut items: Vec<Element> = Vec::new();
            for (idx, info) in shots.iter().enumerate() {
                let info = info.clone();
                let is_selected = selected.read().contains(&info.path);
                let editing = *edit_mode.read();

                let activate_path = info.path.clone();
                let on_activate = move |_| {
                    if editing {
                        let p = activate_path.clone();
                        let mut set = selected.write();
                        if !set.remove(&p) {
                            set.insert(p);
                        }
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
                    let cols = grid_columns_for_width(*grid_width.read());
                    grid(items, cols, grid_width).into_element()
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
            .children(vec![content]);

        let confirm_overlay = confirm_delete.read().then(|| {
            let count = selected.read().len();
            confirm_panel(count, confirm_delete, move || {
                let paths: Vec<PathBuf> = selected.read().iter().cloned().collect();
                for path in paths {
                    action.mutate(ScreenshotAction::Delete { path });
                }
                selected.write().clear();
                edit_mode.set(false);
                confirm_delete.clone().set(false);
            })
        });

        let menu_overlay = menu.read().clone().map(|(x, y, path)| {
            let copy_dispatch = menu_dispatch.clone();
            let open_path = path.clone();
            let copy_path = path.clone();
            let delete_path = path.clone();
            ContextMenu::new(x, y)
                .on_close(move |_| menu.set(None))
                .action(IconType::Folder, "Open in folder", move |()| {
                    if let Some(dir) = open_path.parent() {
                        crate::platform::open_url(&dir.to_string_lossy());
                    }
                })
                .action(IconType::Copy01, "Copy", move |()| {
                    crate::platform::copy_image_to_clipboard(copy_path.clone());
                    copy_dispatch
                        .notify("Copied to clipboard")
                        .body("Screenshot copied to your clipboard.")
                        .info()
                        .icon(IconType::ClipboardCheck)
                        .send();
                })
                .separator()
                .danger_action(IconType::Trash01, "Delete", move |()| {
                    action.mutate(ScreenshotAction::Delete {
                        path: delete_path.clone(),
                    });
                })
                .into_element()
        });

        cluster_content()
            .child(
                rect()
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
    mut edit_mode: State<bool>,
    mut selected: State<HashSet<PathBuf>>,
    mut confirm_delete: State<bool>,
) -> impl IntoElement {
    let editing = *edit_mode.read();
    let count = selected.read().len();
    let all_paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();

    let mut right = rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(10.)
        .maybe_child((!editing).then_some(folder).flatten().map(open_folder_button));

    if editing {
        let select_all_paths = all_paths.clone();
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
                    .on_press(move |_| {
                        let mut set = selected.write();
                        *set = select_all_paths.iter().cloned().collect();
                    })
                    .text("Select all"),
            )
            .child(
                Button::new()
                    .ghost()
                    .small()
                    .enabled(count > 0)
                    .on_press(move |_| selected.write().clear())
                    .text("Deselect all"),
            )
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
                    .on_press(move |_| {
                        selected.write().clear();
                        edit_mode.set(false);
                    })
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
                    .on_press(move |_| edit_mode.set(true))
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

fn grid_columns_for_width(width: f32) -> usize {
    if width <= 0. {
        return 1;
    }
    (((width + GRID_GAP) / (MAX_COL_W + GRID_GAP)).ceil() as usize).max(1)
}

fn grid(items: Vec<Element>, cols: usize, mut width: State<f32>) -> impl IntoElement {
    let cols = cols.max(1);

    let mut root = rect().vertical().width(Size::fill()).spacing(GRID_GAP);
    let mut iter = items.into_iter();
    let mut remaining = true;
    while remaining {
        let mut row = rect()
            .horizontal()
            .width(Size::fill())
            .spacing(GRID_GAP)
            .content(Content::Flex);
        let mut filled = 0;
        for _ in 0..cols {
            if let Some(card) = iter.next() {
                row = row.child(card);
                filled += 1;
            } else {
                row = row.child(rect().width(Size::flex(1.0)));
            }
        }
        if filled == 0 {
            break;
        }
        remaining = filled == cols;
        root = root.child(row.into_element());
    }

    root.on_sized(move |event: Event<SizedEventData>| {
        let w = event.data().area.width();
        if (w - *width.peek()).abs() > 0.5 {
            *width.write() = w;
        }
    })
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
            .on_pointer_enter(move |_| {
                hovered.set(true);
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                hovered.set(false);
                Cursor::set(CursorIcon::default());
            })
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
            .on_pointer_enter(move |_| {
                hovered.set(true);
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                hovered.set(false);
                Cursor::set(CursorIcon::default());
            })
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
