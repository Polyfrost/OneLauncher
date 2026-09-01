use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::domain::GameLoader;
use oneclient_common::parse_mc_version;
use oneclient_core::clusters::Cluster;

use crate::components::{
    ART_PREVIEW_EDGE, Button, ClusterCreateDialog, ClusterLandscapeArt, DynamicArt, Icon, IconType,
    ScrollArea, origin_badge,
};
use crate::hooks::{
    settled_or_loading, use_active_cluster_id, use_bundled_targets, use_clusters, use_dispatch,
    use_game_snapshot, use_launcher, use_version_metadata,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::{format_duration_hm, sort_clusters_for_home};
use crate::view::app::launch_button_state;

const GRID_GAP_PX: f32 = 12.;
const MIN_CARD_WIDTH_PX: f32 = 260.;
const SIDEBAR_WIDTH_PX: f32 = 300.;
const CARD_HEIGHT_PX: f32 = 240.;
const ACTION_HEIGHT_PX: f32 = 32.;
const PLACEHOLDER_VERSION_INFO: &str = "Placeholder version info";

#[derive(Clone, Copy, PartialEq)]
enum DialogRequest {
    Create,
    Duplicate(i64),
}

pub fn version_targets(
    bundled: &[(String, GameLoader)],
    clusters: &[Cluster],
) -> Vec<(String, GameLoader)> {
    let mut targets: Vec<(String, GameLoader)> = Vec::new();

    let owned = clusters
        .iter()
        .map(|cluster| (cluster.mc_version.clone(), cluster.mc_loader));

    for target in bundled.iter().cloned().chain(owned) {
        if !targets.contains(&target) {
            targets.push(target);
        }
    }

    targets.sort_by(|a, b| {
        let key = |version: &str| {
            parse_mc_version(version)
                .map(|v| (v.major, v.minor.unwrap_or(0), v.patch.unwrap_or(0)))
                .unwrap_or((0, 0, 0))
        };
        key(&b.0)
            .cmp(&key(&a.0))
            .then_with(|| a.1.to_string().cmp(&b.1.to_string()))
    });

    targets
}

#[derive(PartialEq)]
pub struct Clusters;

impl Component for Clusters {
    fn render(&self) -> impl IntoElement {
        let clusters_query = use_clusters();
        let bundled_query = use_bundled_targets();
        let active_id = use_active_cluster_id();
        let mut grid_columns = use_state(|| 2_usize);
        let selected_id = use_state(|| None::<i64>);
        let dialog = use_state(|| None::<DialogRequest>);

        let clusters =
            sort_clusters_for_home(settled_or_loading(&clusters_query).unwrap_or_default());
        let bundled: Vec<(String, GameLoader)> =
            settled_or_loading(&bundled_query).unwrap_or_default();
        let targets = version_targets(&bundled, &clusters);

        if clusters.is_empty() && targets.is_empty() {
            return rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .overflow(Overflow::Clip)
                .padding(40.)
                .spacing(24.)
                .child(page_header())
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .main_align(Alignment::Center)
                        .child(
                            label()
                                .text("No versions available yet. Bundles are still syncing.")
                                .font_size(16.)
                                .color(colors::fg_secondary()),
                        ),
                );
        }

        let find = |id: i64| clusters.iter().find(|c| c.id == id).cloned();
        let selected = (*selected_id.read())
            .and_then(find)
            .or_else(|| active_id.read().and_then(find))
            .or_else(|| clusters.first().cloned());

        let columns = *grid_columns.read();

        let mut tiles: Vec<Element> = clusters
            .iter()
            .map(|cluster| {
                InstanceCard {
                    cluster: cluster.clone(),
                    selected: selected.as_ref().map(|c| c.id) == Some(cluster.id),
                    selected_id,
                }
                .into_element()
            })
            .collect();

        tiles.push(NewInstanceCard { dialog }.into_element());

        let rows: Vec<Vec<Element>> = tiles.chunks(columns).map(<[Element]>::to_vec).collect();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(Gaps::new(0., 40., 40., 40.))
            .spacing(16.)
            .child(page_header())
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .content(Content::Flex)
                    .spacing(GRID_GAP_PX)
                    .on_sized(move |event: Event<SizedEventData>| {
                        let width = event.data().area.width();
                        let next = grid_columns_for_width(width);
                        if next != *grid_columns.peek() {
                            *grid_columns.write() = next;
                        }
                    })
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .height(Size::fill())
                            .overflow(Overflow::Clip)
                            .child(
                                ScrollArea::new()
                                    .width(Size::fill())
                                    .height(Size::fill())
                                    .spacing(GRID_GAP_PX)
                                    .children(rows.into_iter().map(|row| {
                                        let row_len = row.len();
                                        rect()
                                            .horizontal()
                                            .width(Size::fill())
                                            .content(Content::Flex)
                                            .spacing(GRID_GAP_PX)
                                            .children(row)
                                            .children((row_len..columns).map(|_| {
                                                rect()
                                                    .width(Size::flex(1.0))
                                                    .height(Size::px(CARD_HEIGHT_PX))
                                                    .into_element()
                                            }))
                                            .into_element()
                                    })),
                            ),
                    )
                    .child(match selected.clone() {
                        Some(cluster) => DetailSidebar { cluster, dialog }.into_element(),
                        None => sidebar_error(),
                    }),
            )
            .maybe_child((*dialog.read()).map(|request| {
                let mut dialog = dialog;
                let close = move |()| {
                    *dialog.write() = None;
                };

                match request {
                    DialogRequest::Create => {
                        ClusterCreateDialog::new(targets.clone(), close).into_element()
                    }
                    DialogRequest::Duplicate(source_id) => {
                        let source = clusters.iter().find(|c| c.id == source_id);
                        match source {
                            Some(source) => ClusterCreateDialog::new(targets.clone(), close)
                                .duplicating(
                                    source_id,
                                    source.mc_version.clone(),
                                    source.mc_loader,
                                )
                                .into_element(),
                            None => rect().into_element(),
                        }
                    }
                }
            }))
    }
}

