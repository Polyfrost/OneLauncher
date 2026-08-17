use std::collections::HashMap;

use freya::{prelude::*, router::RouterContext};
use oneclient_content::packages::{CachedPackageMeta, ProviderId};

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
    Optional,
}

impl UpdateTab {
    const ALL: [UpdateTab; 4] = [
        UpdateTab::Updates,
        UpdateTab::Additions,
        UpdateTab::Removals,
        UpdateTab::Optional,
    ];

    fn label(self) -> &'static str {
        match self {
            UpdateTab::Updates => "Updated",
            UpdateTab::Additions => "Added",
            UpdateTab::Removals => "Removed",
            UpdateTab::Optional => "Optional",
        }
    }

    fn empty_text(self) -> &'static str {
        match self {
            UpdateTab::Updates => "Nothing was updated.",
            UpdateTab::Additions => "Nothing was added.",
            UpdateTab::Removals => "Nothing was removed.",
            UpdateTab::Optional => "Nothing optional was offered.",
        }
    }

    fn icon(self) -> IconType {
        match self {
            UpdateTab::Updates => IconType::RefreshCw01,
            UpdateTab::Additions => IconType::Plus,
            UpdateTab::Removals => IconType::Trash01,
            UpdateTab::Optional => IconType::Plus,
        }
    }

    fn accent(self) -> Color {
        match self {
            UpdateTab::Updates => colors::brand(),
            UpdateTab::Additions => colors::success(),
            UpdateTab::Removals => colors::danger(),
            UpdateTab::Optional => colors::brand(),
        }
    }

    fn items(self, summary: &ClusterUpdateSummary) -> &[ClusterUpdateItem] {
        match self {
            UpdateTab::Updates => &summary.updated,
            UpdateTab::Additions => &summary.added,
            UpdateTab::Removals => &summary.removed,
            UpdateTab::Optional => &summary.optional,
        }
    }
}

struct ClusterGroup {
    cluster_id: i64,
    cluster_name: String,
    names: Vec<String>,
}

#[derive(PartialEq)]
pub struct ClusterUpdatePopup;

impl Component for ClusterUpdatePopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();
        let active = use_state(|| UpdateTab::Updates);

        let summaries = snapshot.cluster_update.clone();

        // Hooks must run unconditionally so query every provider before the early return below
        let all_items: Vec<&ClusterUpdateItem> = summaries
            .iter()
            .flatten()
            .flat_map(|s| {
                s.updated
                    .iter()
                    .chain(&s.added)
                    .chain(&s.removed)
                    .chain(&s.optional)
            })
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

        let Some(summaries) = summaries.filter(|s| !s.is_empty()) else {
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
                    .child(dialog(&summaries, &meta, active, dispatch)),
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
    summaries: &[ClusterUpdateSummary],
    meta: &MetaMap,
    active: State<UpdateTab>,
    dispatch: crate::Actions,
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
        .child(content(summaries, meta, active, dispatch))
}

fn content(
    summaries: &[ClusterUpdateSummary],
    meta: &MetaMap,
    active: State<UpdateTab>,
    dispatch: crate::Actions,
) -> impl IntoElement {
    let dismiss = dispatch.clone();
    let total: usize = summaries.iter().map(|s| s.total()).sum();
    let active_tab = *active.read();

    // A batch sync can touch several clusters single-cluster keeps the footer shortcut
    let single = match summaries {
        [only] => Some((only.cluster_id, only.cluster_name.clone())),
        _ => None,
    };

    let subtitle = match &single {
        Some((_, name)) => format!(
            "{total} change{} synced to {name}.",
            if total == 1 { "" } else { "s" }
        ),
        None => format!(
            "{total} change{} across {} clusters.",
            if total == 1 { "" } else { "s" },
            summaries.len()
        ),
    };

    // Every category is shown even at zero so the modal shape stays stable
    let tabs = TabBar::new()
        .width(Size::fill())
        .height(Size::px(30.))
        .spacing(20.)
        .tabs(UpdateTab::ALL.into_iter().map(|tab| {
            let count: usize = summaries.iter().map(|s| tab.items(s).len()).sum();
            let mut set = active;
            TabItem::new(tab.label(), tab == active_tab)
                .count_text(count.to_string())
                .on_press(move |_| *set.write() = tab)
        }));

    // Clusters empty in the active category drop out avoiding empty section headers
    let groups: Vec<ClusterGroup> = summaries
        .iter()
        .filter_map(|summary| {
            let names: Vec<String> = active_tab
                .items(summary)
                .iter()
                .map(|item| resolve_name(item, meta))
                .collect();
            (!names.is_empty()).then(|| ClusterGroup {
                cluster_id: summary.cluster_id,
                cluster_name: summary.cluster_name.clone(),
                names,
            })
        })
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
                        .text(subtitle)
                        .font_size(12.5)
                        .max_lines(2)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(tabs)
        .child(change_list(
            active_tab,
            &groups,
            single.is_none(),
            &dispatch,
        ))
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
                .maybe_child(single.map(|(cluster_id, _)| {
                    let open = dispatch.clone();
                    Button::new()
                        .primary()
                        .on_press(move |_| open_cluster(&open, cluster_id))
                        .text("Open cluster")
                        .child(Icon::new(IconType::ArrowRight).size(15.))
                        .into_element()
                })),
        )
}

fn open_cluster(dispatch: &crate::Actions, cluster_id: i64) {
    dispatch.close_cluster_update();
    let _ = RouterContext::get().push(Route::ClusterOverview { cluster_id });
}

fn change_list(
    tab: UpdateTab,
    groups: &[ClusterGroup],
    show_headers: bool,
    dispatch: &crate::Actions,
) -> impl IntoElement {
    let accent = tab.accent();

    let mut scroll = ScrollArea::new()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .spacing(1.);

    if groups.is_empty() {
        return scroll
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(80.))
                    .center()
                    .child(
                        label()
                            .text(tab.empty_text())
                            .font_size(12.5)
                            .color(colors::fg_secondary()),
                    ),
            )
            .into_element();
    }

    for (index, group) in groups.iter().enumerate() {
        if show_headers {
            scroll = scroll.child(cluster_header(group, index == 0, dispatch.clone()));
        }

        for name in &group.names {
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
    }

    scroll.into_element()
}

/// Multi-cluster stand-in for the footer's "Open cluster" shortcut
fn cluster_header(
    group: &ClusterGroup,
    first: bool,
    dispatch: crate::Actions,
) -> impl IntoElement {
    let cluster_id = group.cluster_id;
    let count = group.names.len();

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(8.)
        .padding(Gaps::new_symmetric(6., 8.))
        .margin(Gaps::new(if first { 0. } else { 8. }, 0., 3., 0.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(move |_| open_cluster(&dispatch, cluster_id))
        .child(
            label()
                .text(group.cluster_name.clone())
                .font_size(12.)
                .font_weight(FontWeight::SEMI_BOLD)
                .max_lines(1)
                .width(Size::flex(1.0))
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(count.to_string())
                .font_size(11.)
                .color(colors::fg_secondary()),
        )
        .child(
            Icon::new(IconType::ArrowRight)
                .size(12.)
                .color(colors::fg_secondary()),
        )
}
