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

#[derive(PartialEq)]
pub struct BarChart {
    values: Vec<i64>,
    labels: Vec<String>,
    readout_labels: Vec<String>,
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
            readout_labels: Vec::new(),
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

    pub fn readout_labels(mut self, labels: Vec<String>) -> Self {
        self.readout_labels = labels;
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
        self.readout_labels
            .get(i)
            .or_else(|| self.labels.get(i))
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("#{}", i + 1))
    }
}

impl Component for BarChart {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| Option::<usize>::None);
        let active = *hovered.read();

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

        let mut label_row = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(14.))
            .spacing(self.gap);
        for text in &self.labels {
            label_row = label_row.child(
                rect()
                    .width(Size::flex(1.0))
                    .main_align(Alignment::Center)
                    .child(
                        label()
                            .text(text.clone())
                            .font_size(10.)
                            .max_lines(1)
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
            .child(rect().width(Size::flex(1.0)).child(plot));

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
