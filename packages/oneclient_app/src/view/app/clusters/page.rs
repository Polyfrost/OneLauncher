use std::path::PathBuf;

use crate::components::{ClusterContextMenu, InstanceOverviewButton, InstanceOverviewSurface};
use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::domain::GameLoader;
use oneclient_common::{VersionKey, parse_mc_version};
use oneclient_core::clusters::Cluster;
use oneclient_core::settings::ViewLayout;

use crate::components::ClusterMenuTarget;
use crate::components::{
    ART_PREVIEW_EDGE, Button, Dropdown, DynamicArt, FilterMenu, FilterOption, Icon, IconType,
    InstanceRow, ScrollArea, Segment, SegmentedControl, TabBar, TabItem, TextInput, VersionCard,
    open_folder_button,
};
use crate::hooks::{
    settled_or_loading, use_active_cluster_id, use_clusters, use_dispatch, use_game_snapshot,
    use_launcher, use_version_metadata, use_view_state,
};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::{colors, rounded, text_size};
use crate::ui::{
    border_all_color, columns_for, fixed_grid, grid_columns_for_width, last_played_label,
    window_logical_size,
};
use crate::utils::{
    ClusterGroups, GridSelection, ReleaseLine, default_line, default_loader, default_version_key,
    format_duration, line_title, resolve_cluster, sort_clusters_for_home, split_clusters,
};
use crate::view::app::clusters::CreateInstanceModal;
use crate::view::app::{
    launch_button_label, launch_button_state, launch_syncing, use_shared_dir_taken,
};

/// Same side gutter as Browse and Wardrobe, so the page lines up under the shared "Back to" button
const PAGE_GUTTER_PX: f32 = 40.;
const PAGE_PADDING: Gaps = Gaps::new(12., PAGE_GUTTER_PX, 28., PAGE_GUTTER_PX);
const COLUMN_GAP_PX: f32 = 24.;
const CARD_GAP_PX: f32 = 16.;
const ROW_GAP_PX: f32 = 8.;
const ROW_HEIGHT_PX: f32 = 56.;
const MAX_CARD_WIDTH_PX: f32 = 340.;
const SCROLLBAR_GUTTER_PX: f32 = 14.;
const MIN_ROW_WIDTH_PX: f32 = 344.;
const MAX_ROW_COLUMNS: usize = 4;
const SIDEBAR_WIDTH_PX: f32 = 360.;
const COMPACT_SIDEBAR_WIDTH_PX: f32 = 300.;
const COMPACT_BELOW_PX: f32 = 1000.;
const INLINE_SEARCH_MIN_PX: f32 = 700.;
const SEARCH_WIDTH_PX: f32 = 220.;
const TOOLBAR_HEIGHT_PX: f32 = 44.;
const ART_MAX_HEIGHT_PX: f32 = 200.;
const ART_MIN_SIDEBAR_PX: f32 = 480.;
const SIDEBAR_PADDING_PX: f32 = 22.;
const SIDEBAR_ACTION_GAP_PX: f32 = 18.;
const SIDEBAR_ACTION_HEIGHT_PX: f32 = 36.;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Filter {
    All,
    OneClient,
    Custom,
    Modpacks,
}

impl Filter {
    pub(crate) const ALL: [Filter; 4] = [
        Filter::All,
        Filter::OneClient,
        Filter::Custom,
        Filter::Modpacks,
    ];

    pub(crate) fn shows(self, section: Filter) -> bool {
        self == Filter::All || self == section
    }

    pub(crate) fn of(cluster: &Cluster) -> Self {
        if !cluster.is_isolated() {
            Filter::OneClient
        } else if cluster.linked_modpack_hash.is_some() {
            Filter::Modpacks
        } else {
            Filter::Custom
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Filter::All => t("oneclient.clusters.page.filter.all"),
            Filter::OneClient => "OneClient",
            Filter::Custom => t("oneclient.clusters.page.filter.custom"),
            Filter::Modpacks => t("oneclient.clusters.page.filter.modpacks"),
        }
    }
}

pub(crate) fn cluster_matches(cluster: &Cluster, loaders: &[String], needle: &str) -> bool {
    (loaders.is_empty() || loaders.contains(&cluster.mc_loader.to_string()))
        && (needle.is_empty()
            || format!(
                "{} {} {}",
                cluster.name, cluster.mc_version, cluster.mc_loader
            )
            .to_lowercase()
            .contains(needle))
}

pub(crate) fn loader_options(clusters: &[Cluster]) -> Vec<String> {
    let mut loaders: Vec<String> = clusters.iter().map(|c| c.mc_loader.to_string()).collect();
    loaders.sort();
    loaders.dedup();
    loaders
}

