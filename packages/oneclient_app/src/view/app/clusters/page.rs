use std::path::PathBuf;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::domain::GameLoader;
use oneclient_common::{VersionKey, parse_mc_version};
use oneclient_core::clusters::Cluster;

use crate::components::{
    ART_PREVIEW_EDGE, Button, Dropdown, DynamicArt, FilterMenu, FilterOption, Icon, IconType,
    InstanceRow, ScrollArea, TabBar, TabItem, TextInput, VersionCard, open_folder_button,
};
use crate::hooks::{
    settled_or_loading, use_active_cluster_id, use_clusters, use_dispatch, use_game_snapshot,
    use_launcher, use_version_metadata,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::{
    border_all_color, fixed_grid, grid_columns_for_width, last_played_label, window_logical_size,
};
use crate::utils::{
    GridSelection, ReleaseLine, default_line, default_loader, default_version_key, format_duration,
    line_title, resolve_cluster, split_clusters,
};
use crate::view::app::clusters::CreateInstanceModal;
use crate::view::app::{launch_button_state, launch_syncing};

const PAGE_PADDING: Gaps = Gaps::new(12., 28., 28., 28.);
const COLUMN_GAP_PX: f32 = 24.;
const CARD_GAP_PX: f32 = 10.;
const ROW_GAP_PX: f32 = 8.;
const CARD_HEIGHT_PX: f32 = 150.;
const ROW_HEIGHT_PX: f32 = 56.;
const MAX_CARD_WIDTH_PX: f32 = 300.;
const MAX_ROW_WIDTH_PX: f32 = 480.;
const SIDEBAR_WIDTH_PX: f32 = 360.;
const COMPACT_SIDEBAR_WIDTH_PX: f32 = 300.;
const COMPACT_BELOW_PX: f32 = 1000.;
const INLINE_SEARCH_MIN_PX: f32 = 700.;
const SEARCH_WIDTH_PX: f32 = 220.;
const TOOLBAR_HEIGHT_PX: f32 = 44.;
const ART_MAX_HEIGHT_PX: f32 = 200.;
const ART_MIN_SIDEBAR_PX: f32 = 420.;

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    OneClient,
    Custom,
    Modpacks,
}

