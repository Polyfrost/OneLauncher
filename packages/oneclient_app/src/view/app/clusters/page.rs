use freya::prelude::*;
use freya::query::QueryStateData;
use freya::router::RouterContext;
use oneclient_core::VersionKey;
use oneclient_core::clusters::Cluster;
use oneclient_core::packages::domain::GameLoader;

use crate::components::{Button, ClusterLandscapeArt, Dropdown, Icon, IconType, MajorVersionCard};
use crate::hooks::{
    use_active_cluster_id, use_clusters, use_dispatch, use_game_snapshot, use_version_metadata,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::{
    default_loader, default_major, default_version_key, group_clusters_by_major, loaders_for_major,
    major_pretty_name, resolve_cluster, version_keys, version_label,
};
use crate::view::app::launch_button_state;

const GRID_GAP_PX: f32 = 12.;
const MIN_CARD_WIDTH_PX: f32 = 280.;
const SIDEBAR_WIDTH_PX: f32 = 300.;
const CARD_HEIGHT_PX: f32 = 146.;
const PLACEHOLDER_VERSION_INFO: &str = "Placeholder version info";

#[derive(PartialEq)]
pub struct Clusters;

impl Component for Clusters {
    fn render(&self) -> impl IntoElement {
        let clusters_query = use_clusters();
        let active_id = use_active_cluster_id();
        let mut selected_major = use_state(|| None::<u32>);
        let mut selected_version = use_state(|| None::<VersionKey>);
        let mut selected_loader = use_state(|| None::<GameLoader>);
        let mut grid_columns = use_state(|| 2_usize);

        let clusters = {
            let reader = clusters_query.read();
            match &*reader.state() {
                QueryStateData::Settled { res: Ok(list), .. } => list.clone(),
                QueryStateData::Loading {
                    res: Some(Ok(list)),
                } => list.clone(),
                _ => Vec::new(),
            }
        };

        let groups = group_clusters_by_major(&clusters);
        let majors: Vec<u32> = groups.keys().copied().collect();

        if majors.is_empty() {
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

        let active_cluster = active_id
            .read()
            .and_then(|id| clusters.iter().find(|c| c.id == id).cloned());

        if selected_major.read().is_none() {
            *selected_major.write() = default_major(&groups, active_cluster.clone());
        }

        let major = (*selected_major.read())
            .filter(|m| groups.contains_key(m))
            .unwrap_or(majors[0]);

        if *selected_major.read() != Some(major) {
            *selected_major.write() = Some(major);
        }

        let clusters_for_major = groups.get(&major).cloned().unwrap_or_default();

        if selected_version.read().is_none() {
            let preferred = active_cluster
                .as_ref()
                .and_then(|c| oneclient_core::parse_mc_version(&c.mc_version))
                .and_then(|p| p.key());
            *selected_version.write() = default_version_key(&clusters_for_major, preferred);
        }

        if selected_loader.read().is_none() {
            let preferred = active_cluster.as_ref().map(|c| c.mc_loader);
            *selected_loader.write() = default_loader(&clusters_for_major, preferred);
        }

        let cluster = resolve_cluster(
            &clusters_for_major,
            *selected_version.read(),
            *selected_loader.read(),
        )
        .or_else(|| clusters_for_major.first().cloned());

        let columns = *grid_columns.read();
        let grid_rows = chunk_majors(&majors, columns);

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
                            .spacing(GRID_GAP_PX)
                            .children(grid_rows.into_iter().map(|row| {
                                let row_len = row.len();
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .content(Content::Flex)
                                    .spacing(GRID_GAP_PX)
                                    .children(row.into_iter().map(|major| {
                                        let list = groups.get(&major).cloned().unwrap_or_default();
                                        let selected = *selected_major.read() == Some(major);
                                        let mut selected_major = selected_major;
                                        let mut selected_version = selected_version;
                                        let mut selected_loader = selected_loader;

                                        MajorVersionCard::new(major, &list, selected, move |_| {
                                            *selected_major.write() = Some(major);
                                            *selected_version.write() = None;
                                            *selected_loader.write() = None;
                                        })
                                        .into_element()
                                    }))
                                    .children((row_len..columns).map(|_| {
                                        rect()
                                            .width(Size::flex(1.0))
                                            .height(Size::px(CARD_HEIGHT_PX))
                                            .into_element()
                                    }))
                                    .into_element()
                            })),
                    )
                    .child(match cluster {
                        Some(cluster) => detail_sidebar(
                            major,
                            cluster,
                            &clusters_for_major,
                            selected_version,
                            selected_loader,
                        ),
                        None => sidebar_error(),
                    }),
            )
    }
}

