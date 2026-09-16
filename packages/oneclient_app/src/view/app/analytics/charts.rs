use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Local, Months, NaiveDate};
use freya::prelude::*;

use oneclient_core::game::{DayPlaytime, PlaytimeStats, WEEKDAY_LABELS};

use crate::components::{
    BarChart, DateRange, DateRangePicker, IconType, Segment, SegmentedControl, ValueUnit,
};
use crate::utils::{format_day, format_duration, format_hour, parse_day};

use super::{chart_card, nav_button};

#[derive(Clone, Copy, PartialEq)]
enum PlaytimeMode {
    Hour,
    Day,
    Month,
}

#[derive(PartialEq)]
pub(super) struct PlaytimeChart {
    per_weekday: Vec<i64>,
    per_hour: Vec<i64>,
    peak_weekday: Option<usize>,
    peak_hour: Option<usize>,
    daily: Vec<DayPlaytime>,
}

impl PlaytimeChart {
    pub(super) fn from_stats(stats: &PlaytimeStats) -> Self {
        Self {
            per_weekday: stats.per_weekday.to_vec(),
            per_hour: stats.per_hour.to_vec(),
            peak_weekday: stats.peak_weekday,
            peak_hour: stats.peak_hour,
            daily: stats.daily.clone(),
        }
    }
}

impl Component for PlaytimeChart {
    fn render(&self) -> impl IntoElement {
        let mode = use_state(|| PlaytimeMode::Day);
        let m = *mode.read();

        let control = SegmentedControl::new(mode)
            .height(30.)
            .segment(Segment::new(PlaytimeMode::Hour).label("Hour"))
            .segment(Segment::new(PlaytimeMode::Day).label("Day"))
            .segment(Segment::new(PlaytimeMode::Month).label("Month"))
            .into_element();

        let (title, subtitle, chart) = match m {
            PlaytimeMode::Hour => {
                let hour = self
                    .peak_hour
                    .map(format_hour)
                    .unwrap_or_else(|| "—".to_string());
                (
                    "Playtime by hour",
                    format!("Every day's {hour} hour, added up"),
                    BarChart::new(self.per_hour.clone(), (0..24).map(format_hour).collect())
                        .readout_labels(
                            (0..24)
                                .map(|h| format!("All {} hours", format_hour(h)))
                                .collect(),
                        )
                        .highlight(self.peak_hour)
                        .unit(ValueUnit::Duration)
                        .gap(3.)
                        .into_element(),
                )
            }
            PlaytimeMode::Day => {
                let day = self
                    .peak_weekday
                    .map(|i| WEEKDAY_FULL[i].to_string())
                    .unwrap_or_else(|| "—".to_string());
                (
                    "Playtime by day",
                    format!("Every {day} you have played, added up"),
                    BarChart::new(self.per_weekday.clone(), weekday_labels())
                        .readout_labels(
                            WEEKDAY_FULL.iter().map(|d| format!("All {d}s")).collect(),
                        )
                        .highlight(self.peak_weekday)
                        .unit(ValueUnit::Duration)
                        .gap(6.)
                        .into_element(),
                )
            }
            PlaytimeMode::Month => {
                let months = monthly_series(&self.daily);
                let values: Vec<i64> = months.iter().map(|(_, s)| *s).collect();
                let labels: Vec<String> = months
                    .iter()
                    .map(|(m, _)| MONTH_ABBR[m.month0() as usize].to_string())
                    .collect();
                let readout: Vec<String> = months
                    .iter()
                    .map(|(m, _)| format!("{} {}", MONTH_ABBR[m.month0() as usize], m.year()))
                    .collect();
                let best = values
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, v)| **v)
                    .filter(|(_, v)| **v > 0)
                    .map(|(i, _)| i);
                let subtitle = match best {
                    Some(i) => format!("Best month so far: {}", readout[i]),
                    None => "No sessions recorded yet".to_string(),
                };
                (
                    "Playtime by month",
                    subtitle,
                    BarChart::new(values, labels)
                        .readout_labels(readout)
                        .highlight(best)
                        .unit(ValueUnit::Duration)
                        .gap(4.)
                        .into_element(),
                )
            }
        };

        chart_card(title, subtitle, Some(control), chart)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Range {
    TwoWeeks,
    Month,
    Quarter,
}

