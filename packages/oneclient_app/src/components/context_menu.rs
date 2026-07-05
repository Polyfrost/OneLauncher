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

pub struct ContextMenu {
    x: f32,
    y: f32,
    entries: Vec<Entry>,
    on_close: EventHandler<()>,
}

impl ContextMenu {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            entries: Vec::new(),
            on_close: (|()| {}).into(),
        }
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
        // the menu is rebuilt from scratch whenever it is reopened (kind of a bad idea but who cares lol)
        false
    }
}

impl Component for ContextMenu {
    fn render(&self) -> impl IntoElement {
        let on_close = self.on_close.clone();

        let mut width = use_state(|| 0f32);

        let item_width = {
            let w = *width.read();
            (w > 0.).then_some(w)
        };

        let mut list = rect().vertical().spacing(4.);
        for entry in &self.entries {
            list = match entry {
                Entry::Separator => {
                    let mut sep = rect()
                        .height(Size::px(1.))
                        .margin(Gaps::new_symmetric(4., 0.))
                        .background(MENU_BORDER);
                    if let Some(w) = item_width {
                        sep = sep.width(Size::px(w));
                    }
                    list.child(sep)
                }
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
            .child(list);

        OverlayPopup::new()
            .backdrop(false)
            .position(Position::new_global().top(self.y).left(self.x))
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
            .on_pointer_enter(move |_| {
                hovered.set(true);
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                hovered.set(false);
                Cursor::set(CursorIcon::default());
            })
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
