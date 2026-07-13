use super::*;

use freya::animation::{AnimNum, Ease, OnCreation, use_animation};
use freya::router::RouterContext;
use oneclient_core::packages::ContentType;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Button, CardLayout, Icon, IconType, PackageEntry, PackageRow, ScrollArea, ScrollAreaCtx,
    Segment, SegmentedControl, TabBar, TabItem, TextInput,
};
use crate::hooks::use_dispatch;
use crate::routes::Route;
use crate::theme::colors;
use crate::{BridgeDispatch, utils};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum SortMode {
    NameAsc,
    NameDesc,
    SizeAsc,
    SizeDesc,
}

impl SortMode {
    const ALL: [SortMode; 4] = [
        SortMode::NameAsc,
        SortMode::NameDesc,
        SortMode::SizeAsc,
        SortMode::SizeDesc,
    ];

    fn key(self) -> &'static str {
        match self {
            SortMode::NameAsc => "name_asc",
            SortMode::NameDesc => "name_desc",
            SortMode::SizeAsc => "size_asc",
            SortMode::SizeDesc => "size_desc",
        }
    }

    pub(super) fn from_key(key: &str) -> Option<Self> {
        SortMode::ALL.into_iter().find(|s| s.key() == key)
    }

    fn label(self) -> &'static str {
        match self {
            SortMode::NameAsc => "Name (A-Z)",
            SortMode::NameDesc => "Name (Z-A)",
            SortMode::SizeAsc => "Size (Smallest)",
            SortMode::SizeDesc => "Size (Largest)",
        }
    }

    pub(super) fn sort(self, rows: &mut [PackageEntry]) {
        let title = |p: &PackageEntry| {
            if p.is_remote() {
                p.name.to_lowercase()
            } else {
                p.file_name.to_lowercase()
            }
        };

        match self {
            SortMode::NameDesc => rows.sort_by_key(|p| std::cmp::Reverse(title(p))),
            SortMode::SizeAsc => rows.sort_by_key(|p| p.size),
            SortMode::SizeDesc => rows.sort_by_key(|p| std::cmp::Reverse(p.size)),
            _ => rows.sort_by_key(title),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum EnabledFilter {
    All,
    Enabled,
    Disabled,
}

impl EnabledFilter {
    const ALL: [EnabledFilter; 3] = [
        EnabledFilter::All,
        EnabledFilter::Enabled,
        EnabledFilter::Disabled,
    ];

    fn label(self) -> &'static str {
        match self {
            EnabledFilter::All => "All packages",
            EnabledFilter::Enabled => "Enabled only",
            EnabledFilter::Disabled => "Disabled only",
        }
    }

    pub(super) fn keep(self, p: &PackageEntry) -> bool {
        match self {
            EnabledFilter::All => true,
            EnabledFilter::Enabled => p.enabled,
            EnabledFilter::Disabled => !p.enabled,
        }
    }
}

const FILTER_PANEL_W: f32 = 172.;
const FILTER_BTN_W: f32 = 34.;

#[allow(clippy::too_many_arguments)]
pub(super) fn toolbar_bar(
    tabs: &[Tab],
    active_idx: usize,
    active: State<usize>,
    search: State<String>,
    sort: State<Option<String>>,
    current_sort: SortMode,
    enabled_filter: State<EnabledFilter>,
    layout: State<ViewLayout>,
) -> impl IntoElement {
    let tab_items = tabs.iter().enumerate().map(|(i, tab)| {
        let mut active = active;
        TabItem::new(tab.label(), i == active_idx).on_press(move |_| *active.write() = i)
    });

    let mut top_corners = CornerRadius::new_all(0.);
    top_corners.fill_top(12.);

    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(8.)
        .content(Content::Flex)
        .overflow(Overflow::Clip)
        .padding(Gaps::new_symmetric(8., 12.))
        .corner_radius(top_corners)
        .background(colors::page_elevated())
        .child(
            ScrollView::new()
                .direction(Direction::Horizontal)
                .show_scrollbar(false)
                .width(Size::flex(1.0))
                .height(Size::auto())
                .child(
                    TabBar::new()
                        .width(Size::auto())
                        .height(Size::auto())
                        .spacing(20.)
                        .font_size(12.)
                        .tabs(tab_items),
                ),
        )
        .child(
            TextInput::new(search)
                .placeholder("Search...")
                .width(Size::px(180.))
                .leading(
                    Icon::new(IconType::SearchMd)
                        .size(14.)
                        .color(colors::fg_secondary())
                        .into_element(),
                ),
        )
        .child(FilterButton {
            sort,
            current_sort,
            enabled_filter,
        })
        .child(
            SegmentedControl::new(layout)
                .height(34.)
                .icon_size(15.)
                .equal_width(34.)
                .segment(Segment::new(ViewLayout::List).icon(IconType::ParagraphWrap))
                .segment(Segment::new(ViewLayout::Grid).icon(IconType::DotsGrid)),
        )
        .into_element()
}

#[derive(PartialEq)]
struct FilterButton {
    sort: State<Option<String>>,
    current_sort: SortMode,
    enabled_filter: State<EnabledFilter>,
}

impl Component for FilterButton {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);

        let sort = self.sort;
        let current_sort = self.current_sort;
        let enabled_filter = self.enabled_filter;

        let is_open = open();
        let active =
            current_sort != SortMode::NameAsc || *enabled_filter.read() != EnabledFilter::All;

        let icon_color = if active {
            colors::brand()
        } else {
            colors::fg_secondary()
        };

        rect()
            .width(Size::px(FILTER_BTN_W))
            .child(
                Button::new()
                    .secondary()
                    .icon()
                    .width(Size::px(FILTER_BTN_W))
                    .height(Size::px(34.))
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        open.toggle();
                    })
                    .child(Icon::new(IconType::Sliders04).size(16.).color(icon_color)),
            )
            .maybe_child(is_open.then(|| {
                let on_close: EventHandler<()> = (move |()| open.set(false)).into();
                FilterPopover {
                    sort,
                    current_sort,
                    enabled_filter,
                    on_close,
                }
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct FilterPopover {
    sort: State<Option<String>>,
    current_sort: SortMode,
    enabled_filter: State<EnabledFilter>,
    on_close: EventHandler<()>,
}

impl Component for FilterPopover {
    fn render(&self) -> impl IntoElement {
        let mut sort = self.sort;
        let current_sort = self.current_sort;
        let mut enabled_filter = self.enabled_filter;
        let backdrop_close = self.on_close.clone();
        let key_close = self.on_close.clone();

        let a11y_id = use_a11y();

        let fade = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(160).ease(Ease::Out)
        });
        let progress = fade.read().value();

        let show = *enabled_filter.read();

        let mut panel = rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .padding(Gaps::new_all(8.))
            .opacity(progress)
            .offset_y((progress - 1.0) * 6.)
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::page_elevated().with_a(230))
            .blur(12.)
            .border(crate::ui::border_all_color(1., colors::component_border()))
            .shadow(Shadow::from((0., 8., 32., 0., Color::from_argb(120, 0, 0, 0))))
            .a11y_id(a11y_id)
            .a11y_role(AccessibilityRole::Dialog)
            .on_global_key_down(move |e: Event<KeyboardEventData>| {
                if e.key == Key::Named(NamedKey::Escape) {
                    key_close.call(());
                }
            })
            .child(section_label("Sort by"));

        for mode in SortMode::ALL {
            let selected = mode == current_sort;
            let on_press: EventHandler<Event<PressEventData>> = (move |_| {
                sort.set(Some(mode.key().to_string()));
            })
            .into();
            panel = panel.child(ChoiceRow {
                text: mode.label(),
                selected,
                on_press,
            });
        }

        panel = panel.child(section_label("Show"));
        for filter in EnabledFilter::ALL {
            let selected = filter == show;
            let on_press: EventHandler<Event<PressEventData>> = (move |_| {
                enabled_filter.set(filter);
            })
            .into();
            panel = panel.child(ChoiceRow {
                text: filter.label(),
                selected,
                on_press,
            });
        }

        rect()
            .height(Size::px(0.))
            .width(Size::px(FILTER_PANEL_W))
            .layer(Layer::Overlay)
            .child(
                rect()
                    .layer(Layer::RelativeOverlay(10))
                    .position(Position::new_global().top(0.).left(0.))
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .on_press(move |_| backdrop_close.call(())),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .layer(Layer::RelativeOverlay(12))
                    .margin(Gaps::new(6., 0., 0., -(FILTER_PANEL_W - FILTER_BTN_W)))
                    .child(panel),
            )
    }
}

