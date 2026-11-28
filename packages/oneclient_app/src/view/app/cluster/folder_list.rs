use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;

use freya::prelude::*;
use oneclient_common::parse_mc_version;
use oneclient_core::clusters::Cluster;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Button, CARD_H, CONTENT_GRID_MIN_W, CardLayout, CardTone, GRID_GAP, Icon, IconType,
    LazySection, LocalImage, OverlayPopup, ROW_GAP, ROW_ICON, ROW_RADIUS, ScrollArea, Segment,
    SegmentedControl, Spinner, TextInput, TooltipPlacement, card_description, card_surface,
    content_card_height, icon_box, joined_line, kebab_button, meta_size, meta_text, on_secondary,
    row_line, row_surface, row_title, use_tooltip_anchor,
};
use crate::hooks::{
    PersistedView, loaded_image, settled_or_loading, use_cached_image, use_clusters,
    use_game_snapshot, use_settings_snapshot, use_view_state,
};
use crate::i18n::t;
use crate::layout::ClusterShell;
use crate::theme::{colors, rounded, text_size};
use crate::ui::{ImageFallbackExt, border_all_color, columns_for};

use super::package_manager::{GRID_MAX_COLS, notice_bar};

/// Height of every toolbar control on the instance content pages
pub(super) const CONTROL_H: f32 = 34.;
pub(super) const SELECTION_COUNT_W: f32 = 90.;
const SEARCH_MIN_W: f32 = 160.;
const SEARCH_MAX_W: f32 = 280.;
/// Between controls of one toolbar group
const CONTROL_GAP: f32 = 8.;
/// Between toolbar groups, and between the toolbar and the list below it
const GROUP_GAP: f32 = 12.;
const CONTENT_TOOLBAR_BREAKPOINT: f32 = SEARCH_MIN_W + GROUP_GAP + 298. + GROUP_GAP + 294.;
const STACKED_TOOLBAR_H: f32 = CONTROL_H * 2. + CONTROL_GAP;
const SCOPE_CHIP_H: f32 = 28.;
const IMAGE_EDGE: u32 = 128;
const FIRST_DATAPACK_MAJOR: u32 = 13;
/// One list or grid choice for every instance content page, so changing page never changes view
pub(super) const CONTENT_VIEW_KEY: &str = "cluster.content";

pub(crate) fn supports_datapacks(mc_version: &str) -> bool {
    parse_mc_version(mc_version).is_none_or(|p| p.major >= FIRST_DATAPACK_MAJOR)
}

pub(super) fn use_game_folder_in_use(cluster: Option<&Cluster>) -> bool {
    let game = use_game_snapshot();
    let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();

    cluster.is_some_and(|cluster| {
        game.is_active(cluster.id)
            || clusters
                .iter()
                .any(|c| game.is_active(c.id) && c.shares_dir_with(cluster))
    })
}

pub(super) fn notify_in_use(dispatch: &crate::Actions, body: &'static str) {
    dispatch
        .notify(t("oneclient.cluster.folder_list.in_use.title"))
        .body(body)
        .error()
        .send();
}

pub(super) fn matches_search(needle: &str, fields: &[&str]) -> bool {
    needle.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(needle))
}

/// The layout the content pages start in before the user has ever picked one
fn initial_content_layout(stored: bool) -> Option<ViewLayout> {
    (!stored).then_some(ViewLayout::List)
}

/// The list or grid choice every instance content page shares, defaulting to List
pub(super) fn use_content_view() -> PersistedView {
    let settings = use_settings_snapshot().settings;
    let initial =
        use_hook(|| initial_content_layout(settings.view_states.contains_key(CONTENT_VIEW_KEY)));
    let view = use_view_state(CONTENT_VIEW_KEY);
    let mut layout = view.layout;
    use_hook(move || {
        if let Some(initial) = initial {
            spawn(async move { layout.set(initial) });
        }
    });
    view
}

/// How the toolbar lays out at a measured width
#[derive(Clone, Copy, Debug, PartialEq)]
struct ToolbarFit {
    stacked: bool,
    /// The search width after reserving room for the controls on its line
    search_w: f32,
}

/// The width every control needs on one line, with the search at its minimum
fn toolbar_intrinsic_width(filters_w: f32, actions_w: f32) -> f32 {
    SEARCH_MIN_W + toolbar_group_space(filters_w) + toolbar_group_space(actions_w)
}

fn toolbar_group_space(width: f32) -> f32 {
    if width > 0. { width + GROUP_GAP } else { 0. }
}

