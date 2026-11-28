use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::domain::GameLoader;
use oneclient_common::parse_mc_version;
use oneclient_common::search::normalize_query;
use oneclient_content::packages::types::{SearchSide, SearchSort};
use oneclient_content::packages::{ContentType, ProviderId};
use oneclient_core::VersionMetadata;
use oneclient_core::clusters::Cluster;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Dropdown, Icon, IconType, Pagination, ScrollArea, Segment, SegmentedControl, TabBar, TabItem,
    TextInput,
};
use crate::hooks::use_cluster;
use crate::hooks::{
    BROWSE_PAGE_SIZE, BrowserUiState, bundle_overrides_map, bundles_with_status_items,
    category_list, cluster_content_items, content_type_for_slug, pick_version_metadata,
    query_error, search_items, search_pending, search_total, settled_or_loading,
    use_browser_compat, use_browser_state_store, use_browser_type, use_bundle_overrides,
    use_bundles_with_status, use_cluster_content, use_clusters, use_debounced,
    use_package_categories, use_package_search, use_versions, use_view_state, versions_metadata,
};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::{colors, text_size};
use crate::ui::grid_columns_for_width;
use crate::view::app::cluster::supports_datapacks;
use crate::view::app::clusters::ClusterPickerButton;

use super::{
    EnableButton, EnableVariant, InstallSource, Installed, ListStatus, ModpackVersionPrompt,
    Thumbnail, WorldInstallPrompt, control_radius, installed_map, load_failed, minecraft_choices,
    preferred_version,
};
use crate::utils::abbreviate_number;

mod cards;
mod instance;
mod override_modal;
mod sidebar;
mod skeletons;
use cards::{empty_state, grid_row, list_row};
use override_modal::OverrideButton;
pub(super) use override_modal::summary as override_summary;
use sidebar::CategorySidebar;
use skeletons::{SkeletonListRow, skeleton_grid_row};

const SCROLLBAR_GUTTER: f32 = 18.;
const CATEGORY_SIDEBAR_W: f32 = 196.;
const PROVIDER_LABELS_W: f32 = 900.;
const WIDE_SEARCH_W: f32 = 780.;
const SEARCH_W: f32 = 340.;
const SEARCH_COMPACT_W: f32 = 190.;
const CARD_H: f32 = 164.;
/// Icon, title and provider band at the top of each card.
const BANNER_H: f32 = 64.;
const CARD_ICON: f32 = 44.;
const MAX_CARD_W: f32 = 300.;
const LIST_ROW_H: f32 = 78.;
const GRID_SPACING: f32 = 16.;
const LIST_SPACING: f32 = 8.;
const SEARCH_DEBOUNCE_MS: u64 = 250;
/// Results column narrower than this drops secondary text and tightens the toolbar
const COMPACT_RESULTS_W: f32 = 620.;
/// Breathing room under the last row, the lists run to the window edge instead of stopping at a dead band
const SCROLL_END_INSET: f32 = 20.;
const PAGER_BOTTOM_INSET: f32 = 10.;
const PICKER_H: f32 = 30.;
const VIEW_TOGGLE_W: f32 = 68.;
/// Below this the result count would only ever show as a stub
const SUMMARY_MIN_W: f32 = 150.;
/// A picker trigger around its label: padding, the chevron and its gap, and a hair of slack
const PICKER_CHROME_W: f32 = 12. + 6. + 14. + 12. + 2.;
/// The sort trigger's leading icon and its gap
const SORT_ICON_W: f32 = 13. + 6.;

/// A picker as wide as its label once measured, `fallback` until then
fn picker_width(measured: f32, chrome: f32, fallback: f32) -> f32 {
    if measured > 0. {
        (measured + chrome).ceil()
    } else {
        fallback
    }
}

/// Measures a picker label off to the side, at the trigger's own type size
fn label_probe(text: &'static str, mut widths: State<[f32; 3]>, slot: usize) -> Element {
    rect()
        .position(Position::new_absolute().left(0.).top(0.))
        .opacity(0.)
        .on_sized(move |event: Event<SizedEventData>| {
            let w = event.data().area.width();
            if (w - widths.peek()[slot]).abs() > 0.5 {
                widths.write()[slot] = w;
            }
        })
        .child(label().text(text).max_lines(1).font_size(text_size(13.)))
        .into_element()
}

