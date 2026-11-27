use freya::prelude::*;

use oneclient_core::game::Analytics;

use crate::components::{Icon, IconType};
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, columns_for};

mod charts;
mod servers;
mod tiles;

use charts::{DailyChart, PlaytimeChart, distribution_card};
use servers::servers_section;
use tiles::{personas_row, tiles_row};

pub fn analytics_body(analytics: &Analytics) -> Element {
    analytics_body_inner(analytics, false)
}

/// `force_all` keeps the normally-hidden sections (session length servers) on screen
fn analytics_body_inner(analytics: &Analytics, force_all: bool) -> Element {
    let stats = &analytics.playtime;

    let mut root = rect().vertical().width(Size::fill()).spacing(24.);

    root = root.child(tiles_row(analytics, force_all));

    if !analytics.personas.is_empty() {
        root = root.child(personas_row(&analytics.personas));
    }

    root = root.child(DailyChart::new(stats.daily.clone()));

    let mut charts: Vec<Element> = vec![PlaytimeChart::from_stats(stats).into_element()];
    if let Some(dist) = distribution_card(&stats.session_secs, force_all) {
        charts.push(dist);
    }
    root = root.child(charts_grid(charts));

    if force_all || !analytics.servers.is_empty() {
        root = root.child(servers_section(&analytics.servers));
    }

    root.into_element()
}

/// What Statistics shows before any playtime is recorded: one readable message and the next step
///
/// It replaces a dimmed dashboard of zeros, which read as disabled rather than empty. The card
/// sits in the middle of the pane rather than filling it.
pub fn analytics_empty(title: &str, hint: &str, action: Element) -> Element {
    analytics_message(
        IconType::BarChartSquare02,
        colors::brand(),
        title,
        hint,
        action,
    )
}

/// The empty card's layout for any centered message, such as a failed load with Retry
pub(super) fn analytics_message(
    icon: IconType,
    tint: Color,
    title: &str,
    hint: &str,
    action: Element,
) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .child(crate::view::app::settings::status_card(
            icon, tint, title, hint, action,
        ))
        .into_element()
}

const CHART_GAP: f32 = 16.;
const CHART_MIN_W: f32 = 420.;

fn charts_grid(cards: Vec<Element>) -> Element {
    ChartsGrid { cards }.into_element()
}

#[derive(PartialEq)]
struct ChartsGrid {
    cards: Vec<Element>,
}

impl Component for ChartsGrid {
    fn render(&self) -> impl IntoElement {
        let mut width = use_state(|| 0f32);
        let cols = columns_for(*width.read(), CHART_MIN_W, 2, CHART_GAP);

        let mut grid = rect().vertical().width(Size::fill()).spacing(24.);
        for chunk in self.cards.chunks(cols) {
            let mut row = rect()
                .horizontal()
                .content(Content::Flex)
                .width(Size::fill())
                .cross_align(Alignment::Start)
                .spacing(CHART_GAP);
            for card in chunk {
                row = row.child(rect().width(Size::flex(1.0)).child(card.clone()));
            }
            for _ in chunk.len()..cols {
                row = row.child(rect().width(Size::flex(1.0)));
            }
            grid = grid.child(row);
        }

        grid.on_sized(move |e: Event<SizedEventData>| {
            let w = e.area.width();
            if (w - *width.peek()).abs() > 0.5 {
                width.set(w);
            }
        })
    }
}

pub(super) fn chart_card(
    title: &str,
    subtitle: String,
    trailing: Option<Element>,
    chart: Element,
) -> Element {
    card()
        .width(Size::fill())
        .spacing(16.)
        .child(card_header(title, subtitle, trailing))
        .child(chart)
        .into_element()
}

pub(super) fn card_header(title: &str, subtitle: String, trailing: Option<Element>) -> Element {
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(text_size(16.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .width(Size::fill())
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(text_size(12.))
                        .max_lines(1)
                        .width(Size::fill())
                        .color(colors::fg_secondary()),
                ),
        )
        .maybe_child(trailing)
        .into_element()
}

pub(super) fn nav_button(
    icon: IconType,
    enabled: bool,
    on_press: impl FnMut(Event<PressEventData>) + 'static,
) -> Element {
    let color = if enabled {
        colors::fg_secondary()
    } else {
        colors::fg_secondary().with_a(70)
    };
    rect()
        .width(Size::px(30.))
        .height(Size::px(30.))
        .center()
        .corner_radius(rounded(8.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .maybe(enabled, |el| {
            el.on_press(on_press).cursor(CursorIcon::Pointer)
        })
        .child(Icon::new(icon).size(14.).color(color))
        .into_element()
}

pub(super) fn card() -> Rect {
    rect()
        .vertical()
        .padding(20.)
        .corner_radius(rounded(14.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
}
