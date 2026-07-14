use freya::{prelude::*, router::RouterContext};

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{use_dispatch, use_notifications_snapshot};
use crate::notifications::ClusterUpdateSummary;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 680.;
const DIALOG_H: f32 = 400.;
const BANNER_W: f32 = 300.;

#[derive(PartialEq)]
pub struct ClusterUpdatePopup;

impl Component for ClusterUpdatePopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();

        let Some(summary) = snapshot.cluster_update.clone() else {
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
                    .child(dialog(&summary, dispatch)),
            )
            .into_element()
    }
}

fn dialog(summary: &ClusterUpdateSummary, dispatch: crate::BridgeDispatch) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::px(DIALOG_W))
        .height(Size::px(DIALOG_H))
        .max_width(Size::window_percent(95.))
        .overflow(Overflow::Clip)
        .corner_radius(CornerRadius::new_all(16.))
        .background(CARD_BG)
        .border(border_all_color(1., colors::component_border()))
        .shadow(Shadow::from((0., 18., 52., 0., Color::from_argb(150, 0, 0, 0))))
        .child(banner(summary))
        .child(content(summary, dispatch))
}

fn banner(summary: &ClusterUpdateSummary) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::px(BANNER_W))
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .background(colors::brand().with_a(30))
        .main_align(Alignment::SpaceBetween)
        // watermark
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .center()
                .child(
                    Icon::new(IconType::DownloadCloud02)
                        .size(150.)
                        .color(colors::brand().with_a(34)),
                ),
        )
        // seated title block
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(10.)
                .padding(Gaps::new(24., 20., 20., 20.))
                .background(CARD_BG.with_a(170))
                .child(
                    rect()
                        .center()
                        .padding(Gaps::new_symmetric(4., 10.))
                        .corner_radius(CornerRadius::new_all(999.))
                        .background(colors::brand())
                        .child(
                            rect()
                                .horizontal()
                                .cross_align(Alignment::Center)
                                .spacing(5.)
                                .child(Icon::new(IconType::RefreshCw01).size(12.).color(Color::WHITE))
                                .child(
                                    label()
                                        .text("Updated")
                                        .font_size(11.)
                                        .font_weight(FontWeight::SEMI_BOLD)
                                        .color(Color::WHITE),
                                ),
                        ),
                )
                .child(
                    label()
                        .text(summary.cluster_name.clone())
                        .font_size(23.)
                        .font_weight(FontWeight::BOLD)
                        .max_lines(2)
                        .color(colors::fg_primary()),
                ),
        )
}

fn content(summary: &ClusterUpdateSummary, dispatch: crate::BridgeDispatch) -> impl IntoElement {
    let dismiss = dispatch.clone();
    let cluster_id = summary.cluster_id;
    let open = move |_| {
        dispatch.close_cluster_update();
        let _ = RouterContext::get().push(Route::ClusterOverview { cluster_id });
    };

    let total = summary.total();

    let mut sections = ScrollArea::new()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .spacing(10.);

    if !summary.updated.is_empty() {
        sections = sections.child(change_section(
            IconType::RefreshCw01,
            "Updated",
            colors::brand(),
            &summary.updated,
        ));
    }
    if !summary.added.is_empty() {
        sections = sections.child(change_section(
            IconType::Plus,
            "Added",
            colors::success(),
            &summary.added,
        ));
    }
    if !summary.removed.is_empty() {
        sections = sections.child(change_section(
            IconType::Trash01,
            "Removed",
            colors::danger(),
            &summary.removed,
        ));
    }

    rect()
        .vertical()
        .width(Size::flex(1.0))
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
        .child(sections)
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

fn change_section(
    icon: IconType,
    heading: &str,
    accent: Color,
    items: &[String],
) -> impl IntoElement {
    let mut list = rect().vertical().width(Size::fill()).spacing(1.);
    for item in items {
        list = list.child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(9.)
                .padding(Gaps::new_symmetric(4., 4.))
                .child(
                    rect()
                        .width(Size::px(5.))
                        .height(Size::px(5.))
                        .corner_radius(CornerRadius::new_all(3.))
                        .background(accent),
                )
                .child(
                    label()
                        .text(item.clone())
                        .font_size(12.5)
                        .max_lines(1)
                        .width(Size::flex(1.0))
                        .color(colors::fg_primary()),
                ),
        );
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(5.)
        .padding(Gaps::new(9., 11., 11., 11.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(colors::page_elevated().with_a(140))
        .border(border_all_color(1., colors::component_border().with_a(120)))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(7.)
                .child(Icon::new(icon).size(14.).color(accent))
                .child(
                    label()
                        .text(format!("{heading} · {}", items.len()))
                        .font_size(12.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(accent),
                ),
        )
        .child(list)
}
