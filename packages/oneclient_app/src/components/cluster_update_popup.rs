use std::collections::HashMap;

use freya::{prelude::*, router::RouterContext};
use oneclient_core::packages::{CachedPackageMeta, ProviderId};

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea, TabBar, TabItem};
use crate::hooks::{package_meta_batch, use_dispatch, use_notifications_snapshot, use_package_meta_batch};
use crate::notifications::{ClusterUpdateItem, ClusterUpdateSummary};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 420.;
const DIALOG_H: f32 = 400.;

type MetaMap = HashMap<(ProviderId, String), CachedPackageMeta>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpdateTab {
    Updates,
    Additions,
    Removals,
}

impl UpdateTab {
    const ALL: [UpdateTab; 3] = [UpdateTab::Updates, UpdateTab::Additions, UpdateTab::Removals];

    fn label(self) -> &'static str {
        match self {
            UpdateTab::Updates => "Updated",
            UpdateTab::Additions => "Added",
            UpdateTab::Removals => "Removed",
        }
    }

    fn icon(self) -> IconType {
        match self {
            UpdateTab::Updates => IconType::RefreshCw01,
            UpdateTab::Additions => IconType::Plus,
            UpdateTab::Removals => IconType::Trash01,
        }
    }

    fn accent(self) -> Color {
        match self {
            UpdateTab::Updates => colors::brand(),
            UpdateTab::Additions => colors::success(),
            UpdateTab::Removals => colors::danger(),
        }
    }

    fn items(self, summary: &ClusterUpdateSummary) -> &[ClusterUpdateItem] {
        match self {
            UpdateTab::Updates => &summary.updated,
            UpdateTab::Additions => &summary.added,
            UpdateTab::Removals => &summary.removed,
        }
    }
}

#[derive(PartialEq)]
pub struct ClusterUpdatePopup;

impl Component for ClusterUpdatePopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();
        let active = use_state(|| UpdateTab::Updates);

        let summary = snapshot.cluster_update.clone();

        // Resolve pretty display names through the package-meta cache. Hooks
        // must run unconditionally, so gather ids (empty when no summary) and
        // query every remote provider before the early return below.
        let all_items: Vec<&ClusterUpdateItem> = summary
            .iter()
            .flat_map(|s| s.updated.iter().chain(&s.added).chain(&s.removed))
            .collect();
        let mut meta = MetaMap::new();
        for provider in ProviderId::REMOTE_PROVIDERS.iter().copied() {
            let ids: Vec<String> = all_items
                .iter()
                .filter(|i| i.provider == provider)
                .filter_map(|i| i.project_id.clone())
                .collect();
            let query = use_package_meta_batch(provider, ids);
            for (pid, m) in package_meta_batch(&query) {
                meta.insert((provider, pid), m);
            }
        }

        let Some(summary) = summary else {
            return rect().into_element();
        };

        let close = dispatch.clone();

        OverlayPopup::new()
            .on_close(move |_| close.close_cluster_update())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(dialog(&summary, &meta, active, dispatch)),
            )
            .into_element()
    }
}

fn resolve_name(item: &ClusterUpdateItem, meta: &MetaMap) -> String {
    item.project_id
        .as_ref()
        .and_then(|pid| meta.get(&(item.provider, pid.clone())))
        .map(|m| m.name.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| item.fallback.clone())
}

fn dialog(
    summary: &ClusterUpdateSummary,
    meta: &MetaMap,
    active: State<UpdateTab>,
    dispatch: crate::BridgeDispatch,
) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::px(DIALOG_W))
        .height(Size::px(DIALOG_H))
        .max_width(Size::window_percent(95.))
        .overflow(Overflow::Clip)
        .corner_radius(CornerRadius::new_all(16.))
        .background(CARD_BG)
        .border(border_all_color(1., colors::component_border()))
        .shadow(Shadow::from((
            0.,
            18.,
            52.,
            0.,
            Color::from_argb(150, 0, 0, 0),
        )))
        .child(content(summary, meta, active, dispatch))
}

fn content(
    summary: &ClusterUpdateSummary,
    meta: &MetaMap,
    active: State<UpdateTab>,
    dispatch: crate::BridgeDispatch,
) -> impl IntoElement {
    let dismiss = dispatch.clone();
    let cluster_id = summary.cluster_id;
    let open = move |_| {
        dispatch.close_cluster_update();
        let _ = RouterContext::get().push(Route::ClusterOverview { cluster_id });
    };

    let total = summary.total();
    let active_tab = *active.read();

    // Tab row: every category always shown with a count pill, even at zero, so
    // the modal shape stays stable across clusters.
    let tabs = TabBar::new()
        .width(Size::fill())
        .height(Size::px(30.))
        .spacing(20.)
        .tabs(UpdateTab::ALL.into_iter().map(|tab| {
            let count = tab.items(summary).len();
            let mut set = active;
            TabItem::new(tab.label(), tab == active_tab)
                .count_text(count.to_string())
                .on_press(move |_| *set.write() = tab)
        }));

    let names: Vec<String> = active_tab
        .items(summary)
        .iter()
        .map(|item| resolve_name(item, meta))
        .collect();

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .padding(Gaps::new_all(22.))
        .spacing(14.)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(3.)
                .child(
                    label()
                        .text("Changes applied")
                        .font_size(17.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(format!(
                            "{total} change{} synced to this cluster.",
                            if total == 1 { "" } else { "s" }
                        ))
                        .font_size(12.5)
                        .max_lines(2)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(tabs)
        .child(change_list(active_tab, &names))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .main_align(Alignment::SpaceBetween)
                .spacing(8.)
                .child(
                    Button::new()
                        .ghost()
                        .on_press(move |_| dismiss.close_cluster_update())
                        .text("Dismiss"),
                )
                .child(
                    Button::new()
                        .primary()
                        .on_press(open)
                        .text("Open cluster")
                        .child(Icon::new(IconType::ArrowRight).size(15.)),
                ),
        )
}

fn change_list(tab: UpdateTab, names: &[String]) -> impl IntoElement {
    let accent = tab.accent();

    let mut scroll = ScrollArea::new()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .spacing(1.);

    if names.is_empty() {
        return scroll
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(80.))
                    .center()
                    .child(
                        label()
                            .text(format!("No {} in this update.", tab.label().to_lowercase()))
                            .font_size(12.5)
                            .color(colors::fg_secondary()),
                    ),
            )
            .into_element();
    }

    for name in names {
        scroll = scroll.child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(9.)
                .padding(Gaps::new_symmetric(5., 6.))
                .child(Icon::new(tab.icon()).size(13.).color(accent))
                .child(
                    label()
                        .text(name.clone())
                        .font_size(12.5)
                        .max_lines(1)
                        .width(Size::flex(1.0))
                        .color(colors::fg_primary()),
                ),
        );
    }

    scroll.into_element()
}
