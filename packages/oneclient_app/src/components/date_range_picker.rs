use chrono::{Datelike, Days, Months, NaiveDate};
use freya::prelude::*;

use crate::components::{Icon, IconType, OverlayPopup};
use crate::theme::colors;
use crate::ui::{border_all_color, clamp_to_window};
use crate::utils::format_day;

const CELL: f32 = 30.;
const CELL_GAP: f32 = 2.;
const WEEKS: u32 = 6;
const POPUP_WIDTH: f32 = 7. * CELL + 6. * CELL_GAP + 24. + 2.;
const POPUP_OFFSET: f32 = 4.;

pub type DateRange = (NaiveDate, NaiveDate);

#[derive(PartialEq)]
pub struct DateRangePicker {
    range: State<Option<DateRange>>,
    bounds: DateRange,
    height: f32,
}

impl DateRangePicker {
    pub fn new(range: State<Option<DateRange>>, bounds: DateRange) -> Self {
        Self {
            range,
            bounds,
            height: 30.,
        }
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl Component for DateRangePicker {
    fn render(&self) -> impl IntoElement {
        let mut range = self.range;
        let (min, max) = self.bounds;

        let mut open = use_state(|| false);
        let mut anchor = use_state(|| Option::<NaiveDate>::None);
        let mut month = use_state(move || first_of_month(max));
        let mut hovering = use_state(|| false);
        let mut trigger = use_state(Area::zero);
        let mut panel_height = use_state(|| 0f32);

        let is_open = open();

        let selected = *range.read();
        let pending = *anchor.read();
        let shown = *month.read();

        let can_prev = shown > first_of_month(min);
        let can_next = shown < first_of_month(max);

        let pick = move |day: NaiveDate| {
            let started = *anchor.peek();
            match started {
                Some(start) => {
                    range.set(Some((start.min(day), start.max(day))));
                    anchor.set(None);
                    open.set(false);
                }
                None => anchor.set(Some(day)),
            }
        };

        let trigger_text = match selected {
            Some((a, b)) if a == b => format_day(a),
            Some((a, b)) => format!("{} – {}", format_day(a), format_day(b)),
            None => "Pick dates".to_string(),
        };

        rect()
            .child(
                rect()
                    .horizontal()
                    .height(Size::px(self.height))
                    .cross_align(Alignment::Center)
                    .spacing(6.)
                    .padding(Gaps::new_symmetric(0., 10.))
                    .corner_radius(CornerRadius::new_all(9.))
                    .background(if hovering() {
                        colors::component_bg_hover()
                    } else {
                        colors::component_bg()
                    })
                    .border(border_all_color(
                        1.,
                        if selected.is_some() {
                            colors::brand()
                        } else {
                            colors::component_border()
                        },
                    ))
                    .cursor(CursorIcon::Pointer)
                    .a11y_role(AccessibilityRole::Button)
                    .on_pointer_enter(move |_| hovering.set(true))
                    .on_pointer_leave(move |_| hovering.set(false))
                    .on_sized(move |e: Event<SizedEventData>| {
                        trigger.set_if_modified(e.area);
                    })
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        anchor.set_if_modified(None);
                        open.toggle();
                    })
                    .child(
                        Icon::new(IconType::Calendar)
                            .size(13.)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        label()
                            .text(trigger_text)
                            .font_size(12.)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    ),
            )
            .maybe_child(is_open.then(|| {
                let area = *trigger.read();
                let height = *panel_height.read();
                let (x, y) = clamp_to_window(
                    area.max_x() - POPUP_WIDTH,
                    area.max_y() + POPUP_OFFSET,
                    POPUP_WIDTH,
                    height,
                );

                let panel = rect()
                    .width(Size::px(POPUP_WIDTH))
                    .vertical()
                    .spacing(8.)
                    .padding(12.)
                    .corner_radius(CornerRadius::new_all(10.))
                    .border(border_all_color(1., colors::component_border()))
                    .background(colors::page_elevated())
                    .on_sized(move |e: Event<SizedEventData>| {
                        panel_height.set_if_modified(e.area.height());
                    })
                    .opacity(if height > 0. { 1. } else { 0. })
                    .child(month_header(shown, can_prev, can_next, move |step| {
                        let next = if step < 0 {
                            shown.checked_sub_months(Months::new(1))
                        } else {
                            shown.checked_add_months(Months::new(1))
                        };
                        if let Some(next) = next {
                            month.set(next.clamp(first_of_month(min), first_of_month(max)));
                        }
                    }))
                    .child(weekday_header())
                    .child(month_grid(shown, min, max, selected, pending, pick))
                    .child(footer(selected.is_some(), pending, move || {
                        range.set(None);
                        anchor.set(None);
                        open.set(false);
                    }));

                OverlayPopup::new()
                    .backdrop(false)
                    .position(Position::new_global().top(y).left(x))
                    .on_close(move |()| {
                        anchor.set(None);
                        open.set(false);
                    })
                    .child(panel.into_element())
                    .into_element()
            }))
    }
}

fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

fn month_header(
    shown: NaiveDate,
    can_prev: bool,
    can_next: bool,
    step: impl FnMut(i32) + Clone + 'static,
) -> Element {
    let back = step.clone();
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .child(arrow(IconType::ChevronsLeft, can_prev, back, -1))
        .child(
            label()
                .text(format!("{} {}", month_name(shown.month0()), shown.year()))
                .font_size(13.)
                .font_weight(FontWeight::SEMI_BOLD)
                .width(Size::flex(1.0))
                .text_align(TextAlign::Center)
                .color(colors::fg_primary()),
        )
        .child(arrow(IconType::ChevronsRight, can_next, step, 1))
        .into_element()
}