pub(crate) fn recent_line_heads(groups: &ClusterGroups) -> Vec<Cluster> {
    sort_clusters_for_home(
        groups
            .values()
            .rev()
            .filter_map(|g| sort_clusters_for_home(g.clone()).into_iter().next())
            .collect(),
    )
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Sort {
    RecentFirst,
    OldestFirst,
    NameAsc,
    NameDesc,
}

impl Sort {
    const ALL: [Sort; 4] = [
        Sort::RecentFirst,
        Sort::OldestFirst,
        Sort::NameAsc,
        Sort::NameDesc,
    ];

    fn label(self) -> &'static str {
        match self {
            Sort::RecentFirst => t("oneclient.clusters.page.sort.recent"),
            Sort::OldestFirst => t("oneclient.clusters.page.sort.oldest"),
            Sort::NameAsc => t("oneclient.clusters.page.sort.name_asc"),
            Sort::NameDesc => t("oneclient.clusters.page.sort.name_desc"),
        }
    }
}

#[derive(PartialEq)]
pub struct Clusters;

impl Component for Clusters {
    fn render(&self) -> impl IntoElement {
        let clusters_query = use_clusters();
        let active_id = use_active_cluster_id();
        let show_create = use_state(|| false);
        let mut selected = use_state(|| None::<GridSelection>);
        let mut selected_cluster = use_state(|| None::<i64>);
        let mut filter = use_state(|| Filter::All);
        let query = use_state(String::new);
        let loaders = use_state(Vec::<String>::new);
        let sort = use_state(|| Sort::RecentFirst);
        let layout = use_view_state("clusters").layout;
        let mut body_width =
            use_state(|| window_logical_size().width - PAGE_PADDING.left() - PAGE_PADDING.right());

        let clusters = settled_or_loading(&clusters_query).unwrap_or_default();

        let (groups, instances) = split_clusters(&clusters);
        // Newest version first
        let lines: Vec<ReleaseLine> = groups.keys().rev().copied().collect();

        if lines.is_empty() && instances.is_empty() {
            return rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .overflow(Overflow::Clip)
                .padding(PAGE_PADDING)
                .spacing(24.)
                .child(page_header(
                    show_create,
                    *body_width.read() >= COMPACT_BELOW_PX,
                ))
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .main_align(Alignment::Center)
                        .child(
                            label()
                                .text(t("oneclient.clusters.page.syncing"))
                                .font_size(text_size(16.))
                                .color(colors::fg_secondary()),
                        ),
                )
                .maybe_child(create_modal(show_create));
        }

        let active_cluster = active_id
            .read()
            .and_then(|id| clusters.iter().find(|c| c.id == id).cloned());

        let fallback = || {
            lines
                .first()
                .copied()
                .map(GridSelection::Line)
                .or_else(|| instances.first().map(|c| GridSelection::Instance(c.id)))
        };

        if selected.read().is_none() {
            *selected.write() = match active_cluster.as_ref() {
                Some(cluster) if cluster.is_isolated() => Some(GridSelection::Instance(cluster.id)),
                other => default_line(&groups, other.cloned())
                    .map(GridSelection::Line)
                    .or_else(fallback),
            };
        }

        let current = (*selected.read())
            .filter(|sel| match sel {
                GridSelection::Line(line) => groups.contains_key(line),
                GridSelection::Instance(id) => instances.iter().any(|c| c.id == *id),
            })
            .or_else(fallback);

        if *selected.read() != current {
            *selected.write() = current;
        }

        let line = match current {
            Some(GridSelection::Line(line)) => Some(line),
            _ => None,
        };

        let clusters_for_line = line
            .and_then(|line| groups.get(&line).cloned())
            .unwrap_or_default();

        if line.is_some() && selected_cluster.read().is_none() {
            *selected_cluster.write() =
                default_line_cluster(&clusters_for_line, active_cluster.as_ref());
        }

        let cluster = selected_cluster
            .read()
            .and_then(|id| clusters_for_line.iter().find(|c| c.id == id).cloned())
            .or_else(|| clusters_for_line.first().cloned());

        let instance = match current {
            Some(GridSelection::Instance(id)) => instances.iter().find(|c| c.id == id).cloned(),
            _ => None,
        };

        let (packs, custom): (Vec<&Cluster>, Vec<&Cluster>) = instances
            .iter()
            .partition(|c| c.linked_modpack_hash.is_some());

