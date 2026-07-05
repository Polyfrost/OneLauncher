use freya::prelude::*;

use oneclient_core::game::Analytics;

use crate::components::{Icon, IconType};
use crate::theme::colors;
use crate::ui::border_all_color;

mod charts;
mod servers;
mod tiles;

use charts::{DailyChart, WhenChart, distribution_card};
use servers::servers_section;
use tiles::{personas_row, tiles_row};

pub fn analytics_body(analytics: &Analytics) -> Element {
    let stats = &analytics.playtime;

    let mut root = rect().vertical().width(Size::fill()).spacing(24.);

    root = root.child(tiles_row(analytics));

    if !stats.personas.is_empty() {
        root = root.child(personas_row(&stats.personas));
    }

    let mut charts: Vec<Element> = vec![
        WhenChart::from_stats(stats).into_element(),
        DailyChart::new(stats.daily.clone()).into_element(),
    ];
    if let Some(dist) = distribution_card(&stats.session_secs) {
        charts.push(dist);
    }
    root = root.child(charts_grid(charts));

    if !analytics.servers.is_empty() {
        root = root.child(servers_section(&analytics.servers));
    }

    root.into_element()
}

fn charts_grid(cards: Vec<Element>) -> Element {
    let mut grid = rect().vertical().width(Size::fill()).spacing(16.);
    for pair in cards.chunks(2) {
        if pair.len() == 1 {
            grid = grid.child(
                rect()
                    .width(Size::fill())
                    .child(pair[0].clone()),
            );
            continue;
        }

        let mut row = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .cross_align(Alignment::Start)
            .spacing(16.);
        for card in pair {
            row = row.child(rect().width(Size::flex(1.0)).child(card.clone()));
        }
        grid = grid.child(row);
    }
    grid.into_element()
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
                        .font_size(16.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .width(Size::fill())
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(12.)
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
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .maybe(enabled, |el| {
            el.on_press(on_press)
                .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        })
        .child(Icon::new(icon).size(14.).color(color))
        .into_element()
}

pub(super) fn card() -> Rect {
    rect()
        .vertical()
        .padding(20.)
        .corner_radius(CornerRadius::new_all(14.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
}
