use freya::prelude::*;
use oneclient_core::clusters::Cluster;

use super::page::{
    Filter, Sort, cluster_matches, filter_menu, loader_options, recent_line_heads, search_input,
    section_header, shown, toolbar,
};
use crate::components::{
    Button, Icon, IconType, LazySection, OVERLAY_BASE_LEVEL, OverlayPopup, ScrollArea, TabItem,
    VersionCard, centered_spinner,
};
use crate::hooks::{settled_or_loading, use_clusters};
use crate::i18n::t;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::{ReleaseLine, sort_clusters_for_home, split_clusters};

const DIALOG_W: f32 = 820.;
const DIALOG_PADDING_X: f32 = 22.;
const INLINE_SEARCH_MIN_W: f32 = 640.;
const MAX_COLUMNS: usize = 3;
const CARD_MIN_W: f32 = 200.;
const CARD_H: f32 = 150.;
const CARD_GAP: f32 = 10.;
const HEADER_H: f32 = 30.;

#[derive(PartialEq)]
pub struct ClusterPickerButton {
    pub label: String,
    pub selected: Option<i64>,
    pub only: Option<Vec<i64>>,
    pub width: Size,
    pub height: f32,
    pub on_pick: EventHandler<Cluster>,
}

impl Component for ClusterPickerButton {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);
        let mut hovered = use_state(|| false);
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let on_pick = self.on_pick.clone();

        rect()
            .width(self.width.clone())
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .height(Size::px(self.height))
                    .spacing(8.)
                    .padding(Gaps::new_symmetric(0., 10.))
                    .corner_radius(CornerRadius::new_all(6.))
                    .background(if *hovered.read() {
                        colors::ghost_overlay_hover()
                    } else {
                        colors::ghost_overlay()
                    })
                    .maybe(focus().is_focused(), |el| {
                        el.border(border_all_color(1., colors::brand()))
                    })
                    .cursor(CursorIcon::Pointer)
                    .a11y_id(a11y_id)
                    .a11y_focusable(true)
                    .a11y_role(AccessibilityRole::Button)
                    .on_pointer_enter(move |_| hovered.set(true))
                    .on_pointer_leave(move |_| hovered.set(false))
                    .on_press(move |_| open.set(true))
                    .child(
                        label()
                            .text(self.label.clone())
                            .font_size(12.)
                            .width(Size::flex(1.))
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        Icon::new(IconType::ChevronDown)
                            .size(13.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child(open.read().then(|| {
                ClusterPickerModal {
                    selected: self.selected,
                    only: self.only.clone(),
                    on_close: (move |()| open.set(false)).into(),
                    on_pick: (move |cluster: Cluster| {
                        open.set(false);
                        on_pick.call(cluster);
                    })
                    .into(),
                }
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct ClusterPickerModal {
    selected: Option<i64>,
    only: Option<Vec<i64>>,
    on_pick: EventHandler<Cluster>,
    on_close: EventHandler<()>,
}

impl Component for ClusterPickerModal {
    fn render(&self) -> impl IntoElement {
        let loaded = settled_or_loading(&use_clusters());
        let loading = loaded.is_none();
        let mut clusters = sort_clusters_for_home(loaded.unwrap_or_default());
        if let Some(only) = &self.only {
            clusters.retain(|c| only.contains(&c.id));
        }
        let mut category = use_state(|| Filter::All);
        let current = *category.read();
        let query = use_state(String::new);
        let loaders = use_state(Vec::<String>::new);
        let sort = use_state(|| Sort::RecentFirst);
        let mut content_w = use_state(|| DIALOG_W - 2. * DIALOG_PADDING_X);

        let needle = query.read().trim().to_lowercase();
        let picked_loaders = loaders.read().clone();
        let matches = |c: &Cluster| cluster_matches(c, &picked_loaders, &needle);
        let available_loaders = loader_options(&clusters);

        let tabs = Filter::ALL
            .into_iter()
            .filter(|f| *f == Filter::All || clusters.iter().any(|c| Filter::of(c) == *f))
            .map(|f| TabItem::new(f.label(), f == current).on_press(move |_| category.set(f)));

        let (groups, instances) = split_clusters(&clusters);
        let ordered: Vec<&Cluster> = recent_line_heads(&groups)
            .iter()
            .filter_map(ReleaseLine::for_cluster)
            .flat_map(|line| groups[&line].iter())
            .chain(&instances)
            .collect();
        let sort_by = *sort.read();

        let mut shown_clusters: Vec<Cluster> = Vec::new();
        let mut headers: Vec<(Filter, &'static str)> = Vec::new();
        let mut sections: Vec<LazySection> = Vec::new();
        for (section, caption) in [
            (
                Filter::OneClient,
                t("oneclient.clusters.picker.caption.oneclient"),
            ),
            (
                Filter::Custom,
                t("oneclient.clusters.picker.caption.custom"),
            ),
            (
                Filter::Modpacks,
                t("oneclient.clusters.picker.caption.modpacks"),
            ),
        ] {
            if !current.shows(section) {
                continue;
            }
            let in_section: Vec<&Cluster> = ordered
                .iter()
                .copied()
                .filter(|c| Filter::of(c) == section)
                .collect();
            let list = shown(&in_section, sort_by, matches);
            if list.is_empty() {
                continue;
            }
            headers.push((section, caption));
            sections.push(LazySection {
                header: true,
                count: list.len(),
            });
            shown_clusters.extend(list.into_iter().cloned());
        }

        let selected = self.selected;
        let on_pick = self.on_pick.clone();
        let body = if loading {
            centered_spinner(t("oneclient.clusters.picker.loading"))
        } else if sections.is_empty() {
            label()
                .text(t("oneclient.clusters.picker.empty"))
                .font_size(13.)
                .color(colors::fg_secondary())
                .into_element()
        } else {
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::fill())
                .lazy_sections(
                    sections,
                    CARD_H,
                    CARD_GAP,
                    CARD_MIN_W,
                    MAX_COLUMNS,
                    HEADER_H,
                    move |idx| {
                        let cluster = &shown_clusters[idx];
                        let on_pick = on_pick.clone();
                        let picked = cluster.clone();
                        VersionCard::for_instance(
                            cluster,
                            Some(cluster.id) == selected,
                            move |_| on_pick.call(picked.clone()),
                        )
                        .into_element()
                    },
                    move |s| {
                        let (section, caption) = headers[s];
                        rect()
                            .width(Size::fill())
                            .height(Size::fill())
                            .main_align(Alignment::End)
                            .child(section_header(section.label(), caption))
                            .into_element()
                    },
                )
                .into_element()
        };

        let close = self.on_close.clone();
        let outside = self.on_close.clone();
        let inline_search = *content_w.read() >= INLINE_SEARCH_MIN_W;

        let filters = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(filter_menu(&available_loaders, loaders, sort))
            .child(
                Button::new()
                    .ghost()
                    .icon()
                    .on_press(move |_| close.call(()))
                    .child(
                        Icon::new(IconType::XClose)
                            .size(16.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .into_element();

        OverlayPopup::new()
            .overlay_level(OVERLAY_BASE_LEVEL + 2)
            .on_close(self.on_close.clone())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        outside.call(());
                    })
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(DIALOG_W))
                            .height(Size::Inner)
                            .max_width(Size::window_percent(95.))
                            .max_height(Size::window_percent(85.))
                            .padding(Gaps::new(8., DIALOG_PADDING_X, 22., DIALOG_PADDING_X))
                            .spacing(14.)
                            .corner_radius(CornerRadius::new_all(16.))
                            .background(colors::page())
                            .border(border_all_color(1., colors::component_border()))
                            .on_press(|e: Event<PressEventData>| e.stop_propagation())
                            .on_sized(move |e: Event<SizedEventData>| {
                                let w = e.data().area.width() - 2. * DIALOG_PADDING_X;
                                if (w - *content_w.peek()).abs() > 0.5 {
                                    *content_w.write() = w;
                                }
                            })
                            .child(toolbar(
                                tabs,
                                query,
                                inline_search.then_some(filters.clone()),
                            ))
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
                                rect()
                                    .width(Size::fill())
                                    .height(Size::window_percent(60.))
                                    .child(body),
                            ),
                    ),
            )
    }
}