        let body_w = *body_width.read();
        let sidebar_w = if body_w < COMPACT_BELOW_PX {
            COMPACT_SIDEBAR_WIDTH_PX
        } else {
            SIDEBAR_WIDTH_PX
        };
        let main_w = body_w - sidebar_w - COLUMN_GAP_PX;
        let grid = *layout.read() == ViewLayout::Grid;
        let (columns, item_height, gap) = if grid {
            let grid_w = main_w - SCROLLBAR_GUTTER_PX;
            let columns = grid_columns_for_width(grid_w, MAX_CARD_WIDTH_PX, CARD_GAP_PX);
            (
                columns,
                VersionCard::height_for_width(card_width_for(grid_w, columns, CARD_GAP_PX)),
                CARD_GAP_PX,
            )
        } else {
            (version_list_columns(main_w), ROW_HEIGHT_PX, ROW_GAP_PX)
        };
        let inline_search = main_w >= INLINE_SEARCH_MIN_PX;

        let active_filter = *filter.read();
        let needle = query.read().trim().to_lowercase();
        let picked_loaders = loaders.read().clone();
        let matches = |c: &Cluster| cluster_matches(c, &picked_loaders, &needle);
        let sort_by = *sort.read();

        let line_heads = recent_line_heads(&groups);
        let shown_lines: Vec<ReleaseLine> =
            shown(&line_heads.iter().collect::<Vec<_>>(), sort_by, |c| {
                active_filter.shows(Filter::OneClient)
                    && ReleaseLine::for_cluster(c).is_some_and(|l| groups[&l].iter().any(matches))
            })
            .into_iter()
            .filter_map(ReleaseLine::for_cluster)
            .collect();
        let shown_custom = shown(&custom, sort_by, |c| {
            active_filter.shows(Filter::Custom) && matches(c)
        });
        let shown_packs = shown(&packs, sort_by, |c| {
            active_filter.shows(Filter::Modpacks) && matches(c)
        });

        let available_loaders = loader_options(&clusters);
        let filters = rect()
            .horizontal()
            .spacing(8.)
            .child(filter_menu(&available_loaders, loaders, sort))
            .child(layout_toggle(layout))
            .into_element();
        let nothing_shown =
            shown_lines.is_empty() && shown_custom.is_empty() && shown_packs.is_empty();

        let tabs = Filter::ALL.map(|value| {
            TabItem::new(value.label(), active_filter == value).on_press(move |_| filter.set(value))
        });

        let active_cluster = clusters
            .iter()
            .find(|c| Some(c.id) == *active_id.read())
            .cloned();
        let line_cards: Vec<Element> = shown_lines
            .iter()
            .filter_map(|&line| {
                let list = &groups[&line];
                let item = GridSelection::Line(line);
                let is_selected = current == Some(item);
                let picked = cluster
                    .as_ref()
                    .filter(|c| is_selected && list.iter().any(|listed| listed.id == c.id));
                let target = picked
                    .or_else(|| {
                        let id = default_line_cluster(list, active_cluster.as_ref())?;
                        list.iter().find(|cluster| cluster.id == id)
                    })
                    .or_else(|| list.first())?;
                let line_menu = Some(ClusterMenuTarget::for_cluster(target, list.len() == 1));
                let on_press = move |_| {
                    selected.set(Some(item));
                    selected_cluster.set(None);
                };
                Some(if grid {
                    VersionCard::new(line, list, target, is_selected, on_press)
                        .height(item_height)
                        .menu(line_menu)
                        .into_element()
                } else {
                    InstanceRow::for_line(line, list, target, is_selected, on_press)
                        .menu(line_menu)
                        .into_element()
                })
            })
            .collect();

        let instance_rows = |list: &[&Cluster]| -> Vec<Element> {
            list.iter()
                .map(|c| {
                    let item = GridSelection::Instance(c.id);
                    let is_selected = current == Some(item);
                    let on_press = move |_| selected.set(Some(item));
                    let menu = Some(ClusterMenuTarget::for_cluster(c, true));
                    if grid {
                        VersionCard::for_instance(c, is_selected, on_press)
                            .height(item_height)
                            .menu(menu)
                            .into_element()
                    } else {
                        InstanceRow::new(c, is_selected, on_press)
                            .menu(menu)
                            .into_element()
                    }
                })
                .collect()
        };

