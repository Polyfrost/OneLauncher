use freya::engine::prelude::{Paint, PaintStyle, PathBuilder, SkColor, SkRect};
use freya::prelude::*;

use crate::theme::colors;
use crate::utils::format_duration_hm;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueUnit {
    Duration,
    Count,
}

impl ValueUnit {
    pub fn format(self, value: i64) -> String {
        match self {
            ValueUnit::Duration => format_duration_hm(value),
            ValueUnit::Count => format!("{value}×"),
        }
    }
}

const DEFAULT_HEIGHT: f32 = 150.;
const Y_AXIS_WIDTH: f32 = 44.;

/// Slice colors, cycled when there are more slices than entries.
pub const SLICE_COLORS: [Color; 10] = [
    Color::from_rgb(43, 75, 255),
    Color::from_rgb(27, 217, 106),
    Color::from_rgb(241, 100, 54),
    Color::from_rgb(198, 120, 221),
    Color::from_rgb(97, 175, 239),
    Color::from_rgb(229, 192, 123),
    Color::from_rgb(35, 154, 96),
    Color::from_rgb(224, 108, 117),
    Color::from_rgb(120, 129, 141),
    Color::from_rgb(152, 195, 121),
];

pub fn slice_color(i: usize) -> Color {
    SLICE_COLORS[i % SLICE_COLORS.len()]
}

const PIE_SIZE: f32 = 190.;
/// Inner hole as a fraction of the radius, making it a donut.
const PIE_HOLE: f32 = 0.58;

/// A donut chart. Hovering a slice reports it via `on_hover` so callers can
/// sync an external legend; the hovered slice is also expanded slightly.
pub struct PieChart {
    values: Vec<i64>,
    labels: Vec<String>,
    unit: ValueUnit,
    size: f32,
    hovered: Option<State<Option<usize>>>,
}

impl PieChart {
    pub fn new(values: Vec<i64>, labels: Vec<String>) -> Self {
        Self {
            values,
            labels,
            unit: ValueUnit::Duration,
            size: PIE_SIZE,
            hovered: None,
        }
    }

    pub fn unit(mut self, unit: ValueUnit) -> Self {
        self.unit = unit;
        self
    }

    /// Shares the hovered slice index with the caller (e.g. to highlight a legend row).
    pub fn hovered(mut self, hovered: State<Option<usize>>) -> Self {
        self.hovered = Some(hovered);
        self
    }
}

impl PartialEq for PieChart {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
            && self.labels == other.labels
            && self.unit == other.unit
            && self.size == other.size
    }
}

impl Component for PieChart {
    fn render(&self) -> impl IntoElement {
        let internal = use_state(|| Option::<usize>::None);
        let mut hovered = self.hovered.unwrap_or(internal);
        let active = *hovered.read();

        let size = self.size;
        let total: i64 = self.values.iter().sum();
        let values = self.values.clone();
        let hit_values = self.values.clone();

        let render_cb = RenderCallback::new(move |ctx: &mut CanvasContext| {
            let (w, h) = (ctx.size.width, ctx.size.height);
            if w <= 0.0 || h <= 0.0 {
                return;
            }

            let cx = w * 0.5;
            let cy = h * 0.5;
            let radius = (w.min(h) * 0.5) - 6.0;
            if radius <= 0.0 {
                return;
            }

            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_style(PaintStyle::Fill);

            // With nothing recorded, draw an empty track so the ring is still there.
            if total <= 0 {
                paint.set_color(SkColor::from(colors::component_bg_hover()));
                ctx.canvas.draw_circle((cx, cy), radius - 5.0, &paint);
                paint.set_color(SkColor::from(colors::page_elevated()));
                ctx.canvas
                    .draw_circle((cx, cy), (radius - 5.0) * PIE_HOLE, &paint);
                return;
            }

            let mut start = -90.0_f32;
            for (i, &v) in values.iter().enumerate() {
                if v <= 0 {
                    continue;
                }
                let sweep = (v as f32 / total as f32) * 360.0;
                // Grow the hovered slice a touch, and dim the rest.
                let is_active = Some(i) == active;
                let r = if is_active { radius } else { radius - 5.0 };
                let color = slice_color(i);
                let color = if active.is_some() && !is_active {
                    color.with_a(120)
                } else {
                    color
                };
                paint.set_color(SkColor::from(color));

                let oval = SkRect::new(cx - r, cy - r, cx + r, cy + r);
                let mut builder = PathBuilder::new();
                builder.move_to((cx, cy));
                builder.arc_to(oval, start, sweep, false);
                builder.close();
                ctx.canvas.draw_path(&builder.detach(), &paint);

                start += sweep;
            }

            // Punch the donut hole using the card background.
            paint.set_color(SkColor::from(colors::page_elevated()));
            ctx.canvas
                .draw_circle((cx, cy), (radius - 5.0) * PIE_HOLE, &paint);
        });

        let (center_name, center_value) = match active {
            Some(i) if i < self.values.len() => (
                self.labels
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("#{}", i + 1)),
                self.unit.format(self.values[i]),
            ),
            _ => ("Total".to_string(), self.unit.format(total)),
        };