impl Filter {
    fn shows(self, section: Filter) -> bool {
        self == Filter::All || self == section
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
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
            Sort::RecentFirst => "Recently played",
            Sort::OldestFirst => "Least recently played",
            Sort::NameAsc => "Name (A-Z)",
            Sort::NameDesc => "Name (Z-A)",
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
        let mut selected_version = use_state(|| None::<VersionKey>);
        let mut selected_loader = use_state(|| None::<GameLoader>);
        let mut filter = use_state(|| Filter::All);
        let query = use_state(String::new);
        let loaders = use_state(Vec::<String>::new);
        let sort = use_state(|| Sort::RecentFirst);
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
                .child(page_header(show_create))
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
                Some(cluster) if cluster.user_created => Some(GridSelection::Instance(cluster.id)),
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

        if line.is_some() && selected_version.read().is_none() {
            let preferred = active_cluster
                .as_ref()
                .and_then(|c| parse_mc_version(&c.mc_version))
                .and_then(|p| p.key());
            *selected_version.write() = default_version_key(&clusters_for_line, preferred);
        }

        if line.is_some() && selected_loader.read().is_none() {
            let preferred = active_cluster.as_ref().map(|c| c.mc_loader);
            *selected_loader.write() = default_loader(&clusters_for_line, preferred);
        }

        let cluster = resolve_cluster(
            &clusters_for_line,
            *selected_version.read(),
            *selected_loader.read(),
        )
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
        let card_columns = grid_columns_for_width(main_w, MAX_CARD_WIDTH_PX, CARD_GAP_PX);
        let row_columns = grid_columns_for_width(main_w, MAX_ROW_WIDTH_PX, ROW_GAP_PX);
        let inline_search = main_w >= INLINE_SEARCH_MIN_PX;

        let active_filter = *filter.read();
        let needle = query.read().trim().to_lowercase();
        let picked_loaders = loaders.read().clone();
        let matches = |c: &Cluster| {
            (picked_loaders.is_empty() || picked_loaders.contains(&c.mc_loader.to_string()))
                && (needle.is_empty()
                    || format!("{} {} {}", c.name, c.mc_version, c.mc_loader)
                        .to_lowercase()
                        .contains(&needle))
        };
        let sort_by = *sort.read();

        let shown_lines: Vec<ReleaseLine> = lines
            .iter()
            .copied()
            .filter(|l| active_filter.shows(Filter::OneClient) && groups[l].iter().any(matches))
            .collect();
        let shown_custom = shown(&custom, sort_by, |c| {
            active_filter.shows(Filter::Custom) && matches(c)
        });
        let shown_packs = shown(&packs, sort_by, |c| {
            active_filter.shows(Filter::Modpacks) && matches(c)
        });

        let mut available_loaders: Vec<String> =
            clusters.iter().map(|c| c.mc_loader.to_string()).collect();
        available_loaders.sort();
        available_loaders.dedup();
        let filters = filter_menu(&available_loaders, loaders, sort);
        let nothing_shown =
            shown_lines.is_empty() && shown_custom.is_empty() && shown_packs.is_empty();

        let tabs = [
            (Filter::All, "All", lines.len() + instances.len()),
            (Filter::OneClient, "OneClient", lines.len()),
            (Filter::Custom, "Custom", custom.len()),
            (Filter::Modpacks, "Modpacks", packs.len()),
        ]
        .map(|(value, name, count)| {
            TabItem::new(name, active_filter == value)
                .count_text(count.to_string())
                .on_press(move |_| filter.set(value))
        });

        let line_cards: Vec<Element> = shown_lines
            .iter()
            .map(|&line| {
                let list = &groups[&line];
                let item = GridSelection::Line(line);
                let is_selected = current == Some(item);
                let caption = match (is_selected, &cluster, list.as_slice()) {
                    (true, Some(c), _) | (_, _, [c]) => cluster_caption(c),
                    _ => format!("{} instances", list.len()),
                };
                VersionCard::new(line, list, caption, is_selected, move |_| {
                    selected.set(Some(item));
                    selected_version.set(None);
                    selected_loader.set(None);
                })
                .into_element()
            })
            .collect();

        let instance_rows = |list: &[&Cluster]| -> Vec<Element> {
            list.iter()
                .map(|c| {
                    let item = GridSelection::Instance(c.id);
                    InstanceRow::new(c, current == Some(item), move |_| selected.set(Some(item)))
                        .into_element()
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
                    base: Sidebar {
                        picker: line_picker(
                            line,
                            &cluster,
                            &clusters_for_line,
                            selected_version,
                            selected_loader,
                        ),
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
                            .child(page_header(show_create))
                            .child(toolbar(tabs, query, inline_search.then(|| filters.clone())))
                            .maybe_child((!inline_search).then(|| {
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .content(Content::Flex)
                                    .spacing(8.)
                                    .child(search_input(query, Size::flex(1.)))
                                    .child(filters)
                                    .into_element()
                            }))
                            .child(
                                ScrollArea::new()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .scrollbar_gutter(true)
                                    .spacing(22.)
                                    .append_children((!shown_lines.is_empty()).then(|| {
                                        section(
                                            "OneClient",
                                            "Grouped by Minecraft version",
                                            fixed_grid(
                                                line_cards,
                                                card_columns,
                                                CARD_HEIGHT_PX,
                                                CARD_GAP_PX,
                                            ),
                                        )
                                    }))
                                    .append_children(
                                        [
                                            (
                                                "Custom",
                                                "Your own instances, modded or vanilla",
                                                &shown_custom,
                                            ),
                                            ("Modpacks", "Installed from Browse", &shown_packs),
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
                                                        row_columns,
                                                        ROW_HEIGHT_PX,
                                                        ROW_GAP_PX,
                                                    ),
                                                )
                                            },
                                        ),
                                    )
                                    .append_children(nothing_shown.then(|| {
                                        label()
                                            .text("No instances match your search or filters.")
                                            .font_size(13.)
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

fn shown<'a>(
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
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::End)
                .spacing(10.)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(caption.to_string())
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(grid)
        .into_element()
}

fn toolbar(
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
        .border(bottom_border())
        .child(
            TabBar::new()
                .width(Size::auto())
                .height(Size::fill())
                .spacing(24.)
                .font_size(14.)
                .tabs(tabs),
        )
        .child(rect().width(Size::flex(1.0)))
        .maybe_child(inline_filters.map(|filters| {
            rect()
                .horizontal()
                .spacing(8.)
                .child(search_input(query, Size::px(SEARCH_WIDTH_PX)))
                .child(filters)
                .into_element()
        }))
}

fn filter_menu(
    available: &[String],
    mut loaders: State<Vec<String>>,
    mut sort: State<Sort>,
) -> Element {
    let picked = loaders.read().clone();
    let sort_by = *sort.read();

    FilterMenu::new(!picked.is_empty() || sort_by != Sort::RecentFirst)
        .section(
            "Sort by",
            Sort::ALL.map(|option| {
                FilterOption::new(option.label(), sort_by == option, move |()| {
                    sort.set(option)
                })
            }),
        )
        .section(
            "Loader",
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

fn search_input(query: State<String>, width: Size) -> Element {
    TextInput::new(query)
        .width(width)
        .height(Size::px(34.))
        .placeholder("Search instances")
        .leading(
            Icon::new(IconType::SearchMd)
                .size(14.)
                .color(colors::fg_secondary())
                .into_element(),
        )
        .into_element()
}

fn line_picker(
    line: ReleaseLine,
    cluster: &Cluster,
    siblings: &[Cluster],
    mut selected_version: State<Option<VersionKey>>,
    mut selected_loader: State<Option<GameLoader>>,
) -> Option<Element> {
    if siblings.len() <= 1 {
        return None;
    }

    let options: Vec<String> = siblings.iter().map(cluster_caption).collect();
    let current = siblings
        .iter()
        .position(|c| c.id == cluster.id)
        .unwrap_or(0);
    let picks: Vec<(Option<VersionKey>, GameLoader)> = siblings
        .iter()
        .map(|c| {
            let key = parse_mc_version(&c.mc_version).and_then(|p| p.key());
            (key, c.mc_loader)
        })
        .collect();

    Some(
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .child(
                label()
                    .text(format!(
                        "{} instances on {}",
                        siblings.len(),
                        line_title(line, siblings)
                    ))
                    .font_size(12.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_secondary()),
            )
            .child(
                Dropdown::new(options[current].clone(), options)
                    .outlined()
                    .width(Size::fill())
                    .height(Size::px(40.))
                    .on_select(move |idx: usize| {
                        if let Some((key, loader)) = picks.get(idx).copied() {
                            selected_version.set(key);
                            selected_loader.set(Some(loader));
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
            art: DynamicArt::for_cluster(cluster).max_edge(ART_PREVIEW_EDGE),
            type_label: "OneClient",
            title: cluster.name.clone(),
            subtitle: cluster_caption(cluster),
            description: None,
            meta: vec![
                ("Minecraft", cluster.mc_version.clone()),
                ("Loader", cluster.mc_loader.to_string()),
                ("Last played", last_played_label(cluster.last_played)),
            ],
            folder: cluster.dir().ok(),
            picker: None,
        }
    }

    fn for_instance(cluster: &Cluster, width: f32) -> Self {
        let mut sidebar = Self::base(cluster, width);
        sidebar.type_label = if cluster.linked_modpack_hash.is_some() {
            "Modpack"
        } else {
            "Custom"
        };
        sidebar.description = cluster.description.clone();
        sidebar.meta.push((
            "Playtime",
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
    base: Sidebar,
}

impl Component for LineSidebar {
    fn render(&self) -> impl IntoElement {
        let metadata = use_version_metadata(Some(self.major), self.key, Some(self.loader));

        let mut sidebar = self.base.clone();
        if let Some(metadata) = metadata {
            sidebar.title = metadata.name;
            sidebar.description = metadata.long_description;
        }
        sidebar
    }
}

impl Component for Sidebar {
    fn render(&self) -> impl IntoElement {
        let mut sidebar_height = use_state(|| 0f32);
        let mut active_id = use_active_cluster_id();
        let dispatch = use_dispatch();
        let game = use_game_snapshot();
        let launcher = use_launcher();

        let cluster_id = self.cluster_id;
        let syncing = launch_syncing(&launcher, self.uses_bundles);
        let art_height = art_height_for(*sidebar_height.read());
        let top_padding = if art_height.is_some() { 4. } else { 22. };

        rect()
            .width(Size::px(self.width))
            .min_width(Size::px(self.width))
            .height(Size::fill())
            .vertical()
            .content(Content::Flex)
            .corner_radius(CornerRadius::new_all(20.))
            .background(colors::page_elevated())
            .border(border_all_color(1., colors::component_border()))
            .overflow(Overflow::Clip)
            .on_sized(move |event: Event<SizedEventData>| {
                let height = event.data().area.height();
                if (*sidebar_height.peek() - height).abs() > 0.5 {
                    *sidebar_height.write() = height;
                }
            })
            .maybe_child(
                art_height.map(|height| sidebar_art(self.art.clone(), self.type_label, height)),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .content(Content::Flex)
                    .padding(Gaps::new(top_padding, 22., 22., 22.))
                    .spacing(18.)
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .scrollbar_gutter(true)
                            .spacing(18.)
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::fill())
                                    .spacing(4.)
                                    .child(
                                        label()
                                            .text(self.title.clone())
                                            .font_size(24.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(Color::WHITE),
                                    )
                                    .child(
                                        label()
                                            .text(self.subtitle.clone())
                                            .font_size(13.)
                                            .color(colors::fg_secondary()),
                                    ),
                            )
                            .append_children(self.picker.clone())
                            .child(meta_list(&self.meta))
                            .append_children(self.description.clone().map(|text| {
                                label()
                                    .text(text)
                                    .font_size(12.)
                                    .color(colors::fg_secondary())
                                    .into_element()
                            })),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::Flex)
                            .spacing(8.)
                            .child(play_button(
                                cluster_id,
                                dispatch,
                                launch_button_state(&game, cluster_id, syncing),
                            ))
                            .child(
                                Button::new()
                                    .secondary()
                                    .icon()
                                    .tooltip("Instance settings")
                                    .on_press(move |_| {
                                        *active_id.write() = Some(cluster_id);
                                        let _ = RouterContext::get()
                                            .push(Route::ClusterSettings { cluster_id });
                                    })
                                    .child(Icon::new(IconType::Settings01).size(16.)),
                            )
                            .maybe_child(self.folder.clone().map(open_folder_button)),
                    ),
            )
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
                .width(Size::fill())
                .height(Size::fill())
                .position(Position::new_absolute())
                .layer(Layer::Relative(3))
                .background(
                    LinearGradient::new()
                        .angle(0.)
                        .stop((colors::page_elevated().with_a(0), 40.))
                        .stop((colors::page_elevated().with_a(242), 100.)),
                ),
        )
        .child(
            rect()
                .position(Position::new_absolute().top(14.).left(14.))
                .layer(Layer::Relative(4))
                .padding(Gaps::new_symmetric(5., 10.))
                .corner_radius(CornerRadius::new_all(8.))
                .background(Color::from_af32rgb(0.66, 11, 16, 19))
                .child(
                    label()
                        .text(type_label.to_string())
                        .font_size(11.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
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
                .border(bottom_border())
                .child(
                    label()
                        .text(key.to_string())
                        .font_size(13.)
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(value.clone())
                        .font_size(13.)
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

fn page_header(mut show_create: State<bool>) -> impl IntoElement {
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
                        .text("Versions")
                        .font_size(32.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(Color::WHITE),
                )
                .child(
                    label()
                        .text("Pick a OneClient version, or launch one of your own instances.")
                        .font_size(13.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Button::new()
                .primary()
                .on_press(move |_| show_create.set(true))
                .child(Icon::new(IconType::Plus).size(16.))
                .text("New instance"),
        )
}

fn sidebar_error(width: f32) -> Element {
    rect()
        .width(Size::px(width))
        .height(Size::fill())
        .vertical()
        .padding(16.)
        .spacing(8.)
        .corner_radius(CornerRadius::new_all(20.))
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
                .text("Try selecting a different version or mod loader.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