        let sidebar = match (instance, line, cluster) {
            (Some(instance), _, _) => Sidebar::for_instance(&instance, sidebar_w).into_element(),
            (None, Some(line), Some(cluster)) => {
                let parsed = parse_mc_version(&cluster.mc_version);
                LineSidebar {
                    major: line.major,
                    key: parsed.and_then(|p| p.key()),
                    loader: cluster.mc_loader,
                    user_created: cluster.user_created,
                    base: Sidebar {
                        picker: line_picker(line, &cluster, &clusters_for_line, selected_cluster),
                        description: cluster.description.clone(),
                        ..Sidebar::base(&cluster, sidebar_w)
                    },
                }
                .into_element()
            }
            _ => sidebar_error(sidebar_w),
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(PAGE_PADDING)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::fill())
                    .content(Content::Flex)
                    .spacing(COLUMN_GAP_PX)
                    .on_sized(move |event: Event<SizedEventData>| {
                        let width = event.data().area.width();
                        if (width - *body_width.peek()).abs() > 0.5 {
                            *body_width.write() = width;
                        }
                    })
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .height(Size::fill())
                            .content(Content::Flex)
                            .spacing(18.)
                            .child(page_header(show_create, body_w >= COMPACT_BELOW_PX))
                            .maybe_child((!inline_search).then(|| {
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .content(Content::Flex)
                                    .spacing(8.)
                                    .child(search_input(query, Size::flex(1.)))
                                    .child(filters.clone())
                                    .into_element()
                            }))
                            .child(toolbar(tabs, query, inline_search.then_some(filters)))
                            .child(
                                ScrollArea::new()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .keyboard_paging()
                                    .scrollbar_gutter(true)
                                    .spacing(22.)
                                    .append_children((!shown_lines.is_empty()).then(|| {
                                        section(
                                            "OneClient",
                                            t("oneclient.clusters.page.section.oneclient"),
                                            fixed_grid(line_cards, columns, item_height, gap),
                                        )
                                    }))
                                    .append_children(
                                        [
                                            (
                                                t("oneclient.clusters.page.section.custom"),
                                                t("oneclient.clusters.page.section.custom_caption"),
                                                &shown_custom,
                                            ),
                                            (
                                                t("oneclient.clusters.page.section.modpacks"),
                                                t("oneclient.clusters.page.section.modpacks_caption"),
                                                &shown_packs,
                                            ),
                                        ]
                                        .into_iter()
                                        .filter(|(_, _, list)| !list.is_empty())
                                        .map(
                                            |(title, caption, list)| {
                                                section(
                                                    title,
                                                    caption,
                                                    fixed_grid(
                                                        instance_rows(list),
                                                        columns,
                                                        item_height,
                                                        gap,
                                                    ),
                                                )
                                            },
                                        ),
                                    )
                                    .append_children(nothing_shown.then(|| {
                                        label()
                                            .text(t("oneclient.clusters.page.no_matches"))
                                            .font_size(text_size(13.))
                                            .color(colors::fg_secondary())
                                            .into_element()
                                    })),
                            ),
                    )
                    .child(sidebar),
            )
            .maybe_child(create_modal(show_create))
    }
}

fn create_modal(mut show_create: State<bool>) -> Option<Element> {
    show_create
        .read()
        .then(|| CreateInstanceModal::new(move |()| show_create.set(false)).into_element())
}

pub(crate) fn shown<'a>(
    list: &[&'a Cluster],
    sort: Sort,
    keep: impl Fn(&Cluster) -> bool,
) -> Vec<&'a Cluster> {
    let mut list: Vec<&Cluster> = list.iter().copied().filter(|c| keep(c)).collect();
    match sort {
        Sort::RecentFirst => {}
        Sort::OldestFirst => list.reverse(),
        Sort::NameAsc => list.sort_by_key(|c| c.name.to_lowercase()),
        Sort::NameDesc => list.sort_by_key(|c| std::cmp::Reverse(c.name.to_lowercase())),
    }
    list
}

fn cluster_caption(cluster: &Cluster) -> String {
    format!("{} \u{b7} {}", cluster.mc_version, cluster.mc_loader)
}

/// Width of one grid cell once the gaps between `columns` cells are taken out
pub(super) fn card_width_for(width: f32, columns: usize, gap: f32) -> f32 {
    let columns = columns.max(1) as f32;
    ((width - gap * (columns - 1.)) / columns).max(0.)
}

fn picker_caption(cluster: &Cluster) -> String {
    if cluster.user_created {
        format!("{} \u{b7} {}", cluster.name, cluster_caption(cluster))
    } else {
        crate::utils::builtin_oneclient_title(cluster).unwrap_or_else(|| cluster_caption(cluster))
    }
}

fn default_line_cluster(clusters: &[Cluster], active: Option<&Cluster>) -> Option<i64> {
    if let Some(active) = active.filter(|a| clusters.iter().any(|c| c.id == a.id)) {
        return Some(active.id);
    }
    let preferred_key = active
        .and_then(|c| parse_mc_version(&c.mc_version))
        .and_then(|p| p.key());
    resolve_cluster(
        clusters,
        default_version_key(clusters, preferred_key),
        default_loader(clusters, active.map(|c| c.mc_loader)),
    )
    .map(|c| c.id)
}