const COMPACT_CARD_WIDTH_PX: f32 = 480.;

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
        let mut range = use_state(|| Range::TwoWeeks);
        let mut offset = use_state(|| 0usize);
        let custom = use_state(|| Option::<DateRange>::None);

        let series = continuous_series(&self.daily);
        let (Some((first_day, _)), Some((last_day, _))) = (series.first(), series.last()) else {
            return chart_card(
                "Daily playtime",
                "No sessions recorded yet".to_string(),
                None,
                rect().height(Size::px(120.)).into_element(),
            );
        };
        let (first_day, last_day) = (*first_day, *last_day);

        let total = series.len();
        let picked = *custom.read();

        let (start, end) = match picked {
            Some((from, to)) => (
                day_index(first_day, from).min(total),
                (day_index(first_day, to) + 1).min(total),
            ),
            None => {
                let window = range.read().days();
                let max_offset = total.saturating_sub(1) / window;
                let off = (*offset.read()).min(max_offset);
                let end = total.saturating_sub(off * window);
                (end.saturating_sub(window), end)
            }
        };
        let slice = &series[start.min(end)..end];

        let values: Vec<i64> = slice.iter().map(|(_, s)| *s).collect();
        let labels: Vec<String> = slice.iter().map(|(d, _)| format_day(*d)).collect();

        let win_total: i64 = values.iter().sum();
        let subtitle = match (slice.first(), slice.last()) {
            (Some((a, _)), Some((b, _))) => {
                format!(
                    "{} · {} – {}",
                    format_duration(win_total),
                    format_day(*a),
                    format_day(*b)
                )
            }
            _ => format_duration(win_total),
        };

        let paging = picked.is_none();
        let window = range.read().days();
        let max_offset = total.saturating_sub(1) / window;
        let off = (*offset.read()).min(max_offset);
        let can_older = paging && off < max_offset;
        let can_newer = paging && off > 0;

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
                    .disabled(!paging)
                    .segment(Segment::new(Range::TwoWeeks).label("2W"))
                    .segment(Segment::new(Range::Month).label("1M"))
                    .segment(Segment::new(Range::Quarter).label("3M"))
                    .into_element(),
            )
            .child(DateRangePicker::new(custom, (first_day, last_day)).height(30.))
            .into_element();

        measured_card(
            card_width,
            chart_card(
                "Daily playtime",
                subtitle,
                Some(nav),
                BarChart::new(values, labels)
                    .unit(ValueUnit::Duration)
                    .gap(3.)
                    .into_element(),
            ),
        )
    }
}

fn day_index(first: NaiveDate, day: NaiveDate) -> usize {
    (day - first).num_days().max(0) as usize
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

fn monthly_series(daily: &[DayPlaytime]) -> Vec<(NaiveDate, i64)> {
    let mut totals: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    for day in daily {
        let Some(date) = parse_day(&day.date) else {
            continue;
        };
        let Some(month) = date.with_day(1) else {
            continue;
        };
        *totals.entry(month).or_insert(0) += day.secs;
    }

    let (Some(first), Some(last)) = (
        totals.keys().min().copied(),
        totals.keys().max().copied(),
    ) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut cur = first;
    while cur <= last {
        out.push((cur, totals.get(&cur).copied().unwrap_or(0)));
        cur = match cur.checked_add_months(Months::new(1)) {
            Some(next) => next,
            None => break,
        };
    }
    out
}

const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

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

const WEEKDAY_FULL: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

fn weekday_labels() -> Vec<String> {
    WEEKDAY_LABELS.iter().map(|s| (*s).to_string()).collect()
}
