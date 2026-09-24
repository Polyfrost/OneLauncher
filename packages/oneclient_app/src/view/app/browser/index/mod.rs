use std::time::Duration;

use freya::animation::{
    AnimNum, Ease, Function, OnChange, OnCreation, use_animation_with_dependencies,
};
use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::parse_mc_version;
use oneclient_common::search::normalize_query;
use oneclient_content::packages::types::ProjectSummary;
use oneclient_content::packages::types::SearchSort;
use oneclient_content::packages::{ContentType, ProviderId};
use oneclient_core::VersionMetadata;
use oneclient_core::clusters::Cluster;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Dropdown, Icon, IconType, Pagination, ScrollArea, Segment, SegmentedControl, TextInput,
};
use crate::hooks::use_cluster;
use crate::hooks::{
    BROWSE_PAGE_SIZE, BrowserUiState, bundles_with_status_items, category_list,
    cluster_content_items, content_type_for_slug, pick_version_metadata, search_items,
    search_pending, search_total, settled_or_loading, use_browser_compat, use_browser_state_store,
    use_browser_type, use_bundles_with_status, use_cluster_content, use_clusters, use_debounced,
    use_package_categories, use_package_search, use_versions, use_view_state, versions_metadata,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::grid_columns_for_width;
use crate::view::app::cluster::supports_datapacks;

use super::{
    InstallSource, Installed, PackageBanner, Thumbnail, WorldInstallPrompt, installed_map,
    preferred_version,
};
use crate::utils::{abbreviate_number, sort_clusters_for_home};

mod cards;
mod sidebar;
mod skeletons;
use cards::{empty_state, grid_row, list_row};
use sidebar::CategorySidebar;
use skeletons::{SkeletonListRow, skeleton_grid_row};

const SCROLLBAR_GUTTER: f32 = 18.;
const CATEGORY_SIDEBAR_W: f32 = 196.;
const PROVIDER_LABELS_W: f32 = 900.;
const WIDE_SEARCH_W: f32 = 780.;
const SEARCH_W: f32 = 260.;
const SEARCH_COMPACT_W: f32 = 170.;
const CARD_H: f32 = 226.;
const BANNER_H: f32 = 74.;
const CARD_ICON: f32 = 46.;
/// How far the package icon drops past the banner into the card body
const CARD_ICON_OVERHANG: f32 = 14.;
const MAX_CARD_W: f32 = 300.;
const LIST_ROW_H: f32 = 78.;
const GRID_SPACING: f32 = 16.;
const LIST_SPACING: f32 = 8.;
const SEARCH_DEBOUNCE_MS: u64 = 250;

const SORTS: [(SearchSort, &str); 4] = [
    (SearchSort::Relevance, "Relevance"),
    (SearchSort::Downloads, "Downloads"),
    (SearchSort::Newest, "Newest"),
    (SearchSort::Updated, "Updated"),
];

const BROWSE_TYPES: [(&str, &str); 4] = [
    ("mod", "Mods"),
    ("texture", "Textures"),
    ("shader", "Shaders"),
    ("datapack", "Data packs"),
];

const DATAPACK_SLUG: &str = "datapack";

pub(crate) fn browsable_type(package_type: &str, mc_version: &str) -> String {
    if package_type == DATAPACK_SLUG && !supports_datapacks(mc_version) {
        BROWSE_TYPES[0].0.to_string()
    } else {
        package_type.to_string()
    }
}

fn type_title(package_type: &str) -> &'static str {
    BROWSE_TYPES
        .iter()
        .find(|(slug, _)| *slug == package_type)
        .map_or("Mods", |(_, title)| *title)
}

fn encode_package_id(provider: ProviderId, id: &str) -> String {
    format!("{}:{}", provider as u8, id)
}

#[derive(PartialEq)]
pub struct Browser {
    pub cluster_id: i64,
    pub package_type: String,
    /// Entered from the navbar so the cluster to install into is picked here
    pub pick_cluster: bool,
}