fn bottom_border() -> Border {
    Border::new()
        .fill(colors::component_border())
        .width(BorderWidth {
            top: 0.,
            right: 0.,
            bottom: 1.,
            left: 0.,
        })
}

fn section(title: &str, caption: &str, grid: Element) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(section_header(title, caption))
        .child(grid)
        .into_element()
}

pub(crate) fn section_header(title: &str, caption: &str) -> Element {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(10.)
        .child(
            label()
                .text(title.to_string())
                .font_size(text_size(14.))
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(caption.to_string())
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

pub(crate) fn toolbar(
    tabs: impl IntoIterator<Item = TabItem>,
    query: State<String>,
    inline_filters: Option<Element>,
) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(TOOLBAR_HEIGHT_PX))
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .spacing(12.)
        .border(bottom_border())
        .maybe_child(
            inline_filters
                .as_ref()
                .map(|_| search_input(query, Size::px(SEARCH_WIDTH_PX))),
        )
        .child(
            TabBar::new()
                .width(Size::auto())
                .height(Size::fill())
                .spacing(24.)
                .font_size(text_size(14.))
                .tabs(tabs),
        )
        .child(rect().width(Size::flex(1.0)))
        .maybe_child(inline_filters)
}

pub(crate) fn filter_menu(
    available: &[String],
    mut loaders: State<Vec<String>>,
    mut sort: State<Sort>,
) -> Element {
    let picked = loaders.read().clone();
    let sort_by = *sort.read();

    FilterMenu::new(!picked.is_empty() || sort_by != Sort::RecentFirst)
        .section(
            t("oneclient.clusters.page.sort_by"),
            Sort::ALL.map(|option| {
                FilterOption::new(option.label(), sort_by == option, move |()| {
                    sort.set(option)
                })
            }),
        )
        .section(
            t("oneclient.clusters.page.loader"),
            available.iter().map(|loader| {
                let name = loader.clone();
                FilterOption::new(loader.clone(), picked.contains(loader), move |()| {
                    let mut next = loaders.peek().clone();
                    if next.contains(&name) {
                        next.retain(|l| *l != name);
                    } else {
                        next.push(name.clone());
                    }
                    loaders.set(next);
                })
            }),
        )
        .into_element()
}

fn version_list_columns(width: f32) -> usize {
    let available = (width - SCROLLBAR_GUTTER_PX).max(1.);
    columns_for(available, MIN_ROW_WIDTH_PX, MAX_ROW_COLUMNS, ROW_GAP_PX)
}

fn layout_toggle(layout: State<ViewLayout>) -> Element {
    SegmentedControl::new(layout)
        .static_indicator()
        .height(34.)
        .icon_size(if colors::active_theme().pixel_icons {
            24.
        } else {
            15.
        })
        .equal_width(34.)
        .segment(
            Segment::new(ViewLayout::List)
                .icon(IconType::ParagraphWrap)
                .tooltip(t("oneclient.clusters.page.layout.list")),
        )
        .segment(
            Segment::new(ViewLayout::Grid)
                .icon(IconType::DotsGrid)
                .tooltip(t("oneclient.clusters.page.layout.grid")),
        )
        .into_element()
}

pub(crate) fn search_input(query: State<String>, width: Size) -> Element {
    TextInput::new(query)
        .width(width)
        .height(Size::px(34.))
        .placeholder(t("oneclient.clusters.page.search"))
        .leading(
            Icon::new(IconType::SearchMd)
                .size(if colors::active_theme().pixel_icons {
                    24.
                } else {
                    14.
                })
                .color(colors::fg_secondary())
                .into_element(),
        )
        .into_element()
}