/// Every input that changes which results come back, so a change scrolls the list to the top
/// even when the page number stays the same
fn scroll_key(signature: &str, page: usize, grid: bool) -> u64 {
    let mut hasher = DefaultHasher::new();
    (signature, page, grid).hash(&mut hasher);
    hasher.finish()
}

/// Each provider names its categories differently, so a switch parks the picks under the provider
/// they belong to and brings back the new provider's own
fn swap_provider_categories(
    parked: &mut HashMap<ProviderId, Vec<String>>,
    from: ProviderId,
    to: ProviderId,
    current: Vec<String>,
) -> Vec<String> {
    parked.insert(from, current);
    parked.remove(&to).unwrap_or_default()
}

/// Only names the provider offers reach the search; until its list arrives the picks stand
fn known_categories(selected: &[String], offered: &[String]) -> Vec<String> {
    if offered.is_empty() {
        return selected.to_vec();
    }
    selected
        .iter()
        .filter(|c| offered.contains(c))
        .cloned()
        .collect()
}

fn sorts() -> [(SearchSort, &'static str); 4] {
    [
        (
            SearchSort::Relevance,
            t("oneclient.browser.index.sort.relevance"),
        ),
        (
            SearchSort::Downloads,
            t("oneclient.browser.index.sort.downloads"),
        ),
        (SearchSort::Newest, t("oneclient.browser.index.sort.newest")),
        (
            SearchSort::Updated,
            t("oneclient.browser.index.sort.updated"),
        ),
    ]
}

fn loaders() -> [(Option<GameLoader>, &'static str); 4] {
    [
        (None, t("oneclient.browser.index.loader.any")),
        (Some(GameLoader::Fabric), "Fabric"),
        (Some(GameLoader::Forge), "Forge"),
        (Some(GameLoader::NeoForge), "NeoForge"),
    ]
}

fn sides() -> [(Option<SearchSide>, &'static str); 3] {
    [
        (None, t("oneclient.browser.index.side.any")),
        (
            Some(SearchSide::Client),
            t("oneclient.browser.index.side.client"),
        ),
        (
            Some(SearchSide::Server),
            t("oneclient.browser.index.side.server"),
        ),
    ]
}

fn browse_types() -> [(&'static str, &'static str); 5] {
    [
        ("mod", t("oneclient.browser.index.type.mods")),
        ("texture", t("oneclient.browser.index.type.textures")),
        ("shader", t("oneclient.browser.index.type.shaders")),
        ("datapack", t("oneclient.browser.index.type.datapacks")),
        ("modpack", t("oneclient.browser.index.type.modpacks")),
    ]
}

const DATAPACK_SLUG: &str = "datapack";
const TEXTURE_SLUG: &str = "texture";
const LOADER_SLUGS: [&str; 2] = ["mod", "shader"];

pub(crate) fn browsable_type(package_type: &str, cluster: &Cluster) -> String {
    if cluster.lacks_mod_loader() {
        let unsupported = LOADER_SLUGS.contains(&package_type)
            || (package_type == DATAPACK_SLUG && !supports_datapacks(&cluster.mc_version));
        return if unsupported {
            TEXTURE_SLUG
        } else {
            package_type
        }
        .to_string();
    }
    if package_type == DATAPACK_SLUG && !supports_datapacks(&cluster.mc_version) {
        browse_types()[0].0.to_string()
    } else {
        package_type.to_string()
    }
}

fn type_title(package_type: &str) -> &'static str {
    browse_types()
        .iter()
        .find(|(slug, _)| *slug == package_type)
        .map_or(t("oneclient.browser.index.type.mods"), |(_, title)| *title)
}

pub(super) fn state_key(cluster_id: i64, package_type: &str, pick_cluster: bool) -> String {
    if pick_cluster && content_type_for_slug(package_type) == ContentType::Modpack {
        package_type.to_string()
    } else {
        format!("{cluster_id}:{package_type}")
    }
}

pub(crate) fn encode_package_id(provider: ProviderId, id: &str) -> String {
    format!("{}:{}", provider as u8, id)
}

#[derive(PartialEq)]
pub struct Browser {
    pub cluster_id: i64,
    pub package_type: String,
    /// Entered from the navbar so the cluster to install into is picked here
    pub pick_cluster: bool,
    pub in_instance: bool,
}

impl Component for Browser {
    fn render(&self) -> impl IntoElement {
        let mut last_type = use_browser_type();
        let in_instance = self.in_instance;
        use_side_effect_with_deps(
            &(self.package_type.clone(), in_instance),
            move |(package_type, in_instance)| {
                if !in_instance {
                    last_type.set_if_modified(package_type.clone());
                }
            },
        );

        let redirect = use_cluster(self.cluster_id)
            .filter(Cluster::lacks_mod_loader)
            .map(|c| browsable_type(&self.package_type, &c))
            .filter(|package_type| *package_type != self.package_type)
            .map(|package_type| Route::Browser {
                cluster_id: self.cluster_id,
                package_type,
                pick_cluster: self.pick_cluster,
                in_instance,
            });
        use_side_effect_with_deps(&redirect, move |redirect: &Option<Route>| {
            if let Some(route) = redirect.clone() {
                let _ = RouterContext::get().replace(route);
            }
        });

        BrowserBody {
            cluster_id: self.cluster_id,
            package_type: self.package_type.clone(),
            pick_cluster: self.pick_cluster,
            in_instance,
            key: DiffKey::None,
        }
        .key((
            self.cluster_id,
            self.package_type.as_str(),
            self.pick_cluster,
            in_instance,
        ))
    }
}

#[derive(PartialEq)]
struct BrowserBody {
    cluster_id: i64,
    package_type: String,
    pick_cluster: bool,
    in_instance: bool,
    key: DiffKey,
}

impl KeyExt for BrowserBody {
    fn key_mut(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for BrowserBody {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let package_type = self.package_type.clone();
        let content_type = content_type_for_slug(&package_type);
        let cluster_modpacks = content_type == ContentType::Modpack && !self.pick_cluster;

        let store = use_browser_state_store();
        let state_key = state_key(cluster_id, &package_type, self.pick_cluster);
        let saved = store.peek().get(&state_key).cloned().unwrap_or_default();

        let query = use_state(|| saved.query.clone());
        let mut provider = use_state(|| saved.provider);
        let view_mode = use_view_state(&format!("browser.{package_type}")).layout;
        let compatible_only = use_browser_compat();
        let selected_categories = use_state(|| saved.categories.clone());
        let mut parked_categories = use_state(|| saved.parked_categories.clone());
        let mut categories_provider = use_state(|| saved.provider);
        let sort = use_state(|| saved.sort);
        let loader = use_state(|| saved.loader);
        let side = use_state(|| saved.side);
        let search_override = use_state(|| saved.search_override.clone());
        let page = use_state(|| saved.page);

        // Debounce the search query so typing doesn't fire a request per keystroke
        let debounced_query = use_debounced(
            query.read().clone(),
            Duration::from_millis(SEARCH_DEBOUNCE_MS),
        );

        let cluster = use_cluster(cluster_id);
        let compat = *compatible_only.read();

        let targets_cluster = content_type != ContentType::Modpack;
        let overridden = search_override.read().clone().filter(|_| targets_cluster);
        let compat_cluster = cluster
            .as_ref()
            .filter(|_| compat && (targets_cluster || cluster_modpacks) && overridden.is_none());
        let game_versions = match &overridden {
            Some(o) => o.version.clone().into_iter().collect(),
            None => compat_cluster
                .map(|c| vec![c.mc_version.clone()])
                .unwrap_or_default(),
        };
        let loader_fixed = overridden.is_some()
            || compat_cluster.is_some_and(|c| {
                content_type == ContentType::Mod || (cluster_modpacks && c.mc_loader.is_modded())
            });
        let loader_filterable =
            matches!(content_type, ContentType::Mod | ContentType::Modpack) && !loader_fixed;
        let picked_loader = (*loader.read()).filter(|_| loader_filterable);
        let loaders = match (&overridden, compat_cluster) {
            (Some(o), _) => o
                .loader
                .filter(|_| content_type == ContentType::Mod)
                .into_iter()
                .collect(),
            (None, Some(c)) if loader_fixed => vec![c.mc_loader],
            _ => picked_loader.into_iter().collect(),
        };
        let curseforge_blocked = loader_fixed && loaders.contains(&GameLoader::Ornithe);
        if curseforge_blocked && *provider.peek() == ProviderId::CurseForge {
            provider.set(ProviderId::Modrinth);
        }
        let provider_id = *provider.read();
        let categories_from = *categories_provider.peek();
        if categories_from != provider_id {
            let mut selected = selected_categories;
            let current = selected.peek().clone();
            let restored = swap_provider_categories(
                &mut parked_categories.write(),
                categories_from,
                provider_id,
                current,
            );
            categories_provider.set(provider_id);
            selected.set(restored);
        }
        {
            let mut store = store;
            let key = state_key.clone();
            use_side_effect(move || {
                let snapshot = BrowserUiState {
                    query: query.read().clone(),
                    provider: *categories_provider.read(),
                    categories: selected_categories.read().clone(),
                    parked_categories: parked_categories.read().clone(),
                    sort: *sort.read(),
                    loader: *loader.read(),
                    side: *side.read(),
                    search_override: search_override.read().clone(),
                    page: *page.read(),
                };
                store.write().insert(key.clone(), snapshot);
            });
        }
        let categories_query = use_package_categories(provider_id, content_type);
        let all_categories = category_list(&categories_query);
        let cats = known_categories(&selected_categories.read(), &all_categories);
        let side_filterable =
            content_type == ContentType::Mod && provider_id == ProviderId::Modrinth;
        let picked_side = (*side.read()).filter(|_| side_filterable);

        let mut page_state = page;
        // Normalised like the search key so a query differing only by a space does not reset the page
        let sort_by = *sort.read();
        let signature = format!(
            "{provider_id:?}|{}|{compat}|{}|{sort_by:?}|{picked_loader:?}|{picked_side:?}|{overridden:?}",
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
            picked_side,
            *page.read(),
        );

        let installed = installed_map(
            cluster_content_items(&use_cluster_content(cluster_id, content_type)),
            &bundles_with_status_items(&use_bundles_with_status(cluster_id)),
            &bundle_overrides_map(&use_bundle_overrides(cluster_id)),
        );
        let installed = if matches!(content_type, ContentType::DataPack | ContentType::Modpack) {
            Default::default()
        } else {
            installed
        };

        let packages = search_items(&search);
        let total = search_total(&search);
        let pending = search_pending(&search);
        let failed = query_error(&search).is_some();
        let status = ListStatus::of(!packages.is_empty(), pending, failed);
        let retry = move |()| search.invalidate();
        let current_page = *page.read();
        let mode = *view_mode.read();
        let results_key = scroll_key(
            last_signature.peek().as_str(),
            current_page,
            mode == ViewLayout::Grid,
        );

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

        let target = cluster
            .as_ref()
            .filter(|_| targets_cluster || cluster_modpacks)
            .map(|c| {
                let verb = if cluster_modpacks {
                    t("oneclient.browser.index.target.adding")
                } else {
                    t("oneclient.browser.index.target.installing")
                };
                (verb, c.name.clone())
            });
        let override_button = |height: f32| {
            targets_cluster.then(|| OverrideButton {
                height,
                content_type,
                search_override,
                cluster: cluster
                    .as_ref()
                    .map(|c| (c.mc_loader, c.mc_version.clone())),
            })
        };

        // Inside the instance pop-up: the pop-up's rail, toolbar and rows, always as a list
        if self.in_instance {
            return instance::InstanceBrowse {
                cluster_id,
                package_type,
                target,
                override_button: override_button(instance::CONTROL_H),
                provider,
                curseforge_blocked,
                query,
                categories: all_categories,
                selected_categories,
                sort,
                loader: loader_filterable.then_some(loader),
                side: side_filterable.then_some(side),
                side_slot: content_type == ContentType::Mod,
                packages,
                installed,
                add_to_cluster: cluster_modpacks,
                search_override: overridden,
                pending,
                failed: status == ListStatus::Failed,
                on_retry: retry.into(),
                scroll_key: results_key,
                total,
                page,
                pages,
            }
            .render();
        }

        let mut grid_width = use_state(|| 0f32);
        let controls_width = use_state(|| 0f32);
        let label_widths = use_state(|| [0f32; 3]);
        let cols = grid_columns_for_width(
            (*grid_width.read() - SCROLLBAR_GUTTER).max(0.),
            MAX_CARD_W,
            GRID_SPACING,
        );

        let results_width = *grid_width.read();
        let compact = results_width > 0. && results_width < COMPACT_RESULTS_W;
        let end_inset = if (packages.is_empty() && !pending) || pages.is_some() {
            0.
        } else {
            SCROLL_END_INSET
        };

        let fade_surface = colors::page();
        let sa = ScrollArea::new()
            .bottom_fade(fade_surface)
            .primary_list(crate::components::PrimaryList::Browse)
            .reset_key(results_key)
            .padding(Gaps::new(0., SCROLLBAR_GUTTER, end_inset, 0.));

        let pkg = package_type.clone();
        let ovr = overridden.clone();
        let results = if !packages.is_empty() {
            match mode {
                ViewLayout::Grid => {
                    let rows = packages.len().div_ceil(cols);

                    sa.lazy(rows, CARD_H, GRID_SPACING, move |i| {
                        grid_row(
                            packages[i * cols..((i + 1) * cols).min(packages.len())].to_vec(),
                            cluster_id,
                            &pkg,
                            &installed,
                            cols,
                            cluster_modpacks,
                            ovr.clone(),
                        )
                        .into_element()
                    })
                }
                ViewLayout::List => sa.lazy(packages.len(), LIST_ROW_H, LIST_SPACING, move |i| {
                    list_row(
                        packages[i].clone(),
                        cluster_id,
                        &pkg,
                        &installed,
                        cluster_modpacks,
                        ovr.clone(),
                        compact,
                    )
                    .into_element()
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
        } else if status == ListStatus::Failed {
            sa.children([load_failed(t("oneclient.browser.index.load_failed"), retry)])
        } else {
            sa.children([empty_state().into_element()])
        }
        .into_element();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(Gaps::new(0., 40., 0., 40.))
            .spacing(18.)
            .child(header(
                &package_type,
                target,
                self.pick_cluster.then(|| ClusterPicker {
                    cluster_id,
                    package_type: package_type.clone(),
                    disabled: !targets_cluster,
                }),
                override_button(28.),
                provider,
                curseforge_blocked,
                query,
                controls_width,
            ))
            .child(TypePicker {
                cluster_id,
                package_type: package_type.clone(),
                pick_cluster: self.pick_cluster,
            })
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
                            .maybe(!all_categories.is_empty(), |el| {
                                el.child(CategorySidebar {
                                    categories: all_categories,
                                    selected: selected_categories,
                                    fade_surface,
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
                                loader_filterable.then_some(loader),
                                side_filterable.then_some(side),
                                view_mode,
                                results_width,
                                label_widths,
                            ))
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .cross_align(Alignment::Center)
                                    .child(results)
                                    .on_sized(move |event: Event<SizedEventData>| {
                                        let w = event.data().area.width();
                                        if (w - *grid_width.peek()).abs() > 0.5 {
                                            *grid_width.write() = w;
                                        }
                                    }),
                            )
                            .maybe_child(pages.map(|pages| {
                                rect()
                                    .width(Size::fill())
                                    .main_align(Alignment::Center)
                                    .padding(Gaps::new(
                                        0.,
                                        SCROLLBAR_GUTTER,
                                        PAGER_BOTTOM_INSET,
                                        0.,
                                    ))
                                    .child(
                                        Pagination::new(page, pages)
                                            .window(if compact { 3 } else { 5 })
                                            .enabled(!pending),
                                    )
                            })),
                    ),
            )
            .into_element()
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

#[allow(clippy::too_many_arguments)]
fn header(
    package_type: &str,
    target: Option<(&'static str, String)>,
    picker: Option<ClusterPicker>,
    override_button: Option<OverrideButton>,
    provider: State<ProviderId>,
    curseforge_blocked: bool,
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

    let providers = SegmentedControl::new(provider)
        .no_tint()
        .static_indicator()
        .segments(ProviderId::REMOTE_PROVIDERS.iter().map(move |provider| {
            let mut segment = Segment::new(*provider)
                .icon(IconType::from(*provider))
                .tooltip(provider.to_string());
            if curseforge_blocked && *provider == ProviderId::CurseForge {
                segment = segment
                    .disabled(true)
                    .tooltip(t("oneclient.browser.index.curseforge_blocked"));
            }
            if show_provider_labels {
                segment.label(provider.to_string())
            } else {
                segment
            }
        }));

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(12.)
        .on_sized(move |event: Event<SizedEventData>| {
            let w = event.data().area.width();
            if (w - *header_width.peek()).abs() > 0.5 {
                *header_width.write() = w;
            }
        })
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(12.)
                .child(
                    label()
                        .text(tf(
                            "oneclient.browser.index.title",
                            &[&type_title(package_type)],
                        ))
                        .font_size(text_size(32.))
                        .font_weight(FontWeight::BOLD)
                        .width(Size::flex(1.))
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(colors::fg_primary()),
                )
                .child(providers),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(
                    TextInput::new(query)
                        .width(Size::px(search_width))
                        .placeholder(t("oneclient.browser.index.search"))
                        .leading(
                            Icon::new(IconType::SearchMd)
                                .size(14.)
                                .color(colors::fg_secondary())
                                .into_element(),
                        ),
                )
                .maybe_child(match picker {
                    Some(picker) => Some(picker.into_element()),
                    None => target.map(|(verb, name)| target_pill(verb, &name).into_element()),
                })
                .maybe_child(override_button),
        )
}

/// Reads like the cluster dropdown next to it so a fixed target doesn't look like a missing control
fn target_pill(verb: &'static str, name: &str) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .height(Size::px(30.))
        .spacing(6.)
        .padding(Gaps::new_symmetric(0., 10.))
        .corner_radius(control_radius(30., 6.))
        .background(colors::ghost_overlay())
        .child(
            label()
                .text(verb)
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(name.to_string())
                .font_size(text_size(12.))
                .max_lines(1)
                .color(colors::fg_primary()),
        )
}

#[allow(clippy::too_many_arguments)]
fn results_toolbar(
    total: usize,
    current_page: usize,
    pages: Option<usize>,
    pending: bool,
    sort: State<SearchSort>,
    loader: Option<State<Option<GameLoader>>>,
    side: Option<State<Option<SearchSide>>>,
    view_mode: State<ViewLayout>,
    width: f32,
    label_widths: State<[f32; 3]>,
) -> impl IntoElement {
    const SPACING: f32 = 8.;
    let compact = width > 0. && width < COMPACT_RESULTS_W;
    let filter_w = if compact { 108. } else { 132. };
    // Each picker as wide as its label: the result count gives way before a picker is cut short
    let [side_label_w, loader_label_w, sort_label_w] = *label_widths.read();
    let side_w = picker_width(side_label_w, PICKER_CHROME_W, filter_w);
    let loader_w = picker_width(loader_label_w, PICKER_CHROME_W, filter_w);
    let sort_w = picker_width(sort_label_w, PICKER_CHROME_W + SORT_ICON_W, 132.);
    let controls = VIEW_TOGGLE_W
        + SPACING
        + sort_w
        + side.map_or(0., |_| side_w + SPACING)
        + loader.map_or(0., |_| loader_w + SPACING);
    // A count squeezed to a few letters is worse than none, the pager still says where you are
    let room_for_summary = width <= 0. || width - controls - SPACING >= SUMMARY_MIN_W;

    // Blank rather than a stale count while a page is in flight
    let summary = match (pending, pages) {
        (false, Some(pages)) if room_for_summary => tf(
            "oneclient.browser.index.summary",
            &[
                &abbreviate_number(total as u64),
                &(current_page + 1),
                &pages,
            ],
        ),
        _ => String::new(),
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(PICKER_H))
        .cross_align(Alignment::Center)
        .spacing(SPACING)
        .content(Content::Flex)
        .child(
            // Owns the flexible space so it truncates instead of sliding under the pickers
            label()
                .text(summary)
                .width(Size::flex(1.0))
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(text_size(12.))
                .color(colors::fg_primary().with_a(140)),
        )
        .maybe_child(side.map(|side| SidePicker {
            side,
            width: side_w,
            height: PICKER_H,
        }))
        .maybe_child(loader.map(|loader| LoaderPicker {
            loader,
            width: loader_w,
            height: PICKER_H,
        }))
        .child(SortPicker {
            sort,
            width: sort_w,
            height: PICKER_H,
        })
        // Probed across the whole toolbar's width so a label is never measured squeezed
        .maybe_child(side.map(|side| label_probe(side_label(*side.read()), label_widths, 0)))
        .maybe_child(
            loader.map(|loader| label_probe(loader_label(*loader.read()), label_widths, 1)),
        )
        .child(label_probe(sort_label(*sort.read()), label_widths, 2))
        .child(
            SegmentedControl::new(view_mode)
                .static_indicator()
                .height(PICKER_H)
                .equal_width(30.)
                .icon_size(15.)
                .segment(
                    Segment::new(ViewLayout::Grid)
                        .icon(IconType::DotsGrid)
                        .tooltip(t("oneclient.clusters.page.layout.grid")),
                )
                .segment(
                    Segment::new(ViewLayout::List)
                        .icon(IconType::LayoutTop)
                        .tooltip(t("oneclient.clusters.page.layout.list")),
                ),
        )
}

fn side_label(current: Option<SearchSide>) -> &'static str {
    sides()
        .iter()
        .find(|(s, _)| *s == current)
        .map_or(t("oneclient.browser.index.side.any"), |(_, l)| *l)
}

fn loader_label(current: Option<GameLoader>) -> &'static str {
    loaders()
        .iter()
        .find(|(l, _)| *l == current)
        .map_or(t("oneclient.browser.index.loader.any"), |(_, l)| *l)
}

fn sort_label(current: SearchSort) -> &'static str {
    sorts()
        .iter()
        .find(|(s, _)| *s == current)
        .map_or(t("oneclient.browser.index.sort.relevance"), |(_, l)| *l)
}

#[derive(PartialEq)]
struct SortPicker {
    sort: State<SearchSort>,
    width: f32,
    height: f32,
}

impl Component for SortPicker {
    fn render(&self) -> impl IntoElement {
        let mut sort = self.sort;
        let current = *sort.read();

        let labels: Vec<String> = sorts().iter().map(|(_, l)| (*l).to_string()).collect();
        let selected = sort_label(current);

        Dropdown::new(selected, labels)
            .width(Size::px(self.width))
            .height(Size::px(self.height))
            .leading(
                Icon::new(IconType::Sliders04)
                    .size(13.)
                    .color(colors::fg_secondary()),
            )
            .on_select(move |idx: usize| {
                if let Some((picked, _)) = sorts().get(idx) {
                    sort.set(*picked);
                }
            })
    }
}

#[derive(PartialEq)]
struct LoaderPicker {
    loader: State<Option<GameLoader>>,
    width: f32,
    height: f32,
}

impl Component for LoaderPicker {
    fn render(&self) -> impl IntoElement {
        let mut loader = self.loader;
        let current = *loader.read();

        let labels: Vec<String> = loaders().iter().map(|(_, l)| (*l).to_string()).collect();
        let selected = loader_label(current);

        Dropdown::new(selected, labels)
            .width(Size::px(self.width))
            .height(Size::px(self.height))
            .on_select(move |idx: usize| {
                if let Some((picked, _)) = loaders().get(idx) {
                    loader.set(*picked);
                }
            })
    }
}

#[derive(PartialEq)]
struct SidePicker {
    side: State<Option<SearchSide>>,
    width: f32,
    height: f32,
}

impl Component for SidePicker {
    fn render(&self) -> impl IntoElement {
        let mut side = self.side;
        let current = *side.read();

        let labels: Vec<String> = sides().iter().map(|(_, l)| (*l).to_string()).collect();
        let selected = side_label(current);

        Dropdown::new(selected, labels)
            .width(Size::px(self.width))
            .height(Size::px(self.height))
            .tooltip(t("oneclient.browser.index.side.tooltip"))
            .on_select(move |idx: usize| {
                if let Some((picked, _)) = sides().get(idx) {
                    side.set(*picked);
                }
            })
    }
}

/// Falls back to the bare version while the manifest hasn't arrived or doesn't cover it
fn version_name(metadata: &[VersionMetadata], cluster: &Cluster) -> String {
    if cluster.user_created {
        return format!("{} {}", cluster.mc_version, cluster.mc_loader);
    }
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
        let current = self.package_type.as_str();
        let cluster = use_cluster(cluster_id);
        let datapacks = cluster
            .as_ref()
            .is_none_or(|c| supports_datapacks(&c.mc_version));
        let mod_loader = cluster.as_ref().is_none_or(|c| !c.lacks_mod_loader());

        let tabs = browse_types()
            .into_iter()
            .filter(|(slug, _)| datapacks || *slug != DATAPACK_SLUG)
            .filter(|(slug, _)| mod_loader || !LOADER_SLUGS.contains(slug))
            .map(|(slug, title)| {
                let active = slug == current;
                let tab = TabItem::new(title, active);
                if active {
                    return tab;
                }
                let route = Route::Browser {
                    cluster_id,
                    package_type: slug.to_string(),
                    pick_cluster,
                    in_instance: false,
                };
                tab.on_press(move |_| {
                    let _ = RouterContext::get().push(route.clone());
                })
            });

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(38.))
            .cross_align(Alignment::Center)
            .border(
                Border::new()
                    .fill(colors::component_border())
                    .width(BorderWidth {
                        top: 0.,
                        right: 0.,
                        bottom: 1.,
                        left: 0.,
                    }),
            )
            .child(
                TabBar::new()
                    .width(Size::auto())
                    .height(Size::fill())
                    .spacing(22.)
                    .font_size(text_size(13.))
                    .tabs(tabs),
            )
    }
}

/// The choice lives in the route so back and forward keep the cluster being browsed
#[derive(PartialEq)]
struct ClusterPicker {
    cluster_id: i64,
    package_type: String,
    disabled: bool,
}

impl Component for ClusterPicker {
    fn render(&self) -> impl IntoElement {
        let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
        let metadata = versions_metadata(&use_versions()).unwrap_or_default();
        let selected = clusters
            .iter()
            .find(|c| c.id == self.cluster_id)
            .map(|c| format!("{} · {}", c.name, version_name(&metadata, c)))
            .unwrap_or_else(|| t("oneclient.browser.index.choose_instance").to_string());

        let package_type = self.package_type.clone();
        let from_key = state_key(self.cluster_id, &package_type, true);
        let mut store = use_browser_state_store();

        rect()
            .margin(Gaps::new(4., 0., 0., 0.))
            .child(ClusterPickerButton {
                label: selected,
                selected: (self.cluster_id > 0).then_some(self.cluster_id),
                only: None,
                width: Size::px(240.),
                height: 28.,
                disabled: self.disabled,
                on_pick: (move |cluster: Cluster| {
                    let to_type = browsable_type(&package_type, &cluster);
                    let carried = (to_type == package_type)
                        .then(|| store.peek().get(&from_key).cloned())
                        .flatten();
                    if let Some(saved) = carried {
                        store.write().insert(
                            state_key(cluster.id, &to_type, true),
                            BrowserUiState { page: 0, ..saved },
                        );
                    }
                    let _ = RouterContext::get().replace(Route::Browser {
                        cluster_id: cluster.id,
                        package_type: to_type,
                        pick_cluster: true,
                        in_instance: false,
                    });
                })
                .into(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn categories_stay_with_the_provider_they_were_picked_on() {
        let mut parked = HashMap::new();
        let on_curseforge = swap_provider_categories(
            &mut parked,
            ProviderId::Modrinth,
            ProviderId::CurseForge,
            names(&["optimization"]),
        );
        assert!(
            on_curseforge.is_empty(),
            "Modrinth names never reach CurseForge"
        );
        let back = swap_provider_categories(
            &mut parked,
            ProviderId::CurseForge,
            ProviderId::Modrinth,
            names(&["Performance"]),
        );
        assert_eq!(back, names(&["optimization"]));
        assert_eq!(parked[&ProviderId::CurseForge], names(&["Performance"]));
    }

    #[test]
    fn reopening_browse_restores_the_other_providers_categories() {
        let mut saved = BrowserUiState {
            categories: names(&["optimization"]),
            parked_categories: HashMap::from([(ProviderId::CurseForge, names(&["Performance"]))]),
            ..BrowserUiState::default()
        };
        let store = HashMap::from([(state_key(3, "mods", false), saved.clone())]);
        saved.parked_categories.clear();

        let restored = store.get(&state_key(3, "mods", false)).unwrap();
        let mut parked = restored.parked_categories.clone();
        let categories = swap_provider_categories(
            &mut parked,
            restored.provider,
            ProviderId::CurseForge,
            restored.categories.clone(),
        );
        assert_eq!(categories, names(&["Performance"]));
        assert_eq!(parked[&ProviderId::Modrinth], names(&["optimization"]));
    }

    #[test]
    fn unknown_category_names_are_left_out_of_the_search() {
        let offered = names(&["adventure", "magic"]);
        assert_eq!(
            known_categories(&names(&["magic", "Performance"]), &offered),
            names(&["magic"])
        );
        // The provider's list has not arrived yet
        assert_eq!(known_categories(&names(&["magic"]), &[]), names(&["magic"]));
    }

    #[test]
    fn pickers_size_to_their_measured_label() {
        assert_eq!(picker_width(0., PICKER_CHROME_W, 108.), 108.);
        assert_eq!(picker_width(60.2, PICKER_CHROME_W, 108.), 107.);
    }

    #[test]
    fn any_search_change_on_the_same_page_resets_the_scroll() {
        let first = scroll_key("Modrinth|sodium", 0, false);
        assert_ne!(first, scroll_key("Modrinth|iris", 0, false));
        assert_ne!(first, scroll_key("CurseForge|sodium", 0, false));
        assert_ne!(first, scroll_key("Modrinth|sodium", 1, false));
        assert_eq!(first, scroll_key("Modrinth|sodium", 0, false));
    }
}