        rect()
            .width(Size::px(size))
            .height(Size::px(size))
            .on_pointer_move(move |e: Event<PointerEventData>| {
                let loc = e.element_location();
                let next = slice_at(&hit_values, loc.x as f32, loc.y as f32, size);
                if *hovered.peek() != next {
                    hovered.set(next);
                }
            })
            .on_pointer_leave(move |_| {
                if hovered.peek().is_some() {
                    hovered.set(None);
                }
            })
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .interactive(false)
                    // The canvas defaults to auto-sizing and has no children, so
                    // without an explicit size it measures 0x0 and draws nothing.
                    .child(canvas(render_cb).width(Size::fill()).height(Size::fill())),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .interactive(false)
                    // Sibling absolute rects on an equal layer paint in undefined
                    // order, so pin the readout above the canvas.
                    .layer(Layer::Relative(1))
                    .center()
                    // Keeps the readout inside the donut hole (Gaps is vertical, horizontal).
                    .padding(Gaps::new_symmetric(0., size * 0.25))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .cross_align(Alignment::Center)
                            .spacing(2.)
                            .child(
                                label()
                                    .text(center_value)
                                    .font_size(16.)
                                    .font_weight(FontWeight::BOLD)
                                    .max_lines(1)
                                    .width(Size::fill())
                                    .text_align(TextAlign::Center)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(center_name)
                                    .font_size(10.)
                                    .max_lines(2)
                                    .width(Size::fill())
                                    .text_align(TextAlign::Center)
                                    .color(colors::fg_secondary()),
                            ),
                    ),
            )
    }
}

/// Maps a pointer position inside the chart box to a slice index.
fn slice_at(values: &[i64], x: f32, y: f32, size: f32) -> Option<usize> {
    let total: i64 = values.iter().sum();
    if total <= 0 {
        return None;
    }

    let cx = size * 0.5;
    let cy = size * 0.5;
    let radius = (size * 0.5) - 6.0;
    let (dx, dy) = (x - cx, y - cy);
    let dist = (dx * dx + dy * dy).sqrt();
    if dist > radius || dist < (radius - 5.0) * PIE_HOLE {
        return None;
    }

    // Angle measured clockwise from 12 o'clock, matching the drawing order.
    let mut angle = dy.atan2(dx).to_degrees() + 90.0;
    if angle < 0.0 {
        angle += 360.0;
    }

    let mut start = 0.0_f32;
    for (i, &v) in values.iter().enumerate() {
        if v <= 0 {
            continue;
        }
        let sweep = (v as f32 / total as f32) * 360.0;
        if angle >= start && angle < start + sweep {
            return Some(i);
        }
        start += sweep;
    }
    None
}

#[derive(PartialEq)]
pub struct BarChart {
    values: Vec<i64>,
    labels: Vec<String>,
    highlight: Option<usize>,
    unit: ValueUnit,
    height: f32,
    gap: f32,
}

impl BarChart {
    pub fn new(values: Vec<i64>, labels: Vec<String>) -> Self {
        Self {
            values,
            labels,
            highlight: None,
            unit: ValueUnit::Duration,
            height: DEFAULT_HEIGHT,
            gap: 4.,
        }
    }

    pub fn highlight(mut self, highlight: Option<usize>) -> Self {
        self.highlight = highlight;
        self
    }

    pub fn unit(mut self, unit: ValueUnit) -> Self {
        self.unit = unit;
        self
    }

    #[allow(dead_code)]
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    fn readout_name(&self, i: usize) -> String {
        self.labels
            .get(i)
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("#{}", i + 1))
    }
}

