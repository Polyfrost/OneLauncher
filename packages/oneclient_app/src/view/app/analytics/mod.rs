use chrono::{Duration, Local};
use freya::prelude::*;

use oneclient_core::game::{Analytics, DayPlaytime, PlaytimeStats};

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
    analytics_body_inner(analytics, false)
}

/// `force_all` keeps the sections that are normally hidden for lack of data
/// (session length, servers) on screen, so the zeroed empty state still shows
/// the whole layout.
fn analytics_body_inner(analytics: &Analytics, force_all: bool) -> Element {
    let stats = &analytics.playtime;

    let mut root = rect().vertical().width(Size::fill()).spacing(24.);

    root = root.child(tiles_row(analytics, force_all));

    if !stats.personas.is_empty() {
        root = root.child(personas_row(&stats.personas));
    }

    let mut charts: Vec<Element> = vec![
        WhenChart::from_stats(stats).into_element(),
        DailyChart::new(stats.daily.clone()).into_element(),
    ];
    if let Some(dist) = distribution_card(&stats.session_secs, force_all) {
        charts.push(dist);
    }
    root = root.child(charts_grid(charts));

    if force_all || !analytics.servers.is_empty() {
        root = root.child(servers_section(&analytics.servers));
    }

    root.into_element()
}

/// Renders the full analytics layout with zeroed data, dimmed behind a scrim
/// with `note` centered on top. Used by the cluster overview and global
/// statistics pages when nothing has been recorded yet, so the charts and stats
/// are always visible without inventing numbers.
pub fn analytics_placeholder(note: &str) -> Element {
    rect()
        .width(Size::fill())
        .child(analytics_body_inner(&empty_analytics(), true))
        .child(
            rect()
                .position(Position::new_absolute())
                .width(Size::fill())
                .height(Size::fill())
                .layer(Layer::Relative(1))
                .center()
                .padding(40.)
                .background(colors::page_overlay())
                // swallow clicks so the placeholder controls aren't interactive
                .on_press(|_| {})
                .child(
                    rect()
                        .max_width(Size::px(360.))
                        .padding(Gaps::new_symmetric(16., 24.))
                        .corner_radius(CornerRadius::new_all(12.))
                        .background(colors::page_elevated())
                        .border(border_all_color(1., colors::component_border()))
                        .center()
                        .child(
                            label()
                                .text(note.to_string())
                                .font_size(14.)
                                .text_align(TextAlign::Center)
                                .color(colors::fg_secondary()),
                        ),
                ),
        )
        .into_element()
}

/// Every field zeroed: real, honest "nothing recorded" data rather than
/// invented numbers. The daily series is a run of zero-second days ending today
/// so the timeline still renders its columns for each range selector.
fn empty_analytics() -> Analytics {
    const EMPTY_DAYS: i64 = 90;

    let today = Local::now().date_naive();
    let daily: Vec<DayPlaytime> = (0..EMPTY_DAYS)
        .rev()
        .map(|i| DayPlaytime {
            date: (today - Duration::days(i)).format("%Y-%m-%d").to_string(),
            secs: 0,
        })
        .collect();

    let playtime = PlaytimeStats {
        total_secs: 0,
        session_count: 0,
        per_weekday: [0; 7],
        per_hour: [0; 24],
        daily,
        session_secs: Vec::new(),
        active_days: 0,
        avg_secs_per_active_day: 0.0,
        peak_hour: None,
        peak_weekday: None,
        night_share: 0.0,
        personas: Vec::new(),
    };

    Analytics {
        playtime,
        servers: Vec::new(),
    }
}

fn charts_grid(cards: Vec<Element>) -> Element {
    let mut grid = rect().vertical().width(Size::fill()).spacing(24.);
    for pair in cards.chunks(2) {
        if pair.len() == 1 {
            grid = grid.child(rect().width(Size::fill()).child(pair[0].clone()));
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