fn line_picker(
    line: ReleaseLine,
    cluster: &Cluster,
    siblings: &[Cluster],
    mut selected_cluster: State<Option<i64>>,
) -> Option<Element> {
    if siblings.len() <= 1 {
        return None;
    }

    let current = siblings
        .iter()
        .find(|c| c.id == cluster.id)
        .unwrap_or(&siblings[0]);
    let others: Vec<&Cluster> = siblings.iter().filter(|c| c.id != current.id).collect();
    let options: Vec<String> = others.iter().map(|c| picker_caption(c)).collect();
    let ids: Vec<i64> = others.iter().map(|c| c.id).collect();

    Some(
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .child(
                label()
                    .text(tf(
                        "oneclient.clusters.page.instances_on",
                        &[&siblings.len(), &line_title(line, siblings)],
                    ))
                    .font_size(text_size(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_secondary()),
            )
            .child(
                Dropdown::new(picker_caption(current), options)
                    .outlined()
                    .width(Size::fill())
                    .height(Size::px(40.))
                    .on_select(move |idx: usize| {
                        if let Some(id) = ids.get(idx).copied() {
                            selected_cluster.set(Some(id));
                        }
                    }),
            )
            .into_element(),
    )
}

#[derive(Clone, PartialEq)]
struct Sidebar {
    width: f32,
    cluster_id: i64,
    uses_bundles: bool,
    menu: ClusterMenuTarget,
    art: DynamicArt,
    type_label: &'static str,
    title: String,
    subtitle: String,
    description: Option<String>,
    meta: Vec<(&'static str, String)>,
    folder: Option<PathBuf>,
    picker: Option<Element>,
}

impl Sidebar {
    fn base(cluster: &Cluster, width: f32) -> Self {
        Self {
            width,
            cluster_id: cluster.id,
            uses_bundles: cluster.uses_bundles(),
            menu: ClusterMenuTarget::for_cluster(cluster, true),
            art: DynamicArt::for_cluster(cluster).max_edge(ART_PREVIEW_EDGE),
            type_label: "OneClient",
            title: crate::utils::instance_display_name(cluster),
            subtitle: if !cluster.is_isolated() && !cluster.user_created {
                format!("OneClient {}", cluster.mc_loader)
            } else {
                cluster_caption(cluster)
            },
            description: None,
            meta: vec![
                ("Minecraft", cluster.mc_version.clone()),
                (
                    t("oneclient.clusters.page.loader"),
                    cluster.mc_loader.to_string(),
                ),
                (
                    t("oneclient.clusters.page.meta.last_played"),
                    last_played_label(cluster.last_played),
                ),
            ],
            folder: cluster.dir().ok(),
            picker: None,
        }
    }

    fn for_instance(cluster: &Cluster, width: f32) -> Self {
        let mut sidebar = Self::base(cluster, width);
        sidebar.type_label = if cluster.linked_modpack_hash.is_some() {
            t("oneclient.clusters.page.type.modpack")
        } else {
            t("oneclient.clusters.page.type.custom")
        };
        sidebar.description = cluster.description.clone();
        sidebar.meta.push((
            t("oneclient.clusters.page.meta.playtime"),
            format_duration(i64::try_from(cluster.overall_played.as_secs()).unwrap_or(i64::MAX)),
        ));
        sidebar
    }
}

#[derive(PartialEq)]
struct LineSidebar {
    major: u32,
    key: Option<VersionKey>,
    loader: GameLoader,
    user_created: bool,
    base: Sidebar,
}

impl Component for LineSidebar {
    fn render(&self) -> impl IntoElement {
        let metadata = use_version_metadata(Some(self.major), self.key, Some(self.loader));

        let mut sidebar = self.base.clone();
        if let Some(metadata) = metadata {
            if !self.user_created {
                sidebar.title = metadata.name;
            }
            sidebar.description = sidebar.description.or(metadata.long_description);
        }
        sidebar
    }
}

impl Component for Sidebar {
    fn render(&self) -> impl IntoElement {
        let mut sidebar_height = use_state(|| 0f32);
        let mut menu = use_state(|| None);
        let mut active_id = use_active_cluster_id();
        let dispatch = use_dispatch();
        let game = use_game_snapshot();
        let launcher = use_launcher();

        let cluster_id = self.cluster_id;
        let syncing = launch_syncing(&launcher, self.uses_bundles);
        let shared_dir_taken = use_shared_dir_taken(cluster_id);
        let art_height = art_height_for(*sidebar_height.read());
        let top_padding = if art_height.is_some() {
            16.
        } else {
            SIDEBAR_PADDING_PX
        };

        let menu_button = |surface| InstanceOverviewButton {
            position: menu,
            surface,
            name: self.title.clone(),
        };
        let theme = colors::active_theme();
        let panel = rect()
            .width(Size::px(self.width))
            .min_width(Size::px(self.width))
            .height(Size::fill())
            .vertical()
            .content(Content::Flex)
            .corner_radius(rounded(20.));
        let panel = if theme.liquid_glass {
            crate::theme::glass::panel(
                panel,
                &theme,
                crate::theme::rounded_px(16.),
                colors::page_elevated().with_a(154),
            )
        } else {
            panel
                .background(colors::page_elevated())
                .border(border_all_color(1., colors::component_border()))
        };
        let panel = panel
            .overflow(Overflow::Clip)
            .on_secondary_down(move |event: Event<PressEventData>| {
                if let PressEventData::Mouse(mouse) = event.data() {
                    menu.set(Some((
                        mouse.global_location.x as f32,
                        mouse.global_location.y as f32,
                    )));
                }
            })
            .maybe_child(
                art_height.map(|height| sidebar_art(self.art.clone(), self.type_label, height)),
            )
            .maybe_child(art_height.is_some().then(|| {
                rect()
                    .position(Position::new_absolute().top(14.).right(14.))
                    .layer(Layer::Relative(7))
                    .child(menu_button(InstanceOverviewSurface::Artwork {
                        reveal: true,
                    }))
            }))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .content(Content::Flex)
                    .padding(Gaps::new(
                        top_padding,
                        SIDEBAR_PADDING_PX,
                        SIDEBAR_PADDING_PX,
                        SIDEBAR_PADDING_PX,
                    ))
                    .spacing(SIDEBAR_ACTION_GAP_PX)
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .keyboard_paging()
                            .scrollbar_gutter(true)
                            .bottom_fade(colors::page_elevated())
                            .bottom_fade_height(18.)
                            .spacing(18.)
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::fill())
                                    .spacing(4.)
                                    .child(
                                        label()
                                            .text(self.title.clone())
                                            .font_size(text_size(24.))
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_strong()),
                                    )
                                    .child(
                                        label()
                                            .text(self.subtitle.clone())
                                            .font_size(text_size(13.))
                                            .color(colors::fg_secondary()),
                                    ),
                            )
                            .append_children(self.picker.clone())
                            .child(meta_list(&self.meta))
                            .append_children(self.description.clone().map(|text| {
                                label()
                                    .text(text)
                                    .font_size(text_size(12.))
                                    .color(colors::fg_secondary())
                                    .into_element()
                            })),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::Flex)
                            .height(Size::px(SIDEBAR_ACTION_HEIGHT_PX))
                            .cross_align(Alignment::Center)
                            .spacing(8.)
                            .child(play_button(
                                cluster_id,
                                dispatch,
                                launch_button_state(&game, cluster_id, syncing, shared_dir_taken),
                            ))
                            .child(
                                Button::new()
                                    .secondary()
                                    .icon()
                                    .tooltip(t("oneclient.clusters.page.instance_settings"))
                                    .on_press(move |_| {
                                        *active_id.write() = Some(cluster_id);
                                        let _ = RouterContext::get()
                                            .push(Route::ClusterSettings { cluster_id });
                                    })
                                    .child(Icon::new(IconType::Settings01).size(16.)),
                            )
                            .maybe_child(self.folder.clone().map(open_folder_button))
                            .maybe_child(
                                art_height
                                    .is_none()
                                    .then(|| menu_button(InstanceOverviewSurface::List)),
                            ),
                    ),
            );

        rect()
            .width(Size::px(self.width))
            .min_width(Size::px(self.width))
            .height(Size::fill())
            .on_sized(move |event: Event<SizedEventData>| {
                sidebar_height.set_if_modified(event.area.height());
            })
            .child(panel)
            .child(ClusterContextMenu {
                target: self.menu.clone(),
                position: menu,
                upwards: false,
            })
    }
}