impl Component for Browser {
    fn render(&self) -> impl IntoElement {
        let mut last_type = use_browser_type();
        use_side_effect_with_deps(&self.package_type, move |package_type| {
            last_type.set_if_modified(package_type.clone());
        });

        BrowserBody {
            cluster_id: self.cluster_id,
            package_type: self.package_type.clone(),
            pick_cluster: self.pick_cluster,
            key: DiffKey::None,
        }
        .key((self.cluster_id, self.package_type.as_str()))
    }
}

#[derive(PartialEq)]
struct BrowserBody {
    cluster_id: i64,
    package_type: String,
    pick_cluster: bool,
    key: DiffKey,
}

impl KeyExt for BrowserBody {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for BrowserBody {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let package_type = self.package_type.clone();
        let content_type = content_type_for_slug(&package_type);

        let store = use_browser_state_store();
        let state_key = format!("{cluster_id}:{package_type}");
        let saved = store.peek().get(&state_key).cloned().unwrap_or_default();

        let query = use_state(|| saved.query.clone());
        let provider = use_state(|| saved.provider);
        let view_mode = use_view_state(&format!("browser.{package_type}")).layout;
        let compatible_only = use_browser_compat();
        let selected_categories = use_state(|| saved.categories.clone());
        let sort = use_state(|| saved.sort);
        let page = use_state(|| saved.page);

        {
            let mut store = store;
            let key = state_key.clone();
            use_side_effect(move || {
                let snapshot = BrowserUiState {
                    query: query.read().clone(),
                    provider: *provider.read(),
                    categories: selected_categories.read().clone(),
                    sort: *sort.read(),
                    page: *page.read(),
                };
                store.write().insert(key.clone(), snapshot);
            });
        }

        // Debounce the search query so typing doesn't fire a request per keystroke
        let debounced_query = use_debounced(
            query.read().clone(),
            Duration::from_millis(SEARCH_DEBOUNCE_MS),
        );

        let cluster = use_cluster(cluster_id);
        let provider_id = *provider.read();
        let compat = *compatible_only.read();
        let cats = selected_categories.read().clone();

        let (game_versions, loaders) = match (compat, &cluster) {
            (true, Some(c)) => {
                let loaders = if content_type == ContentType::Mod {
                    vec![c.mc_loader]
                } else {
                    Vec::new()
                };
                (vec![c.mc_version.clone()], loaders)
            }
            _ => (Vec::new(), Vec::new()),
        };

        let mut page_state = page;
        // Normalised like the search key so a query differing only by a space does not reset the page
        let sort_by = *sort.read();
        let signature = format!(
            "{provider_id:?}|{}|{compat}|{}|{sort_by:?}",
            normalize_query(&debounced_query.read()).to_lowercase(),
            cats.join(",")
        );
        let mut last_signature = use_state(|| signature.clone());
        if *last_signature.peek() != signature {
            last_signature.set(signature);
            if *page_state.peek() != 0 {
                page_state.set(0);
            }
        }

        let search = use_package_search(
            provider_id,
            content_type,
            debounced_query.read().clone(),
            game_versions,
            loaders,
            cats.clone(),
            sort_by,
            *page.read(),
        );
        let categories_query = use_package_categories(provider_id, content_type);
        let all_categories = category_list(&categories_query);

        let installed = installed_map(
            cluster_content_items(&use_cluster_content(cluster_id, content_type)),
            &bundles_with_status_items(&use_bundles_with_status(cluster_id)),
        );
        let installed = if content_type == ContentType::DataPack {
            Default::default()
        } else {
            installed
        };

        let packages = search_items(&search);
        let total = search_total(&search);
        let pending = search_pending(&search);
        let current_page = *page.read();

