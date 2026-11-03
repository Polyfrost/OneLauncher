use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Local, Months, NaiveDate};
use freya::prelude::*;

use oneclient_core::game::{DayPlaytime, PlaytimeStats, WEEKDAY_LABELS};

use crate::components::{
    BarChart, DateRange, DateRangePicker, IconType, Segment, SegmentedControl, ValueUnit,
};
use crate::i18n::{t, tf};
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
            .segment(Segment::new(PlaytimeMode::Hour).label(t("oneclient.analytics.playtime.hour")))
            .segment(Segment::new(PlaytimeMode::Day).label(t("oneclient.analytics.playtime.day")))
            .segment(
                Segment::new(PlaytimeMode::Month).label(t("oneclient.analytics.playtime.month")),
            )
            .into_element();

        let (title, subtitle, chart) = match m {
            PlaytimeMode::Hour => {
                let hour = self
                    .peak_hour
                    .map(format_hour)
                    .unwrap_or_else(|| "—".to_string());
                (
                    t("oneclient.analytics.playtime.by_hour.title"),
                    tf("oneclient.analytics.playtime.by_hour.subtitle", &[&hour]),
                    BarChart::new(self.per_hour.clone(), (0..24).map(format_hour).collect())
                        .readout_labels(
                            (0..24)
                                .map(|h| {
                                    tf(
                                        "oneclient.analytics.playtime.by_hour.readout",
                                        &[&format_hour(h)],
                                    )
                                })
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
                    .map(|i| weekday_full(i).to_string())
                    .unwrap_or_else(|| "—".to_string());
                (
                    t("oneclient.analytics.playtime.by_day.title"),
                    tf("oneclient.analytics.playtime.by_day.subtitle", &[&day]),
                    BarChart::new(self.per_weekday.clone(), weekday_labels())
                        .readout_labels((0..7).map(|i| weekday_all(i).to_string()).collect())
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
                    .map(|(m, _)| month_abbr(m.month0()).to_string())
                    .collect();
                let readout: Vec<String> = months
                    .iter()
                    .map(|(m, _)| format!("{} {}", month_abbr(m.month0()), m.year()))
                    .collect();
                let best = values
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, v)| **v)
                    .filter(|(_, v)| **v > 0)
                    .map(|(i, _)| i);
                let subtitle = match best {
                    Some(i) => tf("oneclient.analytics.playtime.by_month.best", &[&readout[i]]),
                    None => t("oneclient.analytics.no_sessions").to_string(),
                };
                (
                    t("oneclient.analytics.playtime.by_month.title"),
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
        let custom = use_state(|| Option::<DateRange>::None);

        let series = continuous_series(&self.daily);
        let (Some((first_day, _)), Some((last_day, _))) = (series.first(), series.last()) else {
            return chart_card(
                t("oneclient.analytics.daily.title"),
                t("oneclient.analytics.no_sessions").to_string(),
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
                    .segment(
                        Segment::new(Range::TwoWeeks)
                            .label(t("oneclient.analytics.daily.range.two_weeks")),
                    )
                    .segment(
                        Segment::new(Range::Month)
                            .label(t("oneclient.analytics.daily.range.month")),
                    )
                    .segment(
                        Segment::new(Range::Quarter)
                            .label(t("oneclient.analytics.daily.range.quarter")),
                    )
                    .into_element(),
            )
            .child(DateRangePicker::new(custom, (first_day, last_day)).height(30.))
            .into_element();

        chart_card(
            t("oneclient.analytics.daily.title"),
            subtitle,
            Some(nav),
            BarChart::new(values, labels)
                .unit(ValueUnit::Duration)
                .gap(3.)
                .into_element(),
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

    let (Some(first), Some(last)) = (totals.keys().min().copied(), totals.keys().max().copied())
    else {
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

fn month_abbr(month0: u32) -> &'static str {
    match month0 {
        0 => t("oneclient.analytics.month.jan"),
        1 => t("oneclient.analytics.month.feb"),
        2 => t("oneclient.analytics.month.mar"),
        3 => t("oneclient.analytics.month.apr"),
        4 => t("oneclient.analytics.month.may"),
        5 => t("oneclient.analytics.month.jun"),
        6 => t("oneclient.analytics.month.jul"),
        7 => t("oneclient.analytics.month.aug"),
        8 => t("oneclient.analytics.month.sep"),
        9 => t("oneclient.analytics.month.oct"),
        10 => t("oneclient.analytics.month.nov"),
        _ => t("oneclient.analytics.month.dec"),
    }
}

pub(super) fn distribution_card(session_secs: &[i64], force: bool) -> Option<Element> {
    if session_secs.len() < 2 && !force {
        return None;
    }

    const EDGES: [i64; 6] = [15 * 60, 30 * 60, 60 * 60, 2 * 3600, 4 * 3600, i64::MAX];

    let mut counts = [0i64; 6];
    for &s in session_secs {
        let idx = EDGES.iter().position(|edge| s < *edge).unwrap_or(5);
        counts[idx] += 1;
    }

    let labels: Vec<String> = [
        t("oneclient.analytics.session_length.under_15m"),
        t("oneclient.analytics.session_length.15m_30m"),
        t("oneclient.analytics.session_length.30m_60m"),
        t("oneclient.analytics.session_length.1h_2h"),
        t("oneclient.analytics.session_length.2h_4h"),
        t("oneclient.analytics.session_length.over_4h"),
    ]
    .iter()
    .map(|l| (*l).to_string())
    .collect();
    let peak = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| **c)
        .filter(|(_, c)| **c > 0)
        .map(|(i, _)| i);

    let subtitle = tf(
        "oneclient.analytics.session_length.subtitle",
        &[&session_secs.len()],
    );

    Some(chart_card(
        t("oneclient.analytics.session_length.title"),
        subtitle,
        None,
        BarChart::new(counts.to_vec(), labels)
            .highlight(peak)
            .unit(ValueUnit::Count)
            .gap(6.)
            .into_element(),
    ))
}

fn weekday_full(i: usize) -> &'static str {
    match i {
        0 => t("oneclient.analytics.weekday.monday"),
        1 => t("oneclient.analytics.weekday.tuesday"),
        2 => t("oneclient.analytics.weekday.wednesday"),
        3 => t("oneclient.analytics.weekday.thursday"),
        4 => t("oneclient.analytics.weekday.friday"),
        5 => t("oneclient.analytics.weekday.saturday"),
        _ => t("oneclient.analytics.weekday.sunday"),
    }
}

fn weekday_all(i: usize) -> &'static str {
    match i {
        0 => t("oneclient.analytics.weekday.all_mondays"),
        1 => t("oneclient.analytics.weekday.all_tuesdays"),
        2 => t("oneclient.analytics.weekday.all_wednesdays"),
        3 => t("oneclient.analytics.weekday.all_thursdays"),
        4 => t("oneclient.analytics.weekday.all_fridays"),
        5 => t("oneclient.analytics.weekday.all_saturdays"),
        _ => t("oneclient.analytics.weekday.all_sundays"),
    }
}

fn weekday_labels() -> Vec<String> {
    WEEKDAY_LABELS.iter().map(|s| (*s).to_string()).collect()
}