fn sidebar_art(art: DynamicArt, type_label: &str, height: f32) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::px(height))
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .position(Position::new_absolute())
                .child(art),
        )
        .child(
            rect()
                .position(Position::new_absolute().top(14.).left(14.))
                .layer(Layer::Relative(4))
                .padding(Gaps::new_symmetric(5., 10.))
                .corner_radius(rounded(8.))
                .background(Color::from_af32rgb(0.66, 11, 16, 19))
                .child(
                    label()
                        .text(type_label.to_string())
                        .font_size(text_size(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::on_media()),
                ),
        )
        .into_element()
}

fn meta_list(meta: &[(&'static str, String)]) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .children(meta.iter().map(|(key, value)| {
            rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::SpaceBetween)
                .padding(Gaps::new_symmetric(9., 0.))
                .child(
                    label()
                        .text(key.to_string())
                        .font_size(text_size(13.))
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(value.clone())
                        .font_size(text_size(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .into_element()
        }))
        .into_element()
}

fn art_height_for(sidebar_height: f32) -> Option<f32> {
    if sidebar_height <= 0. {
        return Some(ART_MAX_HEIGHT_PX);
    }
    if sidebar_height < ART_MIN_SIDEBAR_PX {
        return None;
    }
    Some((sidebar_height * 0.32).min(ART_MAX_HEIGHT_PX))
}

fn play_button(
    cluster_id: i64,
    dispatch: crate::Actions,
    state: (&'static str, bool),
) -> impl IntoElement {
    let (label, enabled) = state;
    let button = Button::new()
        .primary()
        .width(Size::flex(1.))
        .enabled(enabled)
        .on_press(move |_| {
            if enabled {
                dispatch.launch_cluster(cluster_id);
            }
        });
    launch_button_label(button, label)
}

fn page_header(mut show_create: State<bool>, show_description: bool) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::End)
        .content(Content::Flex)
        .spacing(16.)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.))
                .spacing(6.)
                .child(
                    label()
                        .text(t("oneclient.clusters.page.title"))
                        .font_size(text_size(32.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_strong()),
                )
                .maybe_child(show_description.then(|| {
                    label()
                        .text(t("oneclient.clusters.page.description"))
                        .font_size(text_size(13.))
                        .color(colors::fg_secondary())
                        .into_element()
                })),
        )
        .child(
            Button::new()
                .primary()
                .on_press(move |_| show_create.set(true))
                .child(Icon::new(IconType::Plus).size(16.))
                .text(t("oneclient.clusters.page.new_instance")),
        )
}

