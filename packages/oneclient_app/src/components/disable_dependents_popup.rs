//! "Other packages need this one" — the confirmation in front of a disable
//! that would take more than one package with it.
//!
//! Raised from [`crate::disable`], which resolves the impact before anything is
//! written, so a disable with nothing behind it never reaches this. Everything
//! that goes off is named: the user is being asked to make a call about their
//! own cluster, and a count alone is not enough to make it with.

use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea};
use crate::disable::{self, PendingDisable};
use crate::hooks::{invalidate_cluster_queries, use_disable_confirm};
use crate::launcher;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 480.;
const LIST_MAX_H: f32 = 260.;
const ROW_H: f32 = 37.;
const HEADING_H: f32 = 24.;

#[derive(PartialEq)]
pub struct DisableDependentsOverlay;

impl Component for DisableDependentsOverlay {
    fn render(&self) -> impl IntoElement {
        let mut pending = use_disable_confirm();
        let Some(current) = pending.read().clone() else {
            return rect().into_element();
        };

        let confirmed = current.clone();

        OverlayPopup::new()
            .on_close(move |_| pending.set(None))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(DIALOG_W))
                            .max_width(Size::window_percent(92.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(CARD_BG)
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .spacing(10.)
                                    .child(
                                        Icon::new(IconType::AlertTriangle)
                                            .size(20.)
                                            .color(colors::code_warn()),
                                    )
                                    .child(
                                        label()
                                            .text(disable::title(&current))
                                            .font_size(16.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .max_lines(2)
                                            .width(Size::fill())
                                            .color(colors::fg_primary()),
                                    ),
                            )
                            .child(
                                label()
                                    .text(disable::summary(&current))
                                    .font_size(12.)
                                    .max_lines(6)
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
                            )
                            .child(lists(&current))
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .on_press(move |_| pending.set(None))
                                            .text("Cancel"),
                                    )
                                    .child(
                                        Button::new()
                                            .danger()
                                            .on_press(move |_| {
                                                apply(confirmed.clone());
                                                pending.set(None);
                                            })
                                            .text(disable::confirm_label(&current)),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}

/// Applied off the event handler: the write is a database round trip, and the
/// modal has already closed on the user's answer.
fn apply(pending: PendingDisable) {
    spawn_forever(async move {
        if let Err(err) = disable::apply(&pending).await {
            tracing::warn!(%err, "failed to disable a package and its dependents");
            if let Ok(state) = launcher::state() {
                state
                    .services
                    .events
                    .notify("Could not disable")
                    .body(err.to_string())
                    .error()
                    .send();
            }
            return;
        }

        invalidate_cluster_queries().await;
    });
}

fn lists(pending: &PendingDisable) -> impl IntoElement {
    // The list sizes itself to its contents up to a cap, so two packages do not
    // get a scroll area with a screenful of empty space under them and twenty
    // do not push the buttons off the window.
    let rows = pending.required.len() + pending.optional.len();
    let headings =
        usize::from(!pending.required.is_empty()) + usize::from(!pending.optional.is_empty());
    let height = (rows as f32 * ROW_H + headings as f32 * HEADING_H).min(LIST_MAX_H);

    let mut scroll = ScrollArea::new()
        .width(Size::fill())
        .height(Size::px(height))
        .spacing(4.);

    if !pending.required.is_empty() {
        scroll = scroll.child(heading(disable::disabled_heading(pending)));
        for name in &pending.required {
            scroll = scroll.child(row(name, IconType::Minus, colors::danger()));
        }
    }

    if !pending.optional.is_empty() {
        scroll = scroll.child(heading(disable::optional_heading(pending)));
        for name in &pending.optional {
            scroll = scroll.child(row(name, IconType::InfoCircle, colors::fg_secondary()));
        }
    }

    scroll
}

fn heading(text: String) -> impl IntoElement {
    label()
        .text(text)
        .font_size(10.)
        .font_weight(FontWeight::SEMI_BOLD)
        .max_lines(2)
        .width(Size::fill())
        .margin(Gaps::new(6., 0., 2., 0.))
        .color(colors::fg_secondary())
}

fn row(name: &str, icon: IconType, accent: Color) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(8.)
        .padding(Gaps::new_symmetric(7., 10.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .content(Content::Flex)
        .child(Icon::new(icon).size(13.).color(accent))
        .child(
            label()
                .text(name.to_string())
                .font_size(12.5)
                .max_lines(1)
                .width(Size::flex(1.0))
                .color(colors::fg_primary()),
        )
}