impl Component for BarChart {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| Option::<usize>::None);
        let active = *hovered.read();
        let mut plot_width = use_state(|| 0f32);

        let unit = self.unit;
        let height = self.height;
        let max = self.values.iter().copied().max().unwrap_or(0);

        let focus = active.or(self.highlight);
        let (readout_label, readout_value) = match focus {
            Some(i) if i < self.values.len() => (self.readout_name(i), unit.format(self.values[i])),
            _ => {
                let total: i64 = self.values.iter().sum();
                ("Total".to_string(), unit.format(total))
            }
        };

        let mut bars = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::fill())
            .cross_align(Alignment::End)
            .spacing(self.gap)
            .on_pointer_leave(move |_| *hovered.write() = None);

        for (i, &v) in self.values.iter().enumerate() {
            let frac = if max > 0 { v as f32 / max as f32 } else { 0.0 };
            let h = if v > 0 { (frac * height).max(4.0) } else { 0.0 };
            let is_active = Some(i) == focus;
            let color = if is_active {
                colors::brand()
            } else if active.is_some() {
                colors::component_bg_hover().with_a(150)
            } else {
                colors::component_bg_hover()
            };

            bars = bars.child(
                rect()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .main_align(Alignment::End)
                    .on_pointer_enter(move |_| {
                        *hovered.write() = Some(i);
                        Cursor::set(CursorIcon::Pointer);
                    })
                    .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(h))
                            .corner_radius(CornerRadius {
                                top_left: 4.,
                                top_right: 4.,
                                bottom_right: 0.,
                                bottom_left: 0.,
                                smoothing: 0.,
                            })
                            .background(color),
                    ),
            );
        }

        let stride = label_stride(&self.labels, *plot_width.read(), self.gap);
        let mut label_row = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(14.))
            .spacing(self.gap);
        for (i, text) in self.labels.iter().enumerate() {
            let shown = i % stride == 0;
            label_row = label_row.child(
                rect()
                    .width(Size::flex(1.0))
                    .main_align(Alignment::Center)
                    .child(
                        label()
                            .text(if shown { text.clone() } else { String::new() })
                            .font_size(10.)
                            .max_lines(1)
                            .width(Size::fill())
                            .text_align(TextAlign::Center)
                            .color(colors::fg_secondary()),
                    ),
            );
        }

        let plot = rect()
            .width(Size::fill())
            .height(Size::px(height))
            .overflow(Overflow::Clip)
            .child(gridlines(height))
            .child(bars);

        let plot_row = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .spacing(6.)
            .child(y_axis(max, unit, height))
            .child(
                rect()
                    .width(Size::flex(1.0))
                    .on_sized(move |e: Event<SizedEventData>| {
                        let w = e.area.width();
                        if (*plot_width.peek() - w).abs() > 0.5 {
                            plot_width.set(w);
                        }
                    })
                    .child(plot),
            );

        let labels_row = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .spacing(6.)
            .child(rect().width(Size::px(Y_AXIS_WIDTH)))
            .child(rect().width(Size::flex(1.0)).child(label_row));

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .child(readout(readout_label, readout_value))
            .child(plot_row)
            .child(labels_row)
    }
}

/// How many bars to skip between shown labels so text never overlaps.
/// Derived from the measured plot width: a wider chart (e.g. maximised window)
/// fits more labels and returns a smaller stride.
fn label_stride(labels: &[String], plot_width: f32, gap: f32) -> usize {
    let n = labels.len();
    if n == 0 {
        return 1;
    }
    // Before the first measurement, thin conservatively to avoid an overlap flash.
    if plot_width <= 0.0 {
        return (n / 12).max(1);
    }

    let max_chars = labels
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0)
        .max(1);
    // ~6.2px per char at font_size 10, plus breathing room between labels.
    let slot_px = max_chars as f32 * 6.2 + gap.max(8.0) + 8.0;
    let max_labels = (plot_width / slot_px).floor().max(1.0) as usize;

    if n <= max_labels {
        1
    } else {
        n.div_ceil(max_labels)
    }
}

fn gridlines(height: f32) -> Element {
    let mut overlay = rect()
        .width(Size::fill())
        .height(Size::fill())
        .position(Position::new_absolute().top(0.).left(0.))
        .interactive(false);

    for frac in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        let top = ((1.0 - frac) * height - if frac == 0.0 { 1.0 } else { 0.0 }).max(0.);
        overlay = overlay.child(
            rect()
                .width(Size::fill())
                .height(Size::px(1.))
                .position(Position::new_absolute().left(0.).top(top))
                .background(colors::component_border().with_a(110)),
        );
    }
    overlay.into_element()
}

fn y_axis(max: i64, unit: ValueUnit, height: f32) -> Element {
    let fmt = |v: i64| match unit {
        ValueUnit::Duration => format_duration_hm(v),
        ValueUnit::Count => v.to_string(),
    };

    let tick = |text: String| {
        label()
            .text(text)
            .font_size(9.)
            .max_lines(1)
            .width(Size::fill())
            .text_align(TextAlign::Right)
            .color(colors::fg_secondary().with_a(160))
    };

    rect()
        .vertical()
        .width(Size::px(Y_AXIS_WIDTH))
        .height(Size::px(height))
        .main_align(Alignment::SpaceBetween)
        .child(tick(fmt(max)))
        .child(tick(fmt(max / 2)))
        .child(tick(fmt(0)))
        .into_element()
}

fn readout(name: String, value: String) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(8.)
        .height(Size::px(20.))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(6.)
                .padding(Gaps::new_symmetric(3., 8.))
                .corner_radius(CornerRadius::new_all(6.))
                .background(colors::component_bg())
                .border(crate::ui::border_all_color(1., colors::component_border()))
                .child(
                    rect()
                        .width(Size::px(8.))
                        .height(Size::px(8.))
                        .corner_radius(CornerRadius::new_all(2.))
                        .background(colors::brand()),
                )
                .child(
                    label()
                        .text(name)
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(value)
                        .font_size(11.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .into_element()
}