fn detail_sidebar(
    major: u32,
    cluster: Cluster,
    clusters_for_major: &[Cluster],
    selected_version: State<Option<VersionKey>>,
    selected_loader: State<Option<GameLoader>>,
) -> Element {
    let active_id = use_active_cluster_id();
    let dispatch = use_dispatch();
    let game = use_game_snapshot();
    let keys = version_keys(clusters_for_major);
    let loaders = loaders_for_major(clusters_for_major);
    let cluster_id = cluster.id;
    let version_title = major_pretty_name(major);
    let version_value = *selected_version.read();
    let loader_value = *selected_loader.read();

    let metadata = use_version_metadata(Some(major), version_value, loader_value);

    let heading = metadata
        .as_ref()
        .map(|m| m.name.clone())
        .unwrap_or_else(|| format!("Version {version_title}"));
    let description = metadata
        .as_ref()
        .and_then(|m| m.long_description.clone())
        .unwrap_or_else(|| PLACEHOLDER_VERSION_INFO.to_string());
    let tags = metadata
        .as_ref()
        .map(|m| m.tags.clone())
        .unwrap_or_default();

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
        .child(rect().width(Size::fill()).max_height(Size::px(140.)).child(
            ClusterLandscapeArt::for_version(major, version_value, loader_value, false),
        ))
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
                                .text(heading)
                                .font_size(24.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .maybe_child(tags_row(&tags))
                        .child(
                            label()
                                .text(description)
                                .font_size(12.)
                                .color(colors::fg_secondary()),
                        )
                        .children(version_rows(major, &keys, version_value, selected_version))
                        .children(loader_rows(&loaders, loader_value, selected_loader)),
                )
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .main_align(Alignment::Center)
                        .spacing(8.)
                        .child(play_button(
                            cluster_id,
                            dispatch,
                            launch_button_state(&game, cluster_id),
                        ))
                        .child(view_button(cluster_id, active_id)),
                ),
        )
        .into_element()
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

fn version_rows(
    major: u32,
    keys: &[VersionKey],
    selected: Option<VersionKey>,
    mut selected_version: State<Option<VersionKey>>,
) -> Option<Element> {
    if keys.len() <= 1 {
        return None;
    }

    let options: Vec<String> = keys.iter().map(|k| version_label(major, *k)).collect();
    let current = selected
        .and_then(|s| keys.iter().position(|k| *k == s))
        .unwrap_or(0);
    let keys = keys.to_vec();

    Some(info_row(
        "Minor Version",
        Dropdown::new(options[current].clone(), options)
            .on_select(move |idx: usize| {
                if let Some(key) = keys.get(idx).copied() {
                    *selected_version.write() = Some(key);
                }
            })
            .into_element(),
    ))
}

fn loader_rows(
    loaders: &[GameLoader],
    selected: Option<GameLoader>,
    mut selected_loader: State<Option<GameLoader>>,
) -> Option<Element> {
    if loaders.len() <= 1 {
        return None;
    }

    let options: Vec<String> = loaders.iter().map(|l| l.to_string()).collect();
    let current = selected
        .and_then(|s| loaders.iter().position(|l| *l == s))
        .unwrap_or(0);
    let loaders = loaders.to_vec();

    Some(info_row(
        "Mod Loader",
        Dropdown::new(options[current].clone(), options)
            .on_select(move |idx: usize| {
                if let Some(loader) = loaders.get(idx).copied() {
                    *selected_loader.write() = Some(loader);
                }
            })
            .into_element(),
    ))
}

fn info_row(label_text: impl Into<String>, control: impl IntoElement) -> Element {
    let label_text = label_text.into();
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .main_align(Alignment::SpaceBetween)
        .padding(Gaps::new_symmetric(4.0, 0.0))
        .child(
            label()
                .text(label_text)
                .font_size(12.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .child(control)
        .into_element()
}

fn play_button(
    cluster_id: i64,
    dispatch: crate::BridgeDispatch,
    state: (&'static str, bool),
) -> impl IntoElement {
    let (label, enabled) = state;
    Button::new()
        .primary()
        .width(Size::flex(1.))
        .enabled(enabled)
        .on_press(move |_| {
            if enabled {
                dispatch.launch_cluster(cluster_id);
            }
        })
        .text(label)
}

fn view_button(cluster_id: i64, mut active_id: State<Option<i64>>) -> impl IntoElement {
    Button::new()
        .secondary()
        .width(Size::flex(1.))
        .on_press(move |_| {
            *active_id.write() = Some(cluster_id);
            let _ = RouterContext::get().push(Route::ClusterOverview { cluster_id });
        })
        .text("View")
        .child(Icon::new(IconType::ArrowRight).size(14.))
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
                    "Something something in corporate style fashion about picking your \
                     preferred gamemodes and versions and optionally loader so that oneclient \
                     can pick something for them",
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
                .text("Could not resolve a cluster for this version.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text("Try selecting a different minor version or mod loader.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn chunk_majors(majors: &[u32], columns: usize) -> Vec<Vec<u32>> {
    let columns = columns.max(1);
    majors.chunks(columns).map(|chunk| chunk.to_vec()).collect()
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
