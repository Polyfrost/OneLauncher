use freya::prelude::*;

use crate::components::tooltip::use_tooltip_anchor;
use crate::components::{Icon, IconType, TooltipPlacement};
use crate::theme::{colors, glass, rounded, text_size};
use crate::ui::border_all_color;

#[derive(PartialEq, Clone)]
pub struct Segment<T> {
    value: T,
    icon: Option<IconType>,
    label: Option<String>,
    disabled: bool,
    tooltip: Option<Box<str>>,
}

impl<T> Segment<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            icon: None,
            label: None,
            disabled: false,
            tooltip: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<Box<str>>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn icon(mut self, icon: IconType) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn label(mut self, text: impl Into<String>) -> Self {
        self.label = Some(text.into());
        self
    }
}

#[derive(PartialEq)]
pub struct SegmentedControl<T: Copy + PartialEq + 'static> {
    selected: State<T>,
    segments: Vec<Segment<T>>,
    height: f32,
    icon_size: Option<f32>,
    tint_icons: bool,
    equal_width: Option<f32>,
    disabled: bool,
    static_indicator: bool,
}

impl<T: Copy + PartialEq + 'static> SegmentedControl<T> {
    pub fn new(selected: State<T>) -> Self {
        Self {
            selected,
            segments: Vec::new(),
            height: 36.,
            icon_size: None,
            tint_icons: true,
            equal_width: None,
            disabled: false,
            static_indicator: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn segment(mut self, segment: Segment<T>) -> Self {
        self.segments.push(segment);
        self
    }

    pub fn segments(mut self, segments: impl IntoIterator<Item = Segment<T>>) -> Self {
        self.segments.extend(segments);
        self
    }

    #[allow(dead_code)]
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn icon_size(mut self, size: f32) -> Self {
        self.icon_size = Some(size);
        self
    }

    pub fn no_tint(mut self) -> Self {
        self.tint_icons = false;
        self
    }

    pub fn equal_width(mut self, width: f32) -> Self {
        self.equal_width = Some(width);
        self
    }

    pub fn static_indicator(mut self) -> Self {
        self.static_indicator = true;
        self
    }
}

impl<T: Copy + PartialEq + 'static> Component for SegmentedControl<T> {
    fn render(&self) -> impl IntoElement {
        let active = *self.selected.read();
        let selected = self.selected;
        let icon_size = self.icon_size;
        let tint = self.tint_icons;
        let equal_width = self.equal_width;
        let disabled = self.disabled;
        let liquid = colors::active_theme().liquid_glass && !self.static_indicator;
        let track = glass::use_pill_track(self.segments.len());
        let index = self.segments.iter().position(|seg| seg.value == active);
        let draggable = liquid
            && !disabled
            && self
                .segments
                .iter()
                .filter(|seg| !seg.disabled)
                .nth(1)
                .is_some();

        rect()
            .horizontal()
            .height(Size::px(self.height))
            .cross_align(Alignment::Center)
            .spacing(2.)
            .padding(Gaps::new_all(3.))
            .corner_radius(if liquid {
                CornerRadius::new_all(self.height / 2.)
            } else {
                rounded(9.)
            })
            // In PolyGlass the lens owns the only contour; an outlined track would double it
            .background(if liquid {
                glass::track_fill()
            } else {
                colors::component_bg()
            })
            .maybe(!liquid, |el| {
                el.border(border_all_color(1., colors::component_border()))
            })
            .maybe(draggable, |el| {
                let choices: Vec<_> = self
                    .segments
                    .iter()
                    .map(|seg| (!disabled && !seg.disabled).then_some(seg.value))
                    .collect();
                let mut selected = selected;
                track.events(el, move |i| {
                    if let Some(Some(value)) = choices.get(i) {
                        selected.set_if_modified(*value);
                    }
                })
            })
            .maybe_child(liquid.then(|| track.indicator(index)))
            .children(self.segments.iter().enumerate().map(move |(index, seg)| {
                SegmentButton {
                    selected,
                    value: seg.value,
                    is_selected: seg.value == active,
                    icon: seg.icon,
                    label: seg.label.clone(),
                    icon_size,
                    tint,
                    equal_width,
                    disabled: disabled || seg.disabled,
                    tooltip: seg.tooltip.clone(),
                    pill: liquid.then_some((track, index)),
                    draggable,
                }
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct SegmentButton<T: Copy + PartialEq + 'static> {
    draggable: bool,
    pill: Option<(glass::PillTrack, usize)>,
    selected: State<T>,
    value: T,
    is_selected: bool,
    icon: Option<IconType>,
    label: Option<String>,
    icon_size: Option<f32>,
    tint: bool,
    equal_width: Option<f32>,
    disabled: bool,
    tooltip: Option<Box<str>>,
}

impl<T: Copy + PartialEq + 'static> Component for SegmentButton<T> {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let tooltip = use_tooltip_anchor(self.tooltip.clone(), TooltipPlacement::default());

        let mut selected = self.selected;
        let value = self.value;
        let is_selected = self.is_selected;
        let disabled = self.disabled;
        let equal_width = self.equal_width;
        let liquid = self.pill.is_some();
        let draggable = self.draggable;
        let focused = if liquid {
            focus() == Focus::Keyboard
        } else {
            focus().is_focused()
        };

        // The label under a dragged lens takes the selected look as the lens arrives
        let lit = self.pill.map_or(is_selected, |(track, index)| {
            track.highlights(index, is_selected)
        });

        let content_color = if disabled {
            colors::fg_secondary().with_a(110)
        } else if lit {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        let rect = rect()
            .horizontal()
            .height(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(6.)
            .corner_radius(if liquid {
                CornerRadius::new_all(128.)
            } else {
                rounded(6.)
            })
            .a11y_id(a11y_id)
            .a11y_focusable(!disabled)
            .a11y_role(AccessibilityRole::Button)
            .map(
                self.tooltip.as_deref().filter(|_| self.label.is_none()),
                |el, name| el.a11y_alt(name),
            )
            .map(equal_width, |el, w| {
                el.width(Size::px(w)).main_align(Alignment::Center)
            })
            .maybe(equal_width.is_none(), |el| {
                el.padding(Gaps::new_symmetric(0., 12.))
            })
            .background(if is_selected && !liquid {
                colors::component_bg_pressed()
            } else {
                Color::TRANSPARENT
            })
            .maybe(focused && !disabled, |el| {
                el.border(border_all_color(1., colors::brand()))
            })
            .maybe(!disabled, |el| {
                el.cursor(CursorIcon::Pointer)
                    .on_press(move |event: Event<PressEventData>| {
                        if !draggable
                            || !matches!(
                                event.data(),
                                PressEventData::Mouse(_) | PressEventData::Touch(_)
                            )
                        {
                            selected.set_if_modified(value);
                        }
                    })
            })
            .map(self.pill, |el, (track, index)| {
                track.option(el, index, draggable && !disabled)
            })
            .maybe_child(self.icon.map(|icon| {
                let mut ic = Icon::new(icon);
                if let Some(size) = self.icon_size {
                    ic = ic.size(size);
                }
                if self.tint {
                    ic = ic.color(content_color);
                }
                ic.into_element()
            }))
            .maybe_child(self.label.clone().map(|text| {
                label()
                    .text(text)
                    .font_size(text_size(13.))
                    .font_weight(if lit {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(content_color)
                    .into_element()
            }));
        tooltip.attach(rect)
    }
}
