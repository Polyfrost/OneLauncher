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
    use_bundles_with_status, use_cluster_content, use_clusters, use_debounced,
    use_package_categories, use_package_search, use_versions, use_view_state, versions_metadata,
};
use crate::routes::Route;
use crate::theme::colors;

use super::{
    InstallSource, Installed, PackageBanner, Thumbnail, installed_badge, installed_badge_overlay,
    installed_map,
};
use crate::utils::{abbreviate_number, sort_clusters_for_home};

mod cards;
mod sidebar;
mod skeletons;
use cards::{empty_state, grid_row, list_row};
use sidebar::CategorySidebar;
use skeletons::{SkeletonListRow, skeleton_grid_row};

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const CARD_NAME: Color = Color::from_rgb(213, 219, 255);
const GRID_COLUMNS: usize = 3;
const SCROLLBAR_GUTTER: f32 = 18.;
const CARD_H: f32 = 240.;
const BANNER_H: f32 = 100.;
const LIST_ROW_H: f32 = 78.;
const GRID_SPACING: f32 = 12.;
const LIST_SPACING: f32 = 8.;
const SEARCH_DEBOUNCE_MS: u64 = 250;

fn type_title(package_type: &str) -> &'static str {
    match package_type {
        "shader" => "Shaders",
        "texture" => "Textures",
        _ => "Mods",
    }
}

fn encode_package_id(provider: ProviderId, id: &str) -> String {
    format!("{}:{}", provider as u8, id)
}

#[derive(PartialEq)]
pub struct Browser {
    pub cluster_id: i64,
    pub package_type: String,
    /// Entered from the navbar rather than from a cluster's content page, so
    /// the cluster it browses for is picked here instead of being decided.
    pub pick_cluster: bool,
}

impl Component for Browser {
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
        let page = use_state(|| saved.page);

        {
            let mut store = store;
            let key = state_key.clone();
            use_side_effect(move || {
                let snapshot = BrowserUiState {
                    query: query.read().clone(),
                    provider: *provider.read(),
                    categories: selected_categories.read().clone(),
                    page: *page.read(),
                };
                store.write().insert(key.clone(), snapshot);
            });
        }

        // Debounce the search query so typing doesn't fire a request per keystroke.
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
        // Normalised the same way the search key is, so a query that only picked
        // up a space is not treated as a new search that resets the page.
        let signature = format!(
            "{provider_id:?}|{}|{compat}|{}",
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
            SearchSort::Relevance,
            *page.read(),
        );
        let categories_query = use_package_categories(provider_id, content_type);
        let all_categories = category_list(&categories_query);

        let installed = installed_map(
            cluster_content_items(&use_cluster_content(cluster_id, content_type)),
            &bundles_with_status_items(&use_bundles_with_status(cluster_id)),
        );

        let packages = search_items(&search);
        let total = search_total(&search);
        let pending = search_pending(&search);
        let current_page = *page.read();

        // A page switch re-runs the search, which reports no total until it
        // settles. Holding the last count keeps the pager in place — disabled
        // while the page loads — instead of dropping out and back in.
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
                        packages.chunks(GRID_COLUMNS).map(|c| c.to_vec()).collect();

                    sa.lazy(rows.len(), CARD_H, GRID_SPACING, move |i| {
                        grid_row(rows[i].clone(), cluster_id, &pkg, &installed).into_element()
                    })
                }
                ViewLayout::List => sa.lazy(packages.len(), LIST_ROW_H, LIST_SPACING, move |i| {
                    list_row(packages[i].clone(), cluster_id, &pkg, &installed).into_element()
                }),
            }
        } else if pending {
            match mode {
                ViewLayout::Grid => {
                    let rows = BROWSE_PAGE_SIZE.div_ceil(GRID_COLUMNS);
                    sa.lazy(rows, CARD_H, GRID_SPACING, |_| {
                        skeleton_grid_row().into_element()
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
            .spacing(16.)
            .child(page_title(
                &package_type,
                cluster.as_ref().map(|c| c.name.clone()),
                self.pick_cluster.then(|| ClusterPicker {
                    cluster_id,
                    package_type: package_type.clone(),
                }),
            ))
            .child(controls(provider, query, view_mode))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .spacing(24.)
                    .maybe(!all_categories.is_empty(), |el| {
                        el.child(CategorySidebar {
                            categories: all_categories,
                            selected: selected_categories,
                        })
                    })
                    .child(
                        rect()
                            .vertical()
                            .content(Content::Flex)
                            .width(Size::flex(1.0))
                            .height(Size::fill())
                            .spacing(12.)
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .child(results)
                                    .opacity(fade_opacity),
                            )
                            .maybe_child(pages.map(|pages| {
                                Pagination::new(page, pages)
                                    .enabled(!pending)
                                    .into_element()
                            })),
                    ),
            )
    }
}

fn page_title(
    package_type: &str,
    cluster_name: Option<String>,
    picker: Option<ClusterPicker>,
) -> impl IntoElement {
    rect()
        .vertical()
        .spacing(2.)
        .child(
            label()
                .text(format!("Browse {}", type_title(package_type)))
                .font_size(36.)
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .maybe_child(match picker {
            Some(picker) => Some(picker.into_element()),
            // Without the picker the cluster is fixed, so it's just a subtitle.
            None => cluster_name.map(|name| {
                label()
                    .text(format!("for {name}"))
                    .font_size(14.)
                    .color(colors::fg_secondary())
                    .into_element()
            }),
        })
}

/// The version's display name from the manifest ("Tricky Trials"), falling back
/// to the bare version while the manifest hasn't arrived or doesn't cover it.
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

/// Switches which cluster the browser installs into. The choice lives in the
/// route, so going back and forward keeps the cluster that was being browsed.
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
                label()
                    .text("for")
                    .font_size(14.)
                    .color(colors::fg_secondary()),
            )
            .child(
                Dropdown::new(selected, labels)
                    .width(Size::px(240.))
                    .height(Size::px(28.))
                    .on_select(move |idx: usize| {
                        if let Some(cluster_id) = ids.get(idx).copied() {
                            let _ = RouterContext::get().replace(Route::Browser {
                                cluster_id,
                                package_type: package_type.clone(),
                                pick_cluster: true,
                            });
                        }
                    }),
            )
    }
}

fn controls(
    provider: State<ProviderId>,
    query: State<String>,
    view_mode: State<ViewLayout>,
) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(12.)
        .content(Content::Flex)
        .child(rect().width(Size::flex(1.0)))
        .child(
            SegmentedControl::new(view_mode)
                .equal_width(30.)
                .icon_size(16.)
                .segment(Segment::new(ViewLayout::Grid).icon(IconType::DotsGrid))
                .segment(Segment::new(ViewLayout::List).icon(IconType::LayoutTop)),
        )
        .child(
            SegmentedControl::new(provider)
                .no_tint()
                .segments(ProviderId::REMOTE_PROVIDERS.iter().map(|provider| {
                    Segment::new(*provider)
                        .icon(IconType::from(*provider))
                        .label(provider.to_string())
                }))
                .into_element(),
        )
        .child(
            TextInput::new(query)
                .width(Size::px(260.))
                .placeholder("Search for content")
                .leading(
                    Icon::new(IconType::SearchMd)
                        .size(14.)
                        .color(colors::fg_secondary())
                        .into_element(),
                ),
        )
}
