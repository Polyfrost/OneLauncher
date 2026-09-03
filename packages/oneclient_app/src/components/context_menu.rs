use freya::prelude::*;

use crate::components::{Icon, IconType, OverlayPopup};
use crate::theme::colors;

const MENU_BG: Color = Color::from_rgb(25, 32, 38);
const MENU_BORDER: Color = Color::from_argb(26, 255, 255, 255);
const MENU_FG: Color = Color::from_rgb(155, 161, 166);
const MENU_DANGER: Color = Color::from_rgb(242, 84, 90);

enum Entry {
    Action {
        icon: IconType,
        label: String,
        danger: bool,
        on_select: EventHandler<()>,
    },
    Separator,
}

fn separator(item_width: Option<f32>) -> Rect {
    let mut sep = rect()
        .height(Size::px(1.))
        .margin(Gaps::new_symmetric(4., 0.))
        .background(MENU_BORDER);
    if let Some(w) = item_width {
        sep = sep.width(Size::px(w));
    }
    sep
}

pub struct ContextMenu {
    x: f32,
    y: f32,
    upwards: bool,
    title: Option<String>,
    entries: Vec<Entry>,
    on_close: EventHandler<()>,
}

impl ContextMenu {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            upwards: false,
            title: None,
            entries: Vec::new(),
            on_close: (|()| {}).into(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn open_upwards(mut self) -> Self {
        self.upwards = true;
        self
    }

    pub fn on_close(mut self, on_close: impl Into<EventHandler<()>>) -> Self {
        self.on_close = on_close.into();
        self
    }

    pub fn action(
        mut self,
        icon: IconType,
        label: impl Into<String>,
        on_select: impl Into<EventHandler<()>>,
    ) -> Self {
        self.entries.push(Entry::Action {
            icon,
            label: label.into(),
            danger: false,
            on_select: on_select.into(),
        });
        self
    }

    pub fn danger_action(
        mut self,
        icon: IconType,
        label: impl Into<String>,
        on_select: impl Into<EventHandler<()>>,
    ) -> Self {
        self.entries.push(Entry::Action {
            icon,
            label: label.into(),
            danger: true,
            on_select: on_select.into(),
        });
        self
    }

    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }
}

impl PartialEq for ContextMenu {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Component for ContextMenu {
    fn render(&self) -> impl IntoElement {
        let on_close = self.on_close.clone();

        let mut width = use_state(|| 0f32);
        let mut height = use_state(|| 0f32);

        let item_width = {
            let w = *width.read();
            (w > 0.).then_some(w)
        };

        let mut list = rect().vertical().spacing(4.);

        if let Some(title) = &self.title {
            list = list
                .child(
                    // Asymmetric on purpose
                    rect().padding(Gaps::new(6., 8., 4., 8.)).child(
                        label()
                            .text(title.clone())
                            .font_size(11.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .max_lines(1)
                            .color(colors::fg_secondary()),
                    ),
                )
                .child(separator(item_width));
        }

        for entry in &self.entries {
            list = match entry {
                Entry::Separator => list.child(separator(item_width)),
                Entry::Action {
                    icon,
                    label,
                    danger,
                    on_select,
                } => list.child(
                    ContextMenuRow {
                        icon: *icon,
                        label: label.clone(),
                        danger: *danger,
                        width: item_width,
                        on_select: on_select.clone(),
                        on_close: on_close.clone(),
                    }
                    .into_element(),
                ),
            };
        }

        let list = list.on_sized(move |e: Event<SizedEventData>| {
            let measured = e.data().area.width();
            if (measured - *width.peek()).abs() > 0.5 {
                width.set(measured);
            }
        });

        let measured = *height.read();
        let flips = self.upwards && measured > 0. && self.y - measured >= 0.;
        let top = if flips { self.y - measured } else { self.y };
        let placed = !self.upwards || measured > 0.;

        let panel = rect()
            .vertical()
            .padding(Gaps::new_all(6.))
            .corner_radius(CornerRadius::new_all(12.))
            .background(MENU_BG)
            .border(Border::new().fill(MENU_BORDER).width(BorderWidth {
                top: 1.,
                right: 1.,
                bottom: 1.,
                left: 1.,
            }))
            .opacity(if placed { 1. } else { 0. })
            .on_sized(move |e: Event<SizedEventData>| {
                let measured = e.data().area.height();
                if (measured - *height.peek()).abs() > 0.5 {
                    height.set(measured);
                }
            })
            .child(list);

        OverlayPopup::new()
            .backdrop(true)
            .position(Position::new_global().top(top).left(self.x))
            .on_close(move |_| on_close.call(()))
            .child(panel.into_element())
    }
}

#[derive(PartialEq)]
struct ContextMenuRow {
    icon: IconType,
    label: String,
    danger: bool,
    width: Option<f32>,
    on_select: EventHandler<()>,
    on_close: EventHandler<()>,
}

impl Component for ContextMenuRow {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_select = self.on_select.clone();
        let on_close = self.on_close.clone();

        let base = if self.danger { MENU_DANGER } else { MENU_FG };
        let color = if *hovered.read() {
            colors::fg_primary()
        } else {
            base
        };

        let mut root = rect();
        if let Some(w) = self.width {
            root = root.width(Size::px(w));
        }

        root.horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .padding(Gaps::new_symmetric(5., 8.))
            .corner_radius(CornerRadius::new_all(6.))
            .background(if *hovered.read() {
                colors::component_bg_hover()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| {
                on_select.call(());
                on_close.call(());
            })
            .child(Icon::new(self.icon).size(18.).color(color))
            .child(
                label()
                    .text(self.label.clone())
                    .font_size(12.)
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(1)
                    .color(color),
            )
    }
}
