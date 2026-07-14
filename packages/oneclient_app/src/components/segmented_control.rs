use freya::prelude::*;

use crate::components::{Icon, IconType};
use crate::theme::colors;
use crate::ui::border_all_color;

#[derive(PartialEq, Clone)]
pub struct Segment<T> {
    value: T,
    icon: Option<IconType>,
    label: Option<String>,
}

impl<T> Segment<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            icon: None,
            label: None,
        }
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
}

impl<T: Copy + PartialEq + 'static> Component for SegmentedControl<T> {
    fn render(&self) -> impl IntoElement {
        let active = *self.selected.read();
        let selected = self.selected;
        let icon_size = self.icon_size;
        let tint = self.tint_icons;
        let equal_width = self.equal_width;
        let disabled = self.disabled;

        rect()
            .horizontal()
            .height(Size::px(self.height))
            .cross_align(Alignment::Center)
            .spacing(2.)
            .padding(Gaps::new_all(3.))
            .corner_radius(CornerRadius::new_all(9.))
            .background(colors::component_bg())
            .border(border_all_color(1., colors::component_border()))
            .children(self.segments.iter().map(move |seg| {
                SegmentButton {
                    selected,
                    value: seg.value,
                    is_selected: seg.value == active,
                    icon: seg.icon,
                    label: seg.label.clone(),
                    icon_size,
                    tint,
                    equal_width,
                    disabled,
                }
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct SegmentButton<T: Copy + PartialEq + 'static> {
    selected: State<T>,
    value: T,
    is_selected: bool,
    icon: Option<IconType>,
    label: Option<String>,
    icon_size: Option<f32>,
    tint: bool,
    equal_width: Option<f32>,
    disabled: bool,
}

impl<T: Copy + PartialEq + 'static> Component for SegmentButton<T> {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let mut selected = self.selected;
        let value = self.value;
        let is_selected = self.is_selected;
        let disabled = self.disabled;
        let equal_width = self.equal_width;
        let focused = focus().is_focused();

        let content_color = if disabled {
            colors::fg_secondary().with_a(110)
        } else if is_selected {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        rect()
            .horizontal()
            .height(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(6.)
            .corner_radius(CornerRadius::new_all(6.))
            .a11y_id(a11y_id)
            .a11y_focusable(!disabled)
            .a11y_role(AccessibilityRole::Button)
            .map(equal_width, |el, w| {
                el.width(Size::px(w)).main_align(Alignment::Center)
            })
            .maybe(equal_width.is_none(), |el| {
                el.padding(Gaps::new_symmetric(0., 12.))
            })
            .background(if is_selected {
                colors::component_bg_pressed()
            } else {
                Color::TRANSPARENT
            })
            .maybe(focused && !disabled, |el| {
                el.border(border_all_color(1., colors::brand()))
            })
            .maybe(!disabled, |el| {
                el.on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                    .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                    .on_all_press(move |_| *selected.write() = value)
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
                    .font_size(13.)
                    .font_weight(if is_selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(content_color)
                    .into_element()
            }))
    }
}
