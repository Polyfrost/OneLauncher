use freya::prelude::*;

use crate::theme::colors;

pub struct TabItem {
    label: String,
    active: bool,
    count: Option<String>,
    on_press: Option<Box<dyn FnMut(Event<PressEventData>)>>,
}

impl TabItem {
    pub fn new(label: impl Into<String>, active: bool) -> Self {
        Self {
            label: label.into(),
            active,
            count: None,
            on_press: None,
        }
    }

    pub fn count_text(mut self, text: impl Into<String>) -> Self {
        self.count = Some(text.into());
        self
    }

    pub fn on_press(mut self, handler: impl FnMut(Event<PressEventData>) + 'static) -> Self {
        self.on_press = Some(Box::new(handler));
        self
    }
}

pub struct TabBar {
    tabs: Vec<TabItem>,
    width: Size,
    height: Size,
    spacing: f32,
    font_size: f32,
    bold_active: bool,
}

impl Default for TabBar {
    fn default() -> Self {
        Self::new()
    }
}

impl TabBar {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            width: Size::fill(),
            height: Size::fill(),
            spacing: 24.,
            font_size: 12.,
            bold_active: false,
        }
    }

    pub fn tabs(mut self, tabs: impl IntoIterator<Item = TabItem>) -> Self {
        self.tabs = tabs.into_iter().collect();
        self
    }

    pub fn width(mut self, width: Size) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Size) -> Self {
        self.height = height;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    pub fn bold_active(mut self, bold_active: bool) -> Self {
        self.bold_active = bold_active;
        self
    }
}

fn tab_label(
    text: &str,
    font_size: f32,
    active: bool,
    active_weight: FontWeight,
) -> impl IntoElement {
    rect()
        .center()
        .content(Content::Fit)
        .child(
            label()
                .text(text.to_string())
                .font_size(font_size)
                .font_weight(active_weight)
                .color(Color::TRANSPARENT),
        )
        .child(
            rect()
                .position(
                    Position::new_absolute()
                        .top(0.)
                        .left(0.)
                        .right(0.)
                        .bottom(0.),
                )
                .center()
                .child(
                    label()
                        .text(text.to_string())
                        .font_size(font_size)
                        .font_weight(if active {
                            active_weight
                        } else {
                            FontWeight::NORMAL
                        })
                        .color(colors::fg_primary()),
                ),
        )
}

fn count_pill(count: &str, font_size: f32) -> impl IntoElement {
    rect()
        .center()
        .padding(Gaps::new_symmetric(1., 6.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(colors::component_bg())
        .child(
            label()
                .text(count.to_string())
                .font_size((font_size - 3.).max(8.))
                .color(colors::fg_secondary()),
        )
}

impl IntoElement for TabBar {
    fn into_element(self) -> Element {
        let font_size = self.font_size;
        let bold_active = self.bold_active;

        rect()
            .horizontal()
            .width(self.width)
            .height(self.height)
            .spacing(self.spacing)
            .cross_align(Alignment::Center)
            .content(Content::Fit)
            .children(self.tabs.into_iter().map(|tab| {
                let active = tab.active;
                let active_weight = if bold_active {
                    FontWeight::BOLD
                } else {
                    FontWeight::MEDIUM
                };

                let mut el = rect()
                    .vertical()
                    .content(Content::Fit)
                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                    .on_pointer_leave(|_| Cursor::set(CursorIcon::default()));

                if let Some(handler) = tab.on_press {
                    el = el.on_press(handler);
                }

                el.child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(6.)
                        .content(Content::Fit)
                        .child(
                            rect()
                                .vertical()
                                .content(Content::Fit)
                                .child(tab_label(&tab.label, font_size, active, active_weight))
                                .child(
                                    rect()
                                        .height(Size::px(1.5))
                                        .width(Size::fill_minimum())
                                        .margin(Gaps::new_symmetric(0., 4.0))
                                        .corner_radius(CornerRadius::new_all(2.))
                                        .background(if active {
                                            colors::fg_primary()
                                        } else {
                                            Color::TRANSPARENT
                                        }),
                                ),
                        )
                        .maybe_child(tab.count.map(|c| count_pill(&c, font_size).into_element())),
                )
                .into_element()
            }))
            .into_element()
    }
}