fn arrow(
    icon: IconType,
    enabled: bool,
    mut step: impl FnMut(i32) + 'static,
    delta: i32,
) -> Element {
    rect()
        .width(Size::px(24.))
        .height(Size::px(24.))
        .center()
        .corner_radius(CornerRadius::new_all(6.))
        .background(colors::component_bg())
        .maybe(enabled, |el| {
            el.cursor(CursorIcon::Pointer)
                .on_press(move |_| step(delta))
        })
        .child(Icon::new(icon).size(12.).color(if enabled {
            colors::fg_secondary()
        } else {
            colors::fg_secondary().with_a(70)
        }))
        .into_element()
}

fn weekday_header() -> Element {
    let mut row = rect().horizontal().spacing(CELL_GAP);
    for day in ["M", "T", "W", "T", "F", "S", "S"] {
        row = row.child(
            rect().width(Size::px(CELL)).center().child(
                label()
                    .text(day.to_string())
                    .font_size(10.)
                    .color(colors::fg_secondary()),
            ),
        );
    }
    row.into_element()
}

fn month_grid(
    shown: NaiveDate,
    min: NaiveDate,
    max: NaiveDate,
    selected: Option<DateRange>,
    pending: Option<NaiveDate>,
    pick: impl FnMut(NaiveDate) + Clone + 'static,
) -> Element {
    let lead = shown.weekday().num_days_from_monday();
    let first_cell = shown
        .checked_sub_days(Days::new(u64::from(lead)))
        .unwrap_or(shown);

    let mut grid = rect().vertical().spacing(CELL_GAP);
    for week in 0..WEEKS {
        let mut row = rect().horizontal().spacing(CELL_GAP);
        for weekday in 0..7u32 {
            let offset = u64::from(week * 7 + weekday);
            let Some(day) = first_cell.checked_add_days(Days::new(offset)) else {
                continue;
            };
            row = row.child(DayCell {
                day,
                outside: day.month0() != shown.month0(),
                enabled: (min..=max).contains(&day),
                in_range: selected.is_some_and(|(a, b)| (a..=b).contains(&day)),
                edge: selected.is_some_and(|(a, b)| day == a || day == b)
                    || pending == Some(day),
                pick: pick.clone(),
            });
        }
        grid = grid.child(row);
    }
    grid.into_element()
}

struct DayCell<F: FnMut(NaiveDate) + Clone + 'static> {
    day: NaiveDate,
    outside: bool,
    enabled: bool,
    in_range: bool,
    edge: bool,
    pick: F,
}

impl<F: FnMut(NaiveDate) + Clone + 'static> PartialEq for DayCell<F> {
    fn eq(&self, other: &Self) -> bool {
        self.day == other.day
            && self.outside == other.outside
            && self.enabled == other.enabled
            && self.in_range == other.in_range
            && self.edge == other.edge
    }
}

impl<F: FnMut(NaiveDate) + Clone + 'static> Component for DayCell<F> {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        let day = self.day;
        let enabled = self.enabled;
        let mut pick = self.pick.clone();

        let background = if self.edge {
            colors::brand()
        } else if self.in_range {
            colors::brand().with_a(50)
        } else if hovering() && enabled {
            colors::component_bg_hover()
        } else {
            Color::TRANSPARENT
        };

        let color = if self.edge {
            colors::fg_primary()
        } else if !enabled || self.outside {
            colors::fg_secondary().with_a(90)
        } else {
            colors::fg_primary()
        };

        rect()
            .width(Size::px(CELL))
            .height(Size::px(CELL - 2.))
            .center()
            .corner_radius(CornerRadius::new_all(6.))
            .background(background)
            .maybe(enabled, |el| {
                el.cursor(CursorIcon::Pointer)
                    .on_pointer_enter(move |_| hovering.set(true))
                    .on_pointer_leave(move |_| hovering.set(false))
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        pick(day);
                    })
            })
            .child(
                label()
                    .text(day.day().to_string())
                    .font_size(11.)
                    .color(color),
            )
    }
}

fn footer(has_range: bool, pending: Option<NaiveDate>, mut clear: impl FnMut() + 'static) -> Element {
    let hint = match pending {
        Some(day) => format!("From {} — pick an end date", format_day(day)),
        None => "Click a start date, then an end date".to_string(),
    };

    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(
            label()
                .text(hint)
                .font_size(10.)
                .max_lines(2)
                .width(Size::flex(1.0))
                .color(colors::fg_secondary()),
        )
        .maybe_child(has_range.then(|| {
            rect()
                .padding(Gaps::new_symmetric(4., 8.))
                .corner_radius(CornerRadius::new_all(6.))
                .background(colors::component_bg())
                .cursor(CursorIcon::Pointer)
                .a11y_role(AccessibilityRole::Button)
                .on_press(move |e: Event<PressEventData>| {
                    e.stop_propagation();
                    clear();
                })
                .child(
                    label()
                        .text("Reset")
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                )
                .into_element()
        }))
        .into_element()
}

fn month_name(month0: u32) -> &'static str {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    MONTHS[(month0 as usize).min(11)]
}