fn toolbar_fit(available: f32, filters_w: f32, actions_w: f32) -> ToolbarFit {
    let intrinsic = toolbar_intrinsic_width(filters_w, actions_w);
    let stacked = available < intrinsic;
    let search_w = if stacked {
        (available - toolbar_group_space(filters_w)).clamp(0., SEARCH_MAX_W)
    } else {
        (available - (intrinsic - SEARCH_MIN_W)).clamp(SEARCH_MIN_W, SEARCH_MAX_W)
    };
    ToolbarFit { stacked, search_w }
}

fn toolbar_height(available: f32, two_lines: bool) -> f32 {
    // Keep the list aligned with pages with a fuller toolbar at narrow widths.
    if two_lines || available < CONTENT_TOOLBAR_BREAKPOINT {
        STACKED_TOOLBAR_H
    } else {
        CONTROL_H
    }
}

/// The one toolbar of every instance content page
///
/// Search first, then the page's filter and view controls, a spacer, then the page's actions.
#[derive(PartialEq)]
struct ContentToolbar {
    search: Option<State<String>>,
    filters: Vec<Element>,
    actions: Vec<Element>,
}

fn track_width(mut state: State<f32>) -> impl FnMut(Event<SizedEventData>) {
    move |event: Event<SizedEventData>| {
        let next = event.data().area.width();
        if (*state.peek() - next).abs() > 0.5 {
            state.set(next);
        }
    }
}

/// The natural width of a control group, from what layout reports
///
/// A stacked line gives each group the whole pane, so it reports the natural width. On one line
/// a group that does not fit gets squeezed, so a smaller report there is not trusted.
pub(super) fn natural_width(stored: f32, measured: f32, stacked: bool) -> f32 {
    if stacked {
        measured
    } else {
        stored.max(measured)
    }
}

fn track_group_width(mut state: State<f32>, stacked: bool) -> impl FnMut(Event<SizedEventData>) {
    move |event: Event<SizedEventData>| {
        let stored = *state.peek();
        let next = natural_width(stored, event.data().area.width(), stacked);
        if (stored - next).abs() > 0.5 {
            state.set(next);
        }
    }
}

fn gap(width: f32) -> Element {
    rect().width(Size::px(width)).into_element()
}

impl Component for ContentToolbar {
    fn render(&self) -> impl IntoElement {
        let width = use_state(|| 0f32);
        let empty_search = use_state(String::new);
        let mut filters_w = use_state(|| 0f32);
        let mut actions_w = use_state(|| 0f32);
        // New controls (bulk selection, a different tab's action) are measured afresh
        let signature = (self.filters.len(), self.actions.len());
        use_side_effect_with_deps(&signature, move |_| {
            filters_w.set_if_modified(0.);
            actions_w.set_if_modified(0.);
        });
        // Until each group has been measured at its natural width it lays out stacked
        let unmeasured = (!self.filters.is_empty() && *filters_w.read() == 0.)
            || (!self.actions.is_empty() && *actions_w.read() == 0.);
        let available = *width.read();
        let mut fit = toolbar_fit(available, *filters_w.read(), *actions_w.read());
        // Until the groups have reported their natural widths, give them their own lines.
        fit.stacked |= unmeasured;

        let search = TextInput::new(self.search.unwrap_or(empty_search))
            .enabled(self.search.is_some())
            .placeholder(t("oneclient.cluster.package_manager.search"))
            .width(Size::px(fit.search_w))
            .height(Size::px(CONTROL_H))
            .leading(
                Icon::new(IconType::SearchMd)
                    .size(14.)
                    .color(colors::fg_secondary())
                    .into_element(),
            );
        let filters = (!self.filters.is_empty()).then(|| {
            rect()
                .horizontal()
                .height(Size::px(CONTROL_H))
                .cross_align(Alignment::Center)
                .spacing(CONTROL_GAP)
                .on_sized(track_group_width(filters_w, fit.stacked))
                .children(self.filters.clone())
                .into_element()
        });
        let actions = (!self.actions.is_empty()).then(|| {
            rect()
                .horizontal()
                .height(Size::px(CONTROL_H))
                .cross_align(Alignment::Center)
                .spacing(CONTROL_GAP)
                .on_sized(track_group_width(actions_w, fit.stacked))
                .children(self.actions.clone())
                .into_element()
        });
        let line = || {
            rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::px(CONTROL_H))
                .cross_align(Alignment::Center)
                .content(Content::Flex)
        };
        let lead_gap = filters.is_some().then(|| gap(GROUP_GAP));

