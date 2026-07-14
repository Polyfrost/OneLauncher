use super::*;

use freya::router::RouterContext;
use oneclient_core::packages::types::ProjectSummary;
use oneclient_core::packages::ProviderId;

use crate::components::{
    Icon, IconType,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;

pub(super) fn grid_row(row: Vec<ProjectSummary>, cluster_id: i64, package_type: &str) -> impl IntoElement {
    let package_type = package_type.to_string();
    let fill = GRID_COLUMNS - row.len();

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(CARD_H))
        .spacing(GRID_SPACING)
        .content(Content::Flex)
        .children(row.into_iter().map(move |item| {
            PackageCard::new(item, cluster_id, package_type.clone()).into_element()
        }))
        .maybe(fill > 0, move |mut el| {
            for _ in 0..fill {
                el = el.child(rect().width(Size::flex(1.0)));
            }
            el
        })
        .into_element()
}

fn open_package(cluster_id: i64, package_type: &str, provider: ProviderId, id: &str) {
    let _ = RouterContext::get().push(Route::BrowserPackage {
        cluster_id,
        package_type: package_type.to_string(),
        package_id: encode_package_id(provider, id),
    });
}

#[derive(PartialEq)]
struct PackageCard {
    item: ProjectSummary,
    cluster_id: i64,
    package_type: String,
}

impl PackageCard {
    fn new(item: ProjectSummary, cluster_id: i64, package_type: String) -> Self {
        Self {
            item,
            cluster_id,
            package_type,
        }
    }
}

impl Component for PackageCard {
    fn render(&self) -> impl IntoElement {
        let id = self.item.id.clone();
        let provider = self.item.provider;
        let package_type = self.package_type.clone();
        let icon_url = self.item.icon_url.clone();
        let cluster_id = self.cluster_id;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        use_drop(|| {
            Cursor::set(CursorIcon::default());
        });

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .height(Size::px(CARD_H))
            .corner_radius(CornerRadius::new_all(10.))
            .background(CARD_BG)
            .border(border_all_color(
                1.,
                if focused {
                    colors::brand()
                } else {
                    colors::component_border()
                },
            ))
            .overflow(Overflow::Clip)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
            .on_all_press(move |_| open_package(cluster_id, &package_type, provider, &id))
            .child(
                rect()
                    .margin(Gaps::new_all(1.))
                    .corner_radius(CornerRadius {
                        top_left: 10.,
                        top_right: 10.,
                        ..Default::default()
                    })
                    .overflow(Overflow::Clip)
                    .child(PackageBanner::new(icon_url, BANNER_H)),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .padding(Gaps::new_all(12.))
                    .main_align(Alignment::SpaceBetween)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(6.)
                            .child(
                                label()
                                    .text(self.item.name.clone())
                                    .font_size(16.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_lines(1)
                                    .color(CARD_NAME),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .spacing(4.)
                                    .child(
                                        label()
                                            .text(format!("by {}", self.item.author))
                                            .font_size(10.)
                                            .max_lines(1)
                                            .color(colors::fg_secondary()),
                                    )
                                    .child(Icon::new(self.item.provider).size(12.)),
                            )
                            .child(
                                label()
                                    .text(self.item.summary.clone())
                                    .font_size(11.)
                                    .max_lines(2)
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(downloads_row(self.item.downloads)),
            )
    }
}

pub(super) fn list_row(item: ProjectSummary, cluster_id: i64, package_type: &str) -> impl IntoElement {
    ListRow {
        item,
        cluster_id,
        package_type: package_type.to_string(),
    }
}

#[derive(PartialEq)]
struct ListRow {
    item: ProjectSummary,
    cluster_id: i64,
    package_type: String,
}

impl Component for ListRow {
    fn render(&self) -> impl IntoElement {
        let item = self.item.clone();
        let cluster_id = self.cluster_id;
        let id = item.id.clone();
        let provider = item.provider;
        let package_type = self.package_type.clone();

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        use_drop(|| {
            Cursor::set(CursorIcon::default());
        });

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(LIST_ROW_H))
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new_all(16.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(CARD_BG)
        .border(border_all_color(
            1.,
            if focused {
                colors::brand()
            } else {
                colors::component_border()
            },
        ))
        .overflow(Overflow::Clip)
        .content(Content::Flex)
        .a11y_id(a11y_id)
        .a11y_focusable(true)
        .a11y_role(AccessibilityRole::Button)
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_all_press(move |_| open_package(cluster_id, &package_type, provider, &id))
        .child(Thumbnail::new(item.icon_url.clone(), 48.).radius(8.))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(6.)
                        .child(
                            label()
                                .text(item.name.clone())
                                .font_size(15.)
                                .font_weight(FontWeight::MEDIUM)
                                .max_lines(1)
                                .color(CARD_NAME),
                        )
                        .child(
                            label()
                                .text(format!("by {}", item.author))
                                .font_size(10.)
                                .max_lines(1)
                                .color(colors::fg_secondary()),
                        )
                        .child(Icon::new(item.provider).size(12.)),
                )
                .child(
                    label()
                        .text(item.summary.clone())
                        .font_size(11.)
                        .max_lines(2)
                        .width(Size::fill())
                        .color(colors::fg_secondary()),
                ),
        )
        .child(downloads_row(item.downloads))
    }
}

fn downloads_row(downloads: u64) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.)
        .child(
            Icon::new(IconType::Download01)
                .size(12.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(abbreviate_number(downloads))
                .font_size(11.)
                .color(colors::fg_secondary()),
        )
}

pub(super) fn empty_state() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .child(
            label()
                .text("No results. Try a different search, category or provider.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
}