fn section_label(text: &'static str) -> impl IntoElement {
    label()
        .text(text)
        .font_size(10.)
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_secondary())
}

#[derive(PartialEq)]
struct ChoiceRow {
    text: &'static str,
    selected: bool,
    on_press: EventHandler<Event<PressEventData>>,
}

impl Component for ChoiceRow {
    fn render(&self) -> impl IntoElement {
        let text = self.text;
        let selected = self.selected;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let color = if selected {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        let bg = if selected {
            colors::component_bg()
        } else if focused {
            colors::ghost_overlay_hover()
        } else {
            Color::TRANSPARENT
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .padding(Gaps::new_symmetric(5., 8.))
            .corner_radius(CornerRadius::new_all(6.))
            .background(bg)
            .content(Content::Flex)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::MenuItemRadio)
            .maybe(focused, |el| {
                el.border(crate::ui::border_all_color(1., colors::brand()))
            })
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
            .on_all_press(self.on_press.clone())
            .child(
                label()
                    .text(text)
                    .font_size(12.)
                    .width(Size::flex(1.0))
                    .color(color),
            )
            .maybe_child(selected.then(|| {
                Icon::new(IconType::Check)
                    .size(14.)
                    .color(colors::brand())
                    .into_element()
            }))
    }
}

fn add_from_file_button(
    cluster_id: i64,
    content_type: ContentType,
    dispatch: BridgeDispatch,
) -> impl IntoElement {
    Button::new()
        .primary()
        .on_press(move |_| {
            let dispatch = dispatch.clone();
            spawn(async move {
                if let Some(handles) = rfd::AsyncFileDialog::new()
                    .set_title("Select files to import")
                    .pick_files()
                    .await
                {
                    for handle in handles {
                        dispatch.import_local_file(
                            cluster_id,
                            content_type,
                            handle.path().to_path_buf(),
                        );
                    }
                }
            });
        })
        .child(Icon::new(IconType::FilePlus02).size(14.))
        .text("Add from file")
}

fn browse_button(cluster_id: i64, package_type: &'static str) -> impl IntoElement {
    Button::new()
        .primary()
        .on_press(move |_| {
            let _ = RouterContext::get().push(Route::Browser {
                cluster_id,
                package_type: package_type.to_string(),
            });
        })
        .child(Icon::new(IconType::SearchMd).size(14.))
        .text("Browse Content")
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum ContentKind {
    External,
    Local,
    Other,
}

#[derive(PartialEq)]
pub(super) struct ContentBox {
    items: Vec<PackageEntry>,
    noun_plural: &'static str,
    package_type: &'static str,
    content_type: ContentType,
    cluster_id: i64,
    kind: ContentKind,
    layout: CardLayout,
}

impl ContentBox {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        items: Vec<PackageEntry>,
        noun_plural: &'static str,
        package_type: &'static str,
        content_type: ContentType,
        cluster_id: i64,
        kind: ContentKind,
        layout: CardLayout,
    ) -> Self {
        Self {
            items,
            noun_plural,
            package_type,
            content_type,
            cluster_id,
            kind,
            layout,
        }
    }
}

impl Component for ContentBox {
    fn render(&self) -> impl IntoElement {
        let items = self.items.clone();
        let package_type = self.package_type;
        let content_type = self.content_type;
        let cluster_id = self.cluster_id;
        let noun_plural = self.noun_plural;
        let kind = self.kind;
        let layout = self.layout;

        let dispatch = use_dispatch();
        let mut dropping = use_state(|| false);

        let count = items.len();
        let scroll = (count > 0).then(|| match layout {
            CardLayout::List => {
                let items = items.clone();
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .lazy(count, CARD_H, CARD_SPACING, move |i| {
                        let item = items[i].clone();
                        let key = item.package_id.clone();
                        PackageRow::new(item, cluster_id, package_type)
                            .layout(CardLayout::List)
                            .key(key)
                            .into_element()
                    })
                    .into_element()
            }
            CardLayout::Grid => ScrollArea::new()
                .width(Size::fill())
                .height(Size::fill())
                .content(move |ctx: ScrollAreaCtx| {
                    grid_content(&items, package_type, cluster_id, ctx).into_element()
                })
                .into_element(),
        });

        let empty = (count == 0).then(|| match kind {
            ContentKind::External => external_empty(cluster_id, package_type).into_element(),
            ContentKind::Local => {
                local_empty(cluster_id, content_type, dispatch.clone(), noun_plural).into_element()
            }
            ContentKind::Other => empty_state(noun_plural).into_element(),
        });

        let header = (count > 0)
            .then(|| match kind {
                ContentKind::External => Some(
                    action_header(browse_button(cluster_id, package_type)).into_element(),
                ),
                ContentKind::Local => Some(
                    action_header(add_from_file_button(cluster_id, content_type, dispatch.clone()))
                        .into_element(),
                ),
                ContentKind::Other => None,
            })
            .flatten();

        let mut bottom_corners = CornerRadius::new_all(0.);
        bottom_corners.fill_bottom(12.);

        let drop_dispatch = dispatch.clone();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .spacing(8.)
            .padding(Gaps::new_all(8.))
            .corner_radius(bottom_corners)
            .background(colors::page_elevated())
            .overflow(Overflow::Clip)
            .maybe(*dropping.read(), |el| {
                el.border(
                    Border::new()
                        .fill(colors::brand())
                        .width(2.)
                        .alignment(BorderAlignment::Inner),
                )
            })
            .on_global_file_hover(move |_| dropping.set(true))
            .on_global_file_hover_cancelled(move |_| dropping.set(false))
            .on_file_drop(move |e: Event<FileEventData>| {
                dropping.set(false);
                if let Some(path) = &e.file_path {
                    drop_dispatch.import_local_file(cluster_id, content_type, path.clone());
                }
            })
            .maybe_child(header)
            .maybe_child(scroll)
            .maybe_child(empty)
    }
}

fn action_header(button: impl IntoElement) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .child(rect().width(Size::flex(1.0)))
        .child(button)
}