fn sidebar_error(width: f32) -> Element {
    rect()
        .width(Size::px(width))
        .height(Size::fill())
        .vertical()
        .padding(16.)
        .spacing(8.)
        .corner_radius(rounded(20.))
        .border(border_all_color(1., colors::component_border()))
        .background(colors::component_bg())
        .child(
            label()
                .text(t("oneclient.clusters.page.error.title"))
                .font_size(text_size(14.))
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(t("oneclient.clusters.page.error.hint"))
                .font_size(text_size(14.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card_metrics(main_w: f32) -> (usize, f32) {
        let grid_w = main_w - SCROLLBAR_GUTTER_PX;
        let columns = grid_columns_for_width(grid_w, MAX_CARD_WIDTH_PX, CARD_GAP_PX);
        (columns, card_width_for(grid_w, columns, CARD_GAP_PX))
    }

    #[test]
    fn card_grid_fits_compact_and_wide_windows() {
        let main_width = |window, sidebar| {
            window - PAGE_PADDING.left() - PAGE_PADDING.right() - sidebar - COLUMN_GAP_PX
        };
        let (columns, width) = card_metrics(main_width(800., COMPACT_SIDEBAR_WIDTH_PX));
        assert_eq!(columns, 2);
        assert!((175.0..=MAX_CARD_WIDTH_PX).contains(&width), "{width}");

        let (columns, width) = card_metrics(main_width(2000., SIDEBAR_WIDTH_PX));
        assert_eq!(columns, 5);
        assert!((280.0..=MAX_CARD_WIDTH_PX).contains(&width), "{width}");
    }

    #[test]
    fn list_adds_columns_only_when_each_row_has_room() {
        assert_eq!(version_list_columns(420.), 1);
        assert_eq!(version_list_columns(709.), 1);
        assert_eq!(version_list_columns(710.), 2);
        assert_eq!(version_list_columns(920.), 2);
        assert_eq!(version_list_columns(1560.), 4);
    }

    #[test]
    fn layout_switch_changes_both_ways_with_pointer_clicks() {
        use std::{cell::Cell, rc::Rc};

        fn app() -> impl IntoElement {
            let current = use_consume::<Rc<Cell<ViewLayout>>>();
            let layout = use_state(|| ViewLayout::Grid);
            current.set(*layout.read());
            layout_toggle(layout)
        }

        let (mut runner, current) = freya_testing::TestingRunner::new(
            app,
            Size2D::new(200., 80.),
            |runner| runner.provide_root_context(|| Rc::new(Cell::new(ViewLayout::Grid))),
            1.,
        );
        runner.poll_n(std::time::Duration::ZERO, 3);
        runner.move_cursor((20., 18.));
        runner.click_cursor((20., 18.));
        runner.poll_n(std::time::Duration::ZERO, 3);
        assert_eq!(current.get(), ViewLayout::List);

        runner.move_cursor((54., 18.));
        runner.click_cursor((54., 18.));
        runner.poll_n(std::time::Duration::ZERO, 3);
        assert_eq!(current.get(), ViewLayout::Grid);
    }

    #[test]
    fn card_width_accounts_for_gaps() {
        assert!((card_width_for(316., 2, 16.) - 150.).abs() < 1e-4);
        assert!((card_width_for(300., 1, 16.) - 300.).abs() < 1e-4);
        assert!(card_width_for(0., 3, 16.).abs() < 1e-4);
    }
}