        let inner = if fit.stacked {
            let find = {
                line()
                    .child(search)
                    .maybe_child(lead_gap)
                    .maybe_child(filters)
                    .into_element()
            };
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(CONTROL_GAP)
                .child(find)
                .maybe_child(actions.map(|actions| {
                    line()
                        .main_align(Alignment::End)
                        .child(actions)
                        .into_element()
                }))
        } else {
            let trail_gap = actions.is_some().then(|| gap(GROUP_GAP));
            line()
                .child(search)
                .maybe_child(lead_gap)
                .maybe_child(filters)
                .child(rect().width(Size::flex(1.0)))
                .maybe_child(trail_gap)
                .maybe_child(actions)
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::px(toolbar_height(
                available,
                fit.stacked && !self.actions.is_empty(),
            )))
            .on_sized(track_width(width))
            .child(inner)
    }
}

pub(super) fn content_toolbar(
    search: Option<State<String>>,
    filters: Vec<Element>,
    actions: Vec<Element>,
) -> Element {
    ContentToolbar {
        search,
        filters,
        actions,
    }
    .into_element()
}

/// Says whether the page's content is shared with other instances; its tooltip has the full sentence
#[derive(PartialEq)]
struct ScopeChip {
    shared: bool,
    tip: String,
}

impl Component for ScopeChip {
    fn render(&self) -> impl IntoElement {
        let tooltip = use_tooltip_anchor(Some(self.tip.clone().into()), TooltipPlacement::Bottom);
        let (icon, text) = if self.shared {
            (
                IconType::Users01,
                t("oneclient.cluster.content.scope.shared"),
            )
        } else {
            (
                IconType::Lock01,
                t("oneclient.cluster.content.scope.instance"),
            )
        };

        tooltip.attach(
            rect()
                .horizontal()
                .height(Size::px(SCOPE_CHIP_H))
                .padding(Gaps::new_symmetric(0., 10.))
                .cross_align(Alignment::Center)
                .spacing(6.)
                .corner_radius(rounded(999.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::component_border()))
                .child(Icon::new(icon).size(14.).color(colors::fg_secondary()))
                .child(
                    label()
                        .text(text)
                        .font_size(text_size(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                ),
        )
    }
}

pub(super) fn scope_chip(shared: bool, tip: String) -> Element {
    ScopeChip { shared, tip }.into_element()
}

/// The page's "Open folder" action: an icon button whose tooltip names exactly which folder it opens
///
/// With no folder the button stays put but is disabled, so a missing target reads as unavailable
/// rather than as a missing feature.
pub(super) fn open_folder_action(folder: Option<PathBuf>, tooltip: String) -> Element {
    Button::new()
        .secondary()
        .icon()
        .width(Size::px(CONTROL_H))
        .height(Size::px(CONTROL_H))
        .enabled(folder.is_some())
        .tooltip(tooltip)
        .on_press(move |_| open_folder(folder.as_ref()))
        .child(
            Icon::new(IconType::Folder)
                .size(16.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn open_folder(folder: Option<&PathBuf>) {
    if let Some(folder) = folder {
        std::fs::create_dir_all(folder).ok();
        crate::platform::open_path(&folder.to_string_lossy());
    }
}

/// "Open folder" with its label, for the empty states
pub(super) fn open_folder_button(folder: Option<PathBuf>) -> Element {
    toolbar_secondary(IconType::Folder, t("oneclient.cluster.folder.open"))
        .enabled(folder.is_some())
        .on_press(move |_| open_folder(folder.as_ref()))
        .into_element()
}

/// A labelled folder button with its own wording, for pages outside the content toolbar (Logs)
pub(super) fn folder_button(folder: PathBuf, name: &'static str) -> Element {
    toolbar_secondary(IconType::Folder, name)
        .on_press(move |_| open_folder(Some(&folder)))
        .into_element()
}

pub(super) fn layout_toggle(layout: State<ViewLayout>) -> Element {
    SegmentedControl::new(layout)
        .static_indicator()
        .height(CONTROL_H)
        .icon_size(15.)
        .equal_width(CONTROL_H)
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

pub(super) fn toolbar_action(icon: IconType, text: &str) -> Button {
    Button::new()
        .primary()
        .height(Size::px(CONTROL_H))
        .font_size(text_size(12.))
        .child(Icon::new(icon).size(15.))
        .text(text.to_string())
}

/// The widest selection toolbar with its labels, in the widest theme font
const LABELLED_SELECTION_W: f32 = 572.;

/// Whether the pane is too narrow for labels on the selection actions. It depends on the window
/// alone, so the actions never change form while a selection does.
pub(super) fn compact_selection() -> bool {
    ClusterShell::workspace_pane_size().0 < LABELLED_SELECTION_W
}

pub(super) fn selection_icon(icon: IconType, tooltip: impl Into<Box<str>>) -> Button {
    Button::new()
        .secondary()
        .icon()
        .width(Size::px(CONTROL_H))
        .height(Size::px(CONTROL_H))
        .tooltip(tooltip)
        .child(Icon::new(icon).size(15.))
}

/// `width` holds the action's place while its label changes
pub(super) fn selection_action(icon: IconType, text: String, width: Option<f32>) -> Button {
    let button = Button::new().secondary().height(Size::px(CONTROL_H));
    if compact_selection() {
        return selection_icon(icon, text);
    }
    let button = match width {
        Some(width) => button.width(Size::px(width)),
        None => button,
    };
    button
        .font_size(text_size(12.))
        .child(Icon::new(icon).size(15.))
        .text(text)
}

pub(super) fn toolbar_secondary(icon: IconType, text: &str) -> Button {
    Button::new()
        .secondary()
        .height(Size::px(CONTROL_H))
        .font_size(text_size(12.))
        .child(Icon::new(icon).size(15.))
        .text(text.to_string())
}

/// The surface the empty, loading and no-results states share: it takes the list's place and puts
/// its message about 30% of the way down
fn state_panel(body: Rect) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .padding(Gaps::new_all(32.))
        .corner_radius(rounded(ROW_RADIUS))
        .background(colors::card())
        .border(border_all_color(1., colors::component_border()).alignment(BorderAlignment::Inner))
        .content(Content::Flex)
        .child(rect().width(Size::fill()).height(Size::flex(3.)))
        .child(
            body.vertical()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(8.),
        )
        .child(rect().width(Size::fill()).height(Size::flex(7.)))
        .into_element()
}

/// The empty and no-results state of every instance content page
pub(super) fn content_empty(
    icon: IconType,
    title: impl Into<String>,
    hint: Option<String>,
    actions: Vec<Element>,
) -> Element {
    state_panel(
        rect()
            .child(Icon::new(icon).size(28.).color(colors::fg_secondary()))
            .child(
                label()
                    .text(title.into())
                    .font_size(text_size(14.))
                    .font_weight(FontWeight::SEMI_BOLD)
                    .text_align(TextAlign::Center)
                    .width(Size::fill())
                    .color(colors::fg_primary()),
            )
            .maybe_child(hint.map(|hint| {
                label()
                    .text(hint)
                    .font_size(text_size(12.))
                    .text_align(TextAlign::Center)
                    .width(Size::fill())
                    .color(colors::fg_secondary())
                    .into_element()
            }))
            .maybe_child((!actions.is_empty()).then(|| {
                rect()
                    .horizontal()
                    .margin(Gaps::new(8., 0., 0., 0.))
                    .spacing(CONTROL_GAP)
                    .children(actions)
                    .into_element()
            })),
    )
}

/// The loading state of every instance content page
pub(super) fn content_loading() -> Element {
    state_panel(
        rect().child(Spinner::new(28.)).child(
            label()
                .text(t("oneclient.cluster.content.loading"))
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        ),
    )
}

/// Everything below a content page's toolbar starts here, 12 px under it
pub(super) fn content_area(notices: Vec<String>) -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .padding(Gaps::new(GROUP_GAP, 0., 0., 0.))
        .spacing(GROUP_GAP)
        .content(Content::Flex)
        .children(notices.into_iter().map(notice_bar))
}

/// The scrolling list every content page uses; the scrollbar overlays so rows end where the toolbar does
pub(super) fn content_scroll() -> ScrollArea {
    ScrollArea::new()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .auto_hide_scrollbar()
}

/// A grid row and its position in the scroll content. A section header has no items.
struct ContentGridRow {
    section: usize,
    items: Range<usize>,
    top: f32,
    height: f32,
}

fn grid_rows(
    sections: &[LazySection],
    cols: usize,
    descriptions: &[bool],
    header_height: f32,
) -> Vec<ContentGridRow> {
    let mut rows = Vec::new();
    let mut top = 0.;
    let mut start = 0;
    for (section_index, section) in sections.iter().enumerate() {
        if section.header {
            rows.push(ContentGridRow {
                section: section_index,
                items: start..start,
                top,
                height: header_height,
            });
            top += header_height + GRID_GAP;
        }
        let end = start + section.count;
        while start < end {
            let next = (start + cols).min(end);
            let height = content_card_height(descriptions[start..next].iter().any(|&has| has));
            rows.push(ContentGridRow {
                section: section_index,
                items: start..next,
                top,
                height,
            });
            top += height + GRID_GAP;
            start = next;
        }
    }
    rows
}

fn grid_visible_rows(
    rows: &[ContentGridRow],
    scroll_top: f32,
    viewport_height: f32,
) -> Range<usize> {
    let overscan = 3. * (content_card_height(true) + GRID_GAP);
    let view_top = scroll_top - overscan;
    let view_bottom = scroll_top + viewport_height + overscan;
    let first = rows.partition_point(|row| row.top + row.height + GRID_GAP <= view_top);
    let end = rows.partition_point(|row| row.top < view_bottom);
    first..end
}

/// Virtualize variable-height grid rows while retaining each section's own column alignment.
pub(super) fn content_grid(
    sections: Vec<LazySection>,
    descriptions: Vec<bool>,
    header_height: f32,
    render: impl Fn(usize) -> Element + 'static,
    header: impl Fn(usize) -> Element + 'static,
) -> ScrollArea {
    let geometry = RefCell::new((0, Vec::<ContentGridRow>::new()));
    let mounted = RefCell::new(HashMap::<usize, Element>::new());
    let rendered = RefCell::new(None::<((usize, usize, usize), Element)>);
    content_scroll().content(move |ctx| {
        let cols = columns_for(ctx.viewport_w, CONTENT_GRID_MIN_W, GRID_MAX_COLS, GRID_GAP);
        let mut geometry = geometry.borrow_mut();
        if geometry.0 != cols {
            mounted.borrow_mut().clear();
            *geometry = (
                cols,
                grid_rows(&sections, cols, &descriptions, header_height),
            );
        }
        let rows = &geometry.1;
        let range = grid_visible_rows(rows, -ctx.corrected_y, ctx.viewport_h);
        let (first, end) = (range.start, range.end);
        let key = (cols, first, end);
        let mut rendered = rendered.borrow_mut();
        if let Some((previous, element)) = rendered.as_ref()
            && *previous == key
        {
            return element.clone();
        }
        let total = rows.last().map_or(0., |r| r.top + r.height + GRID_GAP);
        let top = rows.get(first).map_or(total, |r| r.top);
        let bottom = rows.get(end).map_or(total, |r| r.top);
        let spacer = |height| rect().width(Size::fill()).height(Size::px(height));
        let mut list = rect().vertical().width(Size::fill()).child(spacer(top));
        // Keep the same handlers and component props for rows shared by both ranges.
        let mut mounted = mounted.borrow_mut();
        mounted.retain(|index, _| range.contains(index));
        for (index, row) in range.clone().zip(&rows[range]) {
            let element = mounted.entry(index).or_insert_with(|| {
                let inner = if row.items.is_empty() {
                    header(row.section)
                } else {
                    let mut cards = rect()
                        .horizontal()
                        .width(Size::fill())
                        .height(Size::px(row.height))
                        .spacing(GRID_GAP)
                        .content(Content::Flex);
                    for col in 0..cols {
                        let cell = rect().width(Size::flex(1.)).height(Size::fill());
                        cards =
                            cards.child(cell.maybe_child(row.items.clone().nth(col).map(&render)));
                    }
                    cards.into_element()
                };
                spacer(row.height + GRID_GAP)
                    .key(index + 1)
                    .child(inner)
                    .into_element()
            });
            list = list.child(element.clone());
        }
        list = list.child(spacer(total - bottom));
        let element = list.into_element();
        *rendered = Some((key, element.clone()));
        element
    })
}

/// The list area below the toolbar: transient notices, then the rows, or the page's state panel
pub(super) fn content_box(
    count: usize,
    layout: CardLayout,
    has_description: impl Fn(usize) -> bool,
    row: impl Fn(usize) -> Element + 'static,
    empty: Option<Element>,
    notices: Vec<String>,
) -> Element {
    let scroll = (count > 0).then(|| {
        match layout {
            CardLayout::List => content_scroll().lazy(count, CARD_H, ROW_GAP, row),
            CardLayout::Grid => content_grid(
                vec![LazySection {
                    header: false,
                    count,
                }],
                (0..count).map(has_description).collect(),
                0.,
                row,
                |_| rect().into_element(),
            ),
        }
        .into_element()
    });

    let empty = if count == 0 { empty } else { None };

    content_area(notices)
        .maybe_child(scroll)
        .maybe_child(empty.map(|empty| {
            rect()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .child(empty)
                .into_element()
        }))
        .into_element()
}

#[derive(Clone, PartialEq)]
pub(super) enum CardIcon {
    Image(PathBuf),
    Cached(String, IconType),
    Symbol(IconType),
}

#[derive(PartialEq)]
struct CachedIcon {
    url: String,
    fallback: IconType,
    size: f32,
}

impl Component for CachedIcon {
    fn render(&self) -> impl IntoElement {
        let query = use_cached_image(Some(self.url.clone()), IMAGE_EDGE);

        match loaded_image(Some(&self.url), &query) {
            Some(loaded) => ImageViewer::new(loaded)
                .width(Size::px(self.size))
                .height(Size::px(self.size))
                .aspect_ratio(AspectRatio::Min)
                .corner_radius(rounded(8.))
                .fallback(icon_box(self.fallback, self.size))
                .into_element(),
            None => icon_box(self.fallback, self.size),
        }
    }
}

/// A row or card on a content page that has no on/off state (worlds, data packs)
///
/// It uses the same surfaces and metrics as the package rows on Mods, Shaders and Textures.
#[derive(PartialEq)]
pub(super) struct FolderCard {
    pub icon: CardIcon,
    pub title: String,
    /// Kind or status, shown first on the secondary line
    pub meta: String,
    pub description: Option<String>,
    pub size: u64,
    pub layout: CardLayout,
    pub on_press: Option<EventHandler<()>>,
    pub on_context: EventHandler<(f32, f32)>,
}

impl Component for FolderCard {
    fn render(&self) -> impl IntoElement {
        let hovered = use_state(|| false);
        let icon = card_icon(&self.icon, ROW_ICON);
        let on_context = Some(self.on_context.clone());
        let on_press = self.on_press.clone();
        let pressable = on_press.is_some();
        let press = move |_: Event<PressEventData>| {
            if let Some(handler) = &on_press {
                handler.call(());
            }
        };
        let description = self
            .description
            .clone()
            .filter(|text| !text.trim().is_empty());

        match self.layout {
            CardLayout::List => row_surface(hovered)
                .maybe(pressable, |el| el.cursor(CursorIcon::Pointer))
                .on_press(press)
                .on_secondary_down(on_secondary(on_context))
                .child(icon)
                .child(
                    rect()
                        .vertical()
                        .width(Size::flex(1.0))
                        .spacing(4.)
                        .child(row_title(self.title.clone(), colors::fg_primary()))
                        .child(row_line(
                            joined_line([self.meta.as_str(), description.as_deref().unwrap_or("")]),
                            colors::fg_secondary(),
                        )),
                )
                .child(meta_size(self.size))
                .child(kebab_button(self.on_context.clone()))
                .into_element(),
            CardLayout::Grid => {
                let size = (self.size > 0).then(|| crate::utils::format_size(self.size));
                let meta = joined_line([self.meta.as_str(), size.as_deref().unwrap_or("")]);
                let header = rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .content(Content::Flex)
                    .child(icon)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(2.)
                            .child(row_title(self.title.clone(), colors::fg_strong()))
                            .child(meta_text(meta, colors::fg_secondary())),
                    )
                    .child(kebab_button(self.on_context.clone()));

                card_surface(hovered, CardTone::Neutral)
                    .maybe(pressable, |el| el.cursor(CursorIcon::Pointer))
                    .on_press(press)
                    .on_secondary_down(on_secondary(on_context))
                    .child(header)
                    .maybe_child(description.map(|text| {
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .overflow(Overflow::Clip)
                            .child(card_description(text, colors::fg_secondary()))
                            .into_element()
                    }))
                    .into_element()
            }
        }
    }
}

pub(super) fn card_icon(icon: &CardIcon, size: f32) -> Element {
    match icon {
        CardIcon::Image(path) => rect()
            .width(Size::px(size))
            .height(Size::px(size))
            .corner_radius(rounded(8.))
            .overflow(Overflow::Clip)
            .background(colors::component_bg())
            .child(LocalImage::new(path.clone(), IMAGE_EDGE, true).skeleton(true))
            .into_element(),
        CardIcon::Cached(url, fallback) => CachedIcon {
            url: url.clone(),
            fallback: *fallback,
            size,
        }
        .into_element(),
        CardIcon::Symbol(symbol) => icon_box(*symbol, size),
    }
}

pub(crate) fn confirm_dialog(
    title: String,
    body: String,
    on_cancel: impl FnMut() + Clone + 'static,
    mut on_confirm: impl FnMut() + 'static,
) -> Element {
    let mut cancel = on_cancel.clone();

    dialog(
        title,
        body,
        None,
        on_cancel,
        [
            Button::new()
                .secondary()
                .on_press(move |_| cancel())
                .text(t("oneclient.common.cancel"))
                .into_element(),
            Button::new()
                .danger()
                .on_press(move |_| on_confirm())
                .child(Icon::new(IconType::Trash01).size(14.))
                .text(t("oneclient.common.delete"))
                .into_element(),
        ],
    )
}

/// A small dialog. Escape and a press outside it both call `on_close`; a caller with something
/// in flight keeps it open by ignoring that call while it is pending.
pub(crate) fn dialog(
    title: String,
    body: String,
    control: Option<Element>,
    on_close: impl FnMut() + 'static,
    buttons: [Element; 2],
) -> Element {
    let on_close = std::rc::Rc::new(std::cell::RefCell::new(on_close));
    let outside = on_close.clone();
    OverlayPopup::new()
        .on_close(move |_| (on_close.borrow_mut())())
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .on_press(move |_| (outside.borrow_mut())())
                .child(
                    crate::theme::glass::popup_panel(rect(), ROW_RADIUS, colors::card())
                        .vertical()
                        .width(Size::px(400.))
                        .max_width(Size::window_percent(90.))
                        .spacing(14.)
                        .padding(Gaps::new_all(20.))
                        .border(border_all_color(1., colors::component_border()))
                        .on_all_press(|event: Event<PressEventData>| event.stop_propagation())
                        .child(
                            label()
                                .text(title)
                                .font_size(text_size(16.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(body)
                                .font_size(text_size(12.))
                                .color(colors::fg_secondary()),
                        )
                        .maybe_child(control)
                        .child(
                            rect()
                                .horizontal()
                                .width(Size::fill())
                                .main_align(Alignment::End)
                                .spacing(8.)
                                .children(buttons),
                        ),
                ),
        )
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_stacks_only_below_its_intrinsic_width() {
        // Mods: category, filter and layout toggle, then copy, folder and add
        let (filters, actions) = (306., 300.);
        let intrinsic = toolbar_intrinsic_width(filters, actions);
        assert_eq!(
            intrinsic,
            SEARCH_MIN_W + GROUP_GAP + filters + GROUP_GAP + actions
        );

        assert!(toolbar_fit(intrinsic - 1., filters, actions).stacked);
        let fits = toolbar_fit(intrinsic, filters, actions);
        assert!(!fits.stacked);
        assert_eq!(fits.search_w, SEARCH_MIN_W);
    }

    #[test]
    fn short_toolbars_stay_on_one_line_without_moving_the_list() {
        let available = 700.;
        assert!(!toolbar_fit(available, 68., 34.).stacked);
        assert!(toolbar_fit(available, 298., 294.).stacked);
        assert_eq!(toolbar_height(available, false), STACKED_TOOLBAR_H);
        assert_eq!(toolbar_height(available, true), STACKED_TOOLBAR_H);
        assert_eq!(toolbar_height(800., false), CONTROL_H);
        assert_eq!(toolbar_height(800., true), STACKED_TOOLBAR_H);
    }

    #[test]
    fn stacked_search_reserves_filter_space_and_caps_its_width() {
        let at = |available| toolbar_fit(available, 298., 294.).search_w;
        assert_eq!(at(516.), 206.);
        assert_eq!(at(748.), 280.);
        assert_eq!(at(776.), 160.);
        assert_eq!(at(896.), 280.);
    }

    #[test]
    fn toolbar_counts_only_present_groups() {
        assert_eq!(toolbar_intrinsic_width(0., 0.), SEARCH_MIN_W);
        assert_eq!(
            toolbar_intrinsic_width(0., 34.),
            SEARCH_MIN_W + GROUP_GAP + 34.
        );
        assert_eq!(toolbar_fit(200., 0., 120.).search_w, 200.);
    }

    #[test]
    fn squeezed_groups_keep_their_natural_width() {
        assert_eq!(natural_width(290., 200., false), 290.);
        assert_eq!(natural_width(290., 320., false), 320.);
        assert_eq!(natural_width(290., 200., true), 200.);
    }

    #[test]
    fn selection_actions_fit_the_narrow_workspace_without_moving() {
        let package_width = SELECTION_COUNT_W + 5. * CONTROL_H + 5. * CONTROL_GAP;
        let screenshot_width = SELECTION_COUNT_W + 4. * CONTROL_H + 4. * CONTROL_GAP;
        assert_eq!(package_width, 300.);
        assert_eq!(screenshot_width, 258.);
        assert!(package_width <= 524.);
        assert!(screenshot_width <= 524.);
    }

    #[test]
    fn content_pages_default_to_the_list_layout() {
        assert_eq!(initial_content_layout(false), Some(ViewLayout::List));
        assert_eq!(initial_content_layout(true), None);
    }

    #[tokio::test]
    async fn scrolling_the_grid_builds_only_the_new_rows() {
        use std::{cell::Cell, rc::Rc, time::Duration};

        fn app() -> impl IntoElement {
            let rendered = use_consume::<Rc<Cell<(usize, usize)>>>();
            content_grid(
                vec![LazySection {
                    header: false,
                    count: 1_000,
                }],
                vec![true; 1_000],
                0.,
                move |index| {
                    let (total, first_card) = rendered.get();
                    rendered.set((total + 1, first_card + usize::from(index == 0)));
                    rect().into_element()
                },
                |_| rect().into_element(),
            )
            .height(Size::px(200.))
        }

        let (mut runner, rendered) = freya_testing::TestingRunner::new(
            app,
            Size2D::new(600., 300.),
            |runner| runner.provide_root_context(|| Rc::new(Cell::new((0usize, 0usize)))),
            1.,
        );
        runner.poll_n(Duration::ZERO, 5);

        let (initial, first_card) = rendered.get();
        assert!(initial < 40, "initial layout built {initial} cards");
        runner.scroll((80., 100.), (0., -124.));
        runner.poll_n(Duration::ZERO, 5);
        assert_eq!(rendered.get(), (initial + 2, first_card));

        // Scrolling far back after the first row leaves the mounted range must build it again.
        runner.scroll((80., 100.), (0., -1240.));
        runner.poll_n(Duration::ZERO, 5);
        assert_eq!(rendered.get().1, first_card);
        runner.scroll((80., 100.), (0., 2000.));
        runner.poll_n(Duration::ZERO, 5);
        assert_eq!(rendered.get().1, first_card + 1);
    }

    #[test]
    fn grid_range_excludes_slots_outside_the_viewport_and_overscan() {
        let sections = [LazySection {
            header: true,
            count: 1_000,
        }];
        let descriptions = (0..1_000).map(|index| index % 2 == 0).collect::<Vec<_>>();
        let rows = grid_rows(&sections, 1, &descriptions, 36.);
        let overscan = 3. * (content_card_height(true) + GRID_GAP);
        let top = rows[470].top + overscan;
        let height = rows[480].top - top - overscan;

        assert_eq!(grid_visible_rows(&rows, top, height), 470..480);
        assert_eq!(grid_visible_rows(&rows, top + 1., height), 470..481);
        assert_eq!(grid_visible_rows(&rows, 0., 0.).start, 0);
        assert_eq!(
            grid_visible_rows(&rows, rows[1_000].top, 600.).end,
            rows.len()
        );
        assert_eq!(grid_visible_rows(&[], 0., 300.), 0..0);
    }

    #[test]
    fn grid_rows_grow_for_their_own_descriptions_and_keep_section_breaks() {
        let sections = [
            LazySection {
                header: false,
                count: 3,
            },
            LazySection {
                header: true,
                count: 3,
            },
        ];
        let descriptions = [false, true, false, false, false, true];
        let at_two = grid_rows(&sections, 2, &descriptions, 36.);
        assert_eq!(
            at_two.iter().map(|row| row.height).collect::<Vec<_>>(),
            [112., 88., 36., 88., 112.]
        );
        assert_eq!(at_two[2].top, 112. + GRID_GAP + 88. + GRID_GAP);
        assert_eq!(at_two[3].items, 3..5);
        let at_three = grid_rows(&sections, 3, &descriptions, 36.);
        assert_eq!(
            at_three.iter().map(|row| row.height).collect::<Vec<_>>(),
            [112., 36., 112.]
        );
    }
}