fn grid_content(
    items: &[PackageEntry],
    package_type: &'static str,
    cluster_id: i64,
    ctx: ScrollAreaCtx,
) -> impl IntoElement {
    let count = items.len();
    let cols =
        (((ctx.viewport_w + GRID_GAP) / (GRID_MIN_W + GRID_GAP)).floor() as usize).clamp(1, 3);
    let rows_total = count.div_ceil(cols);
    let slot = CARD_GRID_H + GRID_GAP;

    let first_row = (((-ctx.corrected_y) / slot).floor() as i64 - LAZY_OVERSCAN).max(0) as usize;
    let span = ((ctx.viewport_h / slot).ceil() as i64 + 2 * LAZY_OVERSCAN).max(0) as usize;
    let last_row = (first_row + span).min(rows_total);

    let top_pad = first_row as f32 * slot;
    let bottom_pad = rows_total.saturating_sub(last_row) as f32 * slot;

    let mut container = rect().vertical().width(Size::fill());
    if top_pad > 0. {
        container = container.child(rect().width(Size::fill()).height(Size::px(top_pad)));
    }
    for r in first_row..last_row {
        let mut row = rect()
            .key(r)
            .horizontal()
            .width(Size::fill())
            .height(Size::px(slot))
            .spacing(GRID_GAP)
            .content(Content::Flex);
        for c in 0..cols {
            let idx = r * cols + c;
            let cell = rect().width(Size::flex(1.0)).height(Size::px(CARD_GRID_H));
            row = row.child(if idx < count {
                let item = items[idx].clone();
                let key = item.package_id.clone();
                cell.child(
                    PackageRow::new(item, cluster_id, package_type)
                        .layout(CardLayout::Grid)
                        .key(key)
                        .into_element(),
                )
            } else {
                cell
            });
        }
        container = container.child(row);
    }
    if bottom_pad > 0. {
        container = container.child(rect().width(Size::fill()).height(Size::px(bottom_pad)));
    }
    container.into_element()
}

