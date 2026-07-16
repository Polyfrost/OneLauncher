use std::collections::HashMap;

use chrono::{Local, NaiveDate};
use freya::prelude::*;

use oneclient_core::game::{DayPlaytime, PlaytimeStats, WEEKDAY_LABELS};

use crate::components::{BarChart, IconType, Segment, SegmentedControl, ValueUnit};
use crate::utils::{format_day, format_duration_hm, format_hour, parse_day};

use super::{chart_card, nav_button};

#[derive(Clone, Copy, PartialEq)]
enum WhenMode {
    Weekday,
    Hour,
}

#[derive(PartialEq)]
pub(super) struct WhenChart {
    per_weekday: Vec<i64>,
    per_hour: Vec<i64>,
    peak_weekday: Option<usize>,
    peak_hour: Option<usize>,
}

impl WhenChart {
    pub(super) fn from_stats(stats: &PlaytimeStats) -> Self {
        Self {
            per_weekday: stats.per_weekday.to_vec(),
            per_hour: stats.per_hour.to_vec(),
            peak_weekday: stats.peak_weekday,
            peak_hour: stats.peak_hour,
        }
    }
}

impl Component for WhenChart {
    fn render(&self) -> impl IntoElement {
        let mode = use_state(|| WhenMode::Weekday);
        let m = *mode.read();

        let control = SegmentedControl::new(mode)
            .height(30.)
            .segment(Segment::new(WhenMode::Weekday).label("Weekday"))
            .segment(Segment::new(WhenMode::Hour).label("Hour"))
            .into_element();

        let (subtitle, chart) = match m {
            WhenMode::Weekday => {
                let day = self
                    .peak_weekday
                    .map(|i| WEEKDAY_LABELS[i].to_string())
                    .unwrap_or_else(|| "—".to_string());
                (
                    format!("Most active on {day}"),
                    BarChart::new(self.per_weekday.clone(), weekday_labels())
                        .highlight(self.peak_weekday)
                        .unit(ValueUnit::Duration)
                        .gap(6.)
                        .into_element(),
                )
            }
            WhenMode::Hour => {
                let hour = self
                    .peak_hour
                    .map(format_hour)
                    .unwrap_or_else(|| "—".to_string());
                (
                    format!("Peak around {hour}"),
                    BarChart::new(self.per_hour.clone(), (0..24).map(format_hour).collect())
                        .highlight(self.peak_hour)
                        .unit(ValueUnit::Duration)
                        .gap(3.)
                        .into_element(),
                )
            }
        };

        chart_card("When you play", subtitle, Some(control), chart)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Range {
    TwoWeeks,
    Month,
    Quarter,
}

impl Range {
    fn days(self) -> usize {
        match self {
            Range::TwoWeeks => 14,
            Range::Month => 30,
            Range::Quarter => 90,
        }
    }
}

#[derive(PartialEq)]
pub(super) struct DailyChart {
    daily: Vec<DayPlaytime>,
}

impl DailyChart {
    pub(super) fn new(daily: Vec<DayPlaytime>) -> Self {
        Self { daily }
    }
}

impl Component for DailyChart {
    fn render(&self) -> impl IntoElement {
        let range = use_state(|| Range::TwoWeeks);
        let mut offset = use_state(|| 0usize);

        let series = continuous_series(&self.daily);
        if series.is_empty() {
            return chart_card(
                "Daily playtime",
                "No sessions recorded yet".to_string(),
                None,
                rect().height(Size::px(120.)).into_element(),
            );
        }

        let window = range.read().days();
        let total = series.len();
        let max_offset = total.saturating_sub(1) / window;
        let off = (*offset.read()).min(max_offset);

        let end = total.saturating_sub(off * window);
        let start = end.saturating_sub(window);
        let slice = &series[start..end];

        let values: Vec<i64> = slice.iter().map(|(_, s)| *s).collect();
        let labels: Vec<String> = slice.iter().map(|(d, _)| format_day(*d)).collect();

        let win_total: i64 = values.iter().sum();
        let subtitle = match (slice.first(), slice.last()) {
            (Some((a, _)), Some((b, _))) => {
                format!(
                    "{} · {} – {}",
                    format_duration_hm(win_total),
                    format_day(*a),
                    format_day(*b)
                )
            }
            _ => format_duration_hm(win_total),
        };

        let can_older = off < max_offset;
        let can_newer = off > 0;
        let nav = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(nav_button(IconType::ChevronsLeft, can_older, move |_| {
                if can_older {
                    *offset.write() = off + 1;
                }
            }))
            .child(nav_button(IconType::ChevronsRight, can_newer, move |_| {
                if can_newer {
                    *offset.write() = off - 1;
                }
            }))
            .child(
                SegmentedControl::new(range)
                    .height(30.)
                    .segment(Segment::new(Range::TwoWeeks).label("2W"))
                    .segment(Segment::new(Range::Month).label("1M"))
                    .segment(Segment::new(Range::Quarter).label("3M"))
                    .into_element(),
            )
            .into_element();

        chart_card(
            "Daily playtime",
            subtitle,
            Some(nav),
            BarChart::new(values, labels)
                .unit(ValueUnit::Duration)
                .gap(3.)
                .into_element(),
        )
    }
}

fn continuous_series(daily: &[DayPlaytime]) -> Vec<(NaiveDate, i64)> {
    let map: HashMap<NaiveDate, i64> = daily
        .iter()
        .filter_map(|d| parse_day(&d.date).map(|date| (date, d.secs)))
        .collect();

    let Some(first) = map.keys().min().copied() else {
        return Vec::new();
    };
    let today = Local::now().date_naive();
    let last = map.keys().max().copied().unwrap_or(first).max(today);

    let mut out = Vec::new();
    let mut cur = first;
    while cur <= last {
        out.push((cur, map.get(&cur).copied().unwrap_or(0)));
        cur = match cur.succ_opt() {
            Some(next) => next,
            None => break,
        };
    }
    out
}

pub(super) fn distribution_card(session_secs: &[i64], force: bool) -> Option<Element> {
    if session_secs.len() < 2 && !force {
        return None;
    }

    const EDGES: [(i64, &str); 6] = [
        (15 * 60, "<15m"),
        (30 * 60, "15m-30m"),
        (60 * 60, "30m-60m"),
        (2 * 3600, "1h-2h"),
        (4 * 3600, "2h-4h"),
        (i64::MAX, "4h+"),
    ];

    let mut counts = [0i64; 6];
    for &s in session_secs {
        let idx = EDGES.iter().position(|(edge, _)| s < *edge).unwrap_or(5);
        counts[idx] += 1;
    }

    let labels: Vec<String> = EDGES.iter().map(|(_, l)| (*l).to_string()).collect();
    let peak = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| **c)
        .filter(|(_, c)| **c > 0)
        .map(|(i, _)| i);

    let subtitle = format!("{} sessions grouped by length", session_secs.len());

    Some(chart_card(
        "Session length",
        subtitle,
        None,
        BarChart::new(counts.to_vec(), labels)
            .highlight(peak)
            .unit(ValueUnit::Count)
            .gap(6.)
            .into_element(),
    ))
}

fn weekday_labels() -> Vec<String> {
    WEEKDAY_LABELS.iter().map(|s| (*s).to_string()).collect()
}
