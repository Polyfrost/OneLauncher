use super::*;

use freya::router::RouterContext;
use oneclient_core::packages::ContentType;
use oneclient_core::settings::ViewLayout;

use crate::BridgeDispatch;
use crate::components::{
    Button, CardLayout, Dropdown, Icon, IconType, PackageEntry, PackageRow, ScrollArea, ScrollAreaCtx, Segment, SegmentedControl, TabBar, TabItem, TextInput
};
use crate::routes::Route;
use crate::theme::colors;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum SortFilter {
    NameAsc,
    NameDesc,
    SizeAsc,
    SizeDesc,
    Enabled,
    Disabled,
}

impl SortFilter {
    const ALL: [SortFilter; 6] = [
        SortFilter::NameAsc,
        SortFilter::NameDesc,
        SortFilter::SizeAsc,
        SortFilter::SizeDesc,
        SortFilter::Enabled,
        SortFilter::Disabled,
    ];

    fn key(self) -> &'static str {
        match self {
            SortFilter::NameAsc => "name_asc",
            SortFilter::NameDesc => "name_desc",
            SortFilter::SizeAsc => "size_asc",
            SortFilter::SizeDesc => "size_desc",
            SortFilter::Enabled => "enabled",
            SortFilter::Disabled => "disabled",
        }
    }

    pub(super) fn from_key(key: &str) -> Option<Self> {
        SortFilter::ALL.into_iter().find(|s| s.key() == key)
    }

    fn label(self) -> &'static str {
        match self {
            SortFilter::NameAsc => "Name (A-Z)",
            SortFilter::NameDesc => "Name (Z-A)",
            SortFilter::SizeAsc => "Size (Smallest)",
            SortFilter::SizeDesc => "Size (Largest)",
            SortFilter::Enabled => "Enabled only",
            SortFilter::Disabled => "Disabled only",
        }
    }

    pub(super) fn keep(self, p: &PackageEntry) -> bool {
        match self {
            SortFilter::Enabled => p.enabled,
            SortFilter::Disabled => !p.enabled,
            _ => true,
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
            SortFilter::NameDesc => rows.sort_by_key(|p| std::cmp::Reverse(title(p))),
            SortFilter::SizeAsc => rows.sort_by_key(|p| p.size),
            SortFilter::SizeDesc => rows.sort_by_key(|p| std::cmp::Reverse(p.size)),
            _ => rows.sort_by_key(title),
        }
    }
}

pub(super) fn toolbar(
    search: State<String>,
    mut sort: State<Option<String>>,
    current: SortFilter,
    layout: State<ViewLayout>,
) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(8.)
        .content(Content::Flex)
        .child(
            TextInput::new(search)
                .placeholder("Search packages...")
                .width(Size::flex(1.0))
                .leading(
                    Icon::new(IconType::SearchMd)
                        .size(14.)
                        .color(colors::fg_secondary())
                        .into_element(),
                ),
        )
        .child(
            Dropdown::new(
                current.label(),
                SortFilter::ALL.iter().map(|s| s.label().to_string()).collect(),
            )
            .width(Size::px(120.))
            .height(Size::px(34.))
            .on_select(move |idx: usize| {
                if let Some(option) = SortFilter::ALL.get(idx) {
                    sort.set(Some(option.key().to_string()));
                }
            }),
        )
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

pub(super) fn header(
    title: &'static str,
    count_label: String,
    package_type: &'static str,
    content_type: ContentType,
    cluster_id: i64,
    dispatch: BridgeDispatch,
) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(12.)
        .content(Content::Flex)
        .child(
            label()
                .text(title)
                .font_size(20.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(pill(count_label))
        .child(
            rect()
                .horizontal()
                .width(Size::flex(1.0))
                .main_align(Alignment::End)
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(add_from_file_button(cluster_id, content_type, dispatch))
                .child(browse_button(cluster_id, package_type)),
        )
        .into_element()
}

pub(super) fn tab_bar(
    tabs: &[Tab],
    items: &[PackageEntry],
    active_idx: usize,
    active: State<usize>,
) -> impl IntoElement {
    let tab_items = tabs.iter().enumerate().map(|(i, tab)| {
        let total = items.iter().filter(|p| tab.matches(p)).count();
        let enabled = items.iter().filter(|p| tab.matches(p) && p.enabled).count();
        let mut active = active;
        TabItem::new(tab.label(), i == active_idx)
            .count_text(format!("{enabled}/{total}"))
            .on_press(move |_| *active.write() = i)
    });

    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .overflow(Overflow::Clip)
        .padding(Gaps::new_symmetric(10., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(PANEL_BG)
        .child(
            TabBar::new()
                .width(Size::fill())
                .height(Size::auto())
                .spacing(24.)
                .font_size(12.)
                .bold_active(true)
                .tabs(tab_items),
        )
        .into_element()
}

fn add_from_file_button(
    cluster_id: i64,
    content_type: ContentType,
    dispatch: BridgeDispatch,
) -> impl IntoElement {
    Button::new()
        .secondary()
        .on_press(move |_| {
            let dispatch = dispatch.clone();
            spawn(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_title("Select a file to import")
                    .pick_file()
                    .await
                {
                    dispatch.import_local_file(
                        cluster_id,
                        content_type,
                        handle.path().to_path_buf(),
                    );
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

fn pill(text: String) -> impl IntoElement {
    rect()
        .center()
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(colors::component_bg())
        .child(
            label()
                .text(text)
                .font_size(9.)
                .color(colors::fg_secondary()),
        )
}

pub(super) fn list(
    items: Vec<PackageEntry>,
    noun_plural: &'static str,
    package_type: &'static str,
    cluster_id: i64,
    layout: CardLayout,
) -> impl IntoElement {
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

    rect()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .padding(Gaps::new_all(8.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(PANEL_BG)
        .overflow(Overflow::Clip)
        .maybe_child(scroll)
        .maybe_child((count == 0).then(|| empty_state(noun_plural)))
        .into_element()
}

fn grid_content(
    items: &[PackageEntry],
    package_type: &'static str,
    cluster_id: i64,
    ctx: ScrollAreaCtx,
) -> impl IntoElement {
    let count = items.len();
    let cols = (((ctx.viewport_w + GRID_GAP) / (GRID_MIN_W + GRID_GAP)).floor() as usize)
        .clamp(1, 3);
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
            let cell = rect()
                .width(Size::flex(1.0))
                .height(Size::px(CARD_GRID_H));
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

fn empty_state(noun_plural: &'static str) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .padding(Gaps::new_all(48.))
        .spacing(8.)
        .child(
            Icon::new(IconType::DotsGrid)
                .size(28.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(format!("No {noun_plural} here yet."))
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text("Add one from a file or browse provider content.")
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