        // A page switch reports no total until it settles holding the last count keeps the pager in place
        let live_pages = (total > 0).then(|| total.div_ceil(BROWSE_PAGE_SIZE).max(1));
        let mut settled_pages = use_state(|| live_pages);
        if !pending {
            settled_pages.set_if_modified(live_pages);
        }
        let pages = if pending {
            *settled_pages.read()
        } else {
            live_pages
        };

        let mode = *view_mode.read();

        let mut grid_width = use_state(|| 0f32);
        let controls_width = use_state(|| 0f32);
        let cols = grid_columns_for_width(
            (*grid_width.read() - SCROLLBAR_GUTTER).max(0.),
            MAX_CARD_W,
            GRID_SPACING,
        );

        let fade_dep = (current_page, packages.is_empty(), pending, mode);
        let fade = use_animation_with_dependencies(&fade_dep, |conf, _| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            AnimNum::new(0., 1.)
                .time(240)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let fade_opacity = fade.read().value();

        let sa = ScrollArea::new()
            .reset_key(current_page as u64)
            .padding(Gaps::new(0., SCROLLBAR_GUTTER, 0., 0.));

        let pkg = package_type.clone();
        let results = if !packages.is_empty() {
            match mode {
                ViewLayout::Grid => {
                    let rows: Vec<Vec<ProjectSummary>> =
                        packages.chunks(cols).map(|c| c.to_vec()).collect();

                    sa.lazy(rows.len(), CARD_H, GRID_SPACING, move |i| {
                        grid_row(rows[i].clone(), cluster_id, &pkg, &installed, cols).into_element()
                    })
                }
                ViewLayout::List => sa.lazy(packages.len(), LIST_ROW_H, LIST_SPACING, move |i| {
                    list_row(packages[i].clone(), cluster_id, &pkg, &installed).into_element()
                }),
            }
        } else if pending {
            match mode {
                ViewLayout::Grid => {
                    let rows = BROWSE_PAGE_SIZE.div_ceil(cols);
                    sa.lazy(rows, CARD_H, GRID_SPACING, move |_| {
                        skeleton_grid_row(cols).into_element()
                    })
                }
                ViewLayout::List => sa.lazy(BROWSE_PAGE_SIZE, LIST_ROW_H, LIST_SPACING, |_| {
                    SkeletonListRow.into_element()
                }),
            }
        } else {
            sa.children([empty_state().into_element()])
        }
        .into_element();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(Gaps::new(0., 40., 40., 40.))
            .spacing(18.)
            .child(header(
                &package_type,
                cluster.as_ref().map(|c| c.name.clone()),
                self.pick_cluster.then(|| ClusterPicker {
                    cluster_id,
                    package_type: package_type.clone(),
                }),
                provider,
                query,
                controls_width,
            ))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .spacing(24.)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(CATEGORY_SIDEBAR_W))
                            .height(Size::fill())
                            .spacing(14.)
                            .child(TypePicker {
                                cluster_id,
                                package_type: package_type.clone(),
                                pick_cluster: self.pick_cluster,
                            })
                            .maybe(!all_categories.is_empty(), |el| {
                                el.child(CategorySidebar {
                                    categories: all_categories,
                                    selected: selected_categories,
                                })
                            }),
                    )
                    .child(
                        rect()
                            .vertical()
                            .content(Content::Flex)
                            .width(Size::flex(1.0))
                            .height(Size::fill())
                            .spacing(12.)
                            .child(results_toolbar(
                                total,
                                current_page,
                                pages,
                                pending,
                                sort,
                                view_mode,
                            ))
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .child(results)
                                    .opacity(fade_opacity)
                                    .on_sized(move |event: Event<SizedEventData>| {
                                        let w = event.data().area.width();
                                        if (w - *grid_width.peek()).abs() > 0.5 {
                                            *grid_width.write() = w;
                                        }
                                    }),
                            )
                            .maybe_child(pages.map(|pages| {
                                Pagination::new(page, pages)
                                    .enabled(!pending)
                                    .into_element()
                            })),
                    ),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