#[derive(PartialEq)]
struct InstanceCard {
    cluster: Cluster,
    selected: bool,
    selected_id: State<Option<i64>>,
}

impl Component for InstanceCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let cluster_id = self.cluster.id;
        let selected = self.selected;
        let hovered = *hovering.read();
        let focused = focus().is_focused();

        let mut selected_id = self.selected_id;

        let opacity = if selected {
            1.0
        } else if hovered || focused {
            0.85
        } else {
            0.6
        };

        let border_color = if selected || focused {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        rect()
            .key(cluster_id)
            .width(Size::flex(1.0))
            .height(Size::px(CARD_HEIGHT_PX))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_all_press(move |_| {
                *selected_id.write() = Some(cluster_id);
            })
            .on_pointer_enter(move |_| {
                *hovering.write() = true;
            })
            .on_pointer_leave(move |_| {
                *hovering.write() = false;
            })
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .overflow(Overflow::Clip)
                    .corner_radius(CornerRadius::new_all(12.))
                    .opacity(opacity)
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::fill())
                            .position(Position::new_absolute())
                            .child(
                                DynamicArt::for_cluster(&self.cluster).max_edge(ART_PREVIEW_EDGE),
                            ),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::fill())
                            .padding(Gaps::new_symmetric(12., 16.))
                            .main_align(Alignment::SpaceBetween)
                            .cross_align(Alignment::Start)
                            .corner_radius(CornerRadius::new_all(12.))
                            .border(
                                border_all_color(1., border_color)
                                    .alignment(BorderAlignment::Inner),
                            )
                            .layer(Layer::Relative(3))
                            .background(
                                LinearGradient::new()
                                    .angle(0.)
                                    .stop((Color::from_af32rgb(0.8, 0, 0, 0), 0.))
                                    .stop((Color::from_af32rgb(0.3, 0, 0, 0), 20.))
                                    .stop((Color::from_af32rgb(0.3, 0, 0, 0), 60.))
                                    .stop((Color::from_af32rgb(0.8, 0, 0, 0), 100.)),
                            )
                            .child(origin_badge(&self.cluster))
                            .child(
                                label()
                                    .text(self.cluster.name.clone())
                                    .font_size(32.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .max_lines(1)
                                    .width(Size::fill())
                                    .color(colors::fg_primary()),
                            ),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct NewInstanceCard {
    dialog: State<Option<DialogRequest>>,
}

impl Component for NewInstanceCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let mut dialog = self.dialog;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .height(Size::px(CARD_HEIGHT_PX))
            .center()
            .spacing(8.)
            .corner_radius(CornerRadius::new_all(12.))
            .background(if *hovering.read() {
                colors::component_bg_hover()
            } else {
                colors::component_bg()
            })
            .border(border_all_color(
                if focused { 2. } else { 1. },
                if focused {
                    colors::brand()
                } else {
                    colors::component_border()
                },
            ))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| {
                *hovering.write() = true;
            })
            .on_pointer_leave(move |_| {
                *hovering.write() = false;
            })
            .on_all_press(move |_| {
                *dialog.write() = Some(DialogRequest::Create);
            })
            .child(Icon::new(IconType::Plus).size(28.))
            .child(
                label()
                    .text("New instance")
                    .font_size(13.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_secondary()),
            )
    }
}

#[derive(PartialEq)]
struct DetailSidebar {
    cluster: Cluster,
    dialog: State<Option<DialogRequest>>,
}

