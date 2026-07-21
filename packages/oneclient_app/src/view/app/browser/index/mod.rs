use std::time::Duration;

use freya::animation::{
    AnimNum, Ease, Function, OnChange, OnCreation, use_animation_with_dependencies,
};
use freya::prelude::*;
use oneclient_core::packages::types::ProjectSummary;
use oneclient_core::packages::types::SearchSort;
use oneclient_core::packages::{ContentType, ProviderId};
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Icon, IconType, Pagination, ScrollArea, Segment, SegmentedControl, TextInput,
};
use crate::hooks::{
    BROWSE_PAGE_SIZE, BrowserUiState, category_list, content_type_for_slug, search_items,
    search_pending, search_total, use_browser_compat, use_browser_state_store, use_debounced,
    use_package_categories, use_package_search, use_view_state,
};
use crate::theme::colors;
use crate::view::app::cluster::load_cluster;

use super::{PackageBanner, Thumbnail};
use crate::utils::abbreviate_number;

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

        let cluster = load_cluster(cluster_id);
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
        let signature = format!(
            "{provider_id:?}|{}|{compat}|{}",
            debounced_query.read().trim().to_lowercase(),
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

        let packages = search_items(&search);
        let total = search_total(&search);
        let pending = search_pending(&search);
        let current_page = *page.read();
        let total_pages = total.div_ceil(BROWSE_PAGE_SIZE).max(1);

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
                        grid_row(rows[i].clone(), cluster_id, &pkg).into_element()
                    })
                }
                ViewLayout::List => sa.lazy(packages.len(), LIST_ROW_H, LIST_SPACING, move |i| {
                    list_row(packages[i].clone(), cluster_id, &pkg).into_element()
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
                            .maybe(total > 0, |el| el.child(Pagination::new(page, total_pages))),
                    ),
            )
    }
}

fn page_title(package_type: &str, cluster_name: Option<String>) -> impl IntoElement {
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
        .maybe_child(cluster_name.map(|name| {
            label()
                .text(format!("for {name}"))
                .font_size(14.)
                .color(colors::fg_secondary())
        }))
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