fn header(
    package_type: &str,
    cluster_name: Option<String>,
    picker: Option<ClusterPicker>,
    provider: State<ProviderId>,
    query: State<String>,
    mut header_width: State<f32>,
) -> impl IntoElement {
    let measured = *header_width.read();
    let show_provider_labels = measured == 0. || measured >= PROVIDER_LABELS_W;
    let search_width = if measured == 0. || measured >= WIDE_SEARCH_W {
        SEARCH_W
    } else {
        SEARCH_COMPACT_W
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::End)
        .spacing(12.)
        .content(Content::Flex)
        .on_sized(move |event: Event<SizedEventData>| {
            let w = event.data().area.width();
            if (w - *header_width.peek()).abs() > 0.5 {
                *header_width.write() = w;
            }
        })
        .child(
            rect()
                .vertical()
                .spacing(10.)
                .child(
                    label()
                        .text(format!("Browse {}", type_title(package_type)))
                        .font_size(32.)
                        .font_weight(FontWeight::BOLD)
                        .color(colors::fg_primary()),
                )
                .maybe_child(match picker {
                    Some(picker) => Some(picker.into_element()),
                    // Without the picker the cluster is fixed so the same pill is read-only
                    None => cluster_name.map(|name| target_pill(&name).into_element()),
                }),
        )
        .child(rect().width(Size::flex(1.0)))
        .child(
            SegmentedControl::new(provider)
                .no_tint()
                .segments(ProviderId::REMOTE_PROVIDERS.iter().map(move |provider| {
                    let segment = Segment::new(*provider).icon(IconType::from(*provider));
                    if show_provider_labels {
                        segment.label(provider.to_string())
                    } else {
                        segment
                    }
                }))
                .into_element(),
        )
        .child(
            TextInput::new(query)
                .width(Size::px(search_width))
                .placeholder("Search for content")
                .leading(
                    Icon::new(IconType::SearchMd)
                        .size(14.)
                        .color(colors::fg_secondary())
                        .into_element(),
                ),
        )
}

/// Reads like the cluster dropdown next to it so a fixed target doesn't look like a missing control
fn target_pill(name: &str) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .height(Size::px(30.))
        .spacing(6.)
        .padding(Gaps::new_symmetric(0., 10.))
        .corner_radius(CornerRadius::new_all(6.))
        .background(colors::ghost_overlay())
        .child(
            label()
                .text("Installing to")
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(name.to_string())
                .font_size(12.)
                .max_lines(1)
                .color(colors::fg_primary()),
        )
}