impl Component for DetailSidebar {
    fn render(&self) -> impl IntoElement {
        let mut active_id = use_active_cluster_id();
        let dispatch = use_dispatch();
        let game = use_game_snapshot();
        let launcher = use_launcher();

        let cluster = &self.cluster;
        let cluster_id = cluster.id;
        let syncing = launcher.fetching || launcher.syncing_bundles;
        let (launch_label, launch_enabled) = launch_button_state(&game, cluster_id, syncing);

        let parsed = parse_mc_version(&cluster.mc_version);
        let major = parsed.as_ref().map(|p| p.major);
        let key = parsed.and_then(|p| p.key());
        let metadata = use_version_metadata(major, key, Some(cluster.mc_loader));

        let description = metadata
            .as_ref()
            .and_then(|m| m.long_description.clone())
            .unwrap_or_else(|| PLACEHOLDER_VERSION_INFO.to_string());
        let tags = metadata.as_ref().map(|m| m.tags.clone()).unwrap_or_default();

        let mut dialog = self.dialog;

        rect()
            .width(Size::px(SIDEBAR_WIDTH_PX))
            .min_width(Size::px(SIDEBAR_WIDTH_PX))
            .height(Size::fill())
            .vertical()
            .spacing(8.)
            .padding(8.)
            .corner_radius(CornerRadius::new_all(12.))
            .background(colors::page_elevated())
            .border(border_all_color(1., colors::component_border()))
            .overflow(Overflow::Clip)
            .child(
                rect()
                    .width(Size::fill())
                    .max_height(Size::px(140.))
                    .child(ClusterLandscapeArt::for_version(
                        major.unwrap_or(0),
                        key,
                        Some(cluster.mc_loader),
                        false,
                    )),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .content(Content::Flex)
                    .padding(Gaps::new_all(8.))
                    .spacing(8.)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .spacing(4.)
                            .child(
                                label()
                                    .text(cluster.name.clone())
                                    .font_size(24.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .max_lines(2)
                                    .width(Size::fill())
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(format!("{} {}", cluster.mc_loader, cluster.mc_version))
                                    .font_size(12.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::fg_secondary()),
                            )
                            .maybe_child(tags_row(&tags))
                            .child(
                                label()
                                    .text(description)
                                    .font_size(12.)
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                label()
                                    .text(instance_meta(cluster))
                                    .font_size(11.)
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(8.)
                            .child(
                                Button::new()
                                    .primary()
                                    .width(Size::fill())
                                    .enabled(launch_enabled)
                                    .on_press(move |_| {
                                        if launch_enabled {
                                            dispatch.launch_cluster(cluster_id);
                                        }
                                    })
                                    .text(launch_label),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .content(Content::Flex)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .width(Size::flex(9.))
                                            .height(Size::px(ACTION_HEIGHT_PX))
                                            .on_press(move |_| {
                                                *active_id.write() = Some(cluster_id);
                                                let _ = RouterContext::get()
                                                    .push(Route::ClusterOverview { cluster_id });
                                            })
                                            .text("View")
                                            .child(Icon::new(IconType::ArrowRight).size(14.)),
                                    )
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .icon()
                                            .on_press(move |_| {
                                                *dialog.write() =
                                                    Some(DialogRequest::Duplicate(cluster_id));
                                            })
                                            .alt("Duplicate this instance")
                                            .child(Icon::new(IconType::Copy01).size(14.)),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}

fn instance_meta(cluster: &Cluster) -> String {
    let played = format_duration_hm(cluster.overall_played.as_secs() as i64);

    let folder = if cluster.uses_dedicated_dir() {
        "separate folder"
    } else {
        "shared folder"
    };

    match cluster.last_played {
        Some(_) => format!("{played} played · {folder}"),
        None => format!("Never played · {folder}"),
    }
}

fn tags_row(tags: &[String]) -> Option<Element> {
    if tags.is_empty() {
        return None;
    }

    Some(
        rect()
            .horizontal()
            .width(Size::fill())
            .spacing(6.)
            .children(tags.iter().map(|tag| {
                rect()
                    .padding(Gaps::new_symmetric(3., 8.))
                    .corner_radius(CornerRadius::new_all(999.))
                    .background(colors::component_bg())
                    .child(
                        label()
                            .text(tag.clone())
                            .font_size(11.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_secondary()),
                    )
                    .into_element()
            }))
            .into_element(),
    )
}

fn page_header() -> impl IntoElement {
    rect()
        .vertical()
        .spacing(6.)
        .child(
            label()
                .text("Versions")
                .font_size(36.)
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(
                    "Every version you can play, plus any instances you set up yourself. \
                     Make more than one for the same version to keep separate sets of mods.",
                )
                .font_size(12.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_secondary()),
        )
}

fn sidebar_error() -> Element {
    rect()
        .width(Size::px(SIDEBAR_WIDTH_PX))
        .height(Size::fill())
        .vertical()
        .padding(16.)
        .spacing(8.)
        .corner_radius(CornerRadius::new_all(16.))
        .border(border_all_color(1., colors::component_border()))
        .background(colors::component_bg())
        .child(
            label()
                .text("Select an instance to see its details.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn grid_columns_for_width(available_width_px: f32) -> usize {
    const GAP: f32 = 16.;

    let grid_width = available_width_px - SIDEBAR_WIDTH_PX - GAP;
    if grid_width < MIN_CARD_WIDTH_PX {
        return 1;
    }

    let cols = (grid_width / (MIN_CARD_WIDTH_PX + GRID_GAP_PX)).floor() as usize;
    cols.clamp(1, 3)
}