fn empty_shell(icon: IconType) -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .padding(Gaps::new_all(48.))
        .spacing(8.)
        .child(Icon::new(icon).size(28.).color(colors::fg_secondary()))
}

fn empty_title(text: impl Into<String>) -> impl IntoElement {
    label()
        .text(text.into())
        .font_size(14.)
        .color(colors::fg_secondary())
}

fn empty_hint(text: impl Into<String>) -> impl IntoElement {
    label()
        .text(text.into())
        .font_size(12.)
        .color(colors::fg_secondary())
}

fn empty_state(noun_plural: &'static str) -> impl IntoElement {
    empty_shell(IconType::DotsGrid)
        .child(empty_title(format!("No {noun_plural} here yet.")))
        .child(empty_hint(
            "Add one from a file or browse provider content.",
        ))
        .into_element()
}

fn external_empty(cluster_id: i64, package_type: &'static str) -> impl IntoElement {
    empty_shell(IconType::SearchMd)
        .child(empty_title("No external content installed."))
        .child(empty_hint(
            "Browse providers to add mods, resource packs and more.",
        ))
        .child(rect().height(Size::px(6.)))
        .child(browse_button(cluster_id, package_type))
        .into_element()
}

fn local_empty(
    cluster_id: i64,
    content_type: ContentType,
    dispatch: BridgeDispatch,
    noun_plural: &'static str,
) -> impl IntoElement {
    let is_wayland = utils::is_wayland();

    empty_shell(IconType::FilePlus02)
        .child(empty_title(format!("No local {noun_plural} yet.")))
        .maybe(!is_wayland, |e| {
            e.child(empty_hint(
                "Tip: drag files onto the window to install them.",
            ))
        })
        .child(rect().height(Size::px(6.)))
        .child(add_from_file_button(cluster_id, content_type, dispatch))
        .into_element()
}