fn results_toolbar(
    total: usize,
    current_page: usize,
    pages: Option<usize>,
    pending: bool,
    sort: State<SearchSort>,
    view_mode: State<ViewLayout>,
) -> impl IntoElement {
    // Blank rather than a stale count while a page is in flight
    let summary = match (pending, pages) {
        (false, Some(pages)) => format!(
            "{} results · page {} of {}",
            abbreviate_number(total as u64),
            current_page + 1,
            pages
        ),
        _ => String::new(),
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(8.)
        .content(Content::Flex)
        .child(
            label()
                .text(summary)
                .font_size(12.)
                .color(colors::fg_primary().with_a(140)),
        )
        .child(rect().width(Size::flex(1.0)))
        .child(SortPicker { sort })
        .child(
            SegmentedControl::new(view_mode)
                .equal_width(30.)
                .icon_size(15.)
                .segment(Segment::new(ViewLayout::Grid).icon(IconType::DotsGrid))
                .segment(Segment::new(ViewLayout::List).icon(IconType::LayoutTop)),
        )
}

#[derive(PartialEq)]
struct SortPicker {
    sort: State<SearchSort>,
}

impl Component for SortPicker {
    fn render(&self) -> impl IntoElement {
        let mut sort = self.sort;
        let current = *sort.read();

        let labels: Vec<String> = SORTS.iter().map(|(_, l)| (*l).to_string()).collect();
        let selected = SORTS
            .iter()
            .find(|(s, _)| *s == current)
            .map_or("Relevance", |(_, l)| *l);

        Dropdown::new(selected, labels)
            .width(Size::px(132.))
            .height(Size::px(30.))
            .leading(
                Icon::new(IconType::Sliders04)
                    .size(13.)
                    .color(colors::fg_secondary()),
            )
            .on_select(move |idx: usize| {
                if let Some((picked, _)) = SORTS.get(idx) {
                    sort.set(*picked);
                }
            })
    }
}

/// Falls back to the bare version while the manifest hasn't arrived or doesn't cover it
fn version_name(metadata: &[VersionMetadata], cluster: &Cluster) -> String {
    parse_mc_version(&cluster.mc_version)
        .and_then(|parsed| {
            pick_version_metadata(
                metadata,
                parsed.major,
                parsed.key(),
                Some(cluster.mc_loader),
            )
        })
        .map(|m| m.name)
        .unwrap_or_else(|| cluster.mc_version.clone())
}

#[derive(PartialEq)]
struct TypePicker {
    cluster_id: i64,
    package_type: String,
    pick_cluster: bool,
}

impl Component for TypePicker {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let pick_cluster = self.pick_cluster;
        let current = self.package_type.clone();
        let datapacks = use_cluster(cluster_id).is_none_or(|c| supports_datapacks(&c.mc_version));

        let types: Vec<(&str, &str)> = BROWSE_TYPES
            .into_iter()
            .filter(|(slug, _)| datapacks || *slug != DATAPACK_SLUG)
            .collect();
        let labels: Vec<String> = types
            .iter()
            .map(|(_, title)| (*title).to_string())
            .collect();
        let selected = type_title(&self.package_type).to_string();

        Dropdown::new(selected, labels)
            .width(Size::px(CATEGORY_SIDEBAR_W))
            .height(Size::px(30.))
            .on_select(move |idx: usize| {
                let picked = types.get(idx).filter(|(slug, _)| *slug != current);
                if let Some((slug, _)) = picked {
                    let _ = RouterContext::get().push(Route::Browser {
                        cluster_id,
                        package_type: (*slug).to_string(),
                        pick_cluster,
                    });
                }
            })
    }
}

/// The choice lives in the route so back and forward keep the cluster being browsed
#[derive(PartialEq)]
struct ClusterPicker {
    cluster_id: i64,
    package_type: String,
}

impl Component for ClusterPicker {
    fn render(&self) -> impl IntoElement {
        let clusters =
            sort_clusters_for_home(settled_or_loading(&use_clusters()).unwrap_or_default());
        let metadata = versions_metadata(&use_versions()).unwrap_or_default();

        let labels: Vec<String> = clusters
            .iter()
            .map(|c| format!("{} · {}", c.name, version_name(&metadata, c)))
            .collect();
        let ids: Vec<i64> = clusters.iter().map(|c| c.id).collect();
        let versions: Vec<String> = clusters.iter().map(|c| c.mc_version.clone()).collect();

        let selected = ids
            .iter()
            .position(|id| *id == self.cluster_id)
            .and_then(|idx| labels.get(idx).cloned())
            .unwrap_or_else(|| "Select a version".to_string());

        let package_type = self.package_type.clone();

        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .margin(Gaps::new(4., 0., 0., 0.))
            .child(
                Dropdown::new(selected, labels)
                    .width(Size::px(240.))
                    .height(Size::px(28.))
                    .on_select(move |idx: usize| {
                        if let (Some(cluster_id), Some(mc_version)) =
                            (ids.get(idx).copied(), versions.get(idx))
                        {
                            let _ = RouterContext::get().replace(Route::Browser {
                                cluster_id,
                                package_type: browsable_type(&package_type, mc_version),
                                pick_cluster: true,
                            });
                        }
                    }),
            )
    }
}
