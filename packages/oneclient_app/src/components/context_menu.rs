use freya::prelude::*;

use crate::components::{Icon, IconType, OVERLAY_BASE_LEVEL, OverlayPopup};
use crate::theme::{colors, rounded, text_size};
use crate::ui::{EDGE_MARGIN, clamp_to_window, window_logical_size};

/// Panel padding and border on both axes, which the measured list sits inside of
const PANEL_INSET: f32 = 14.;

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
        .margin(Gaps::new_symmetric(3., 0.))
        .background(colors::component_border());
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
    overlay_level: u8,
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
            overlay_level: OVERLAY_BASE_LEVEL,
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

    pub fn overlay_level(mut self, level: u8) -> Self {
        self.overlay_level = level;
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
        // Always unequal the menu is rebuilt from scratch whenever it is reopened
        false
    }
}

impl Component for ContextMenu {
    fn render(&self) -> impl IntoElement {
        let on_close = self.on_close.clone();

        let mut size = use_state(Size2D::zero);

        let measured = *size.read();
        let item_width = (measured.width > 0.).then_some(measured.width);

        let mut keyboard = use_state(|| None::<usize>);
        let mut count = 0;
        let mut list = rect()
            .vertical()
            .min_width(Size::px(178.))
            .max_width(Size::px(336.));

        if let Some(title) = &self.title {
            list = list
                .child(
                    rect().padding(Gaps::new(7., 10., 5., 10.)).child(
                        label()
                            .text(title.clone())
                            .font_size(text_size(12.))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_primary().with_a(230)),
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
                } => {
                    let index = count;
                    count += 1;
                    list.child(
                        ContextMenuRow {
                            index,
                            keyboard,
                            icon: *icon,
                            label: label.clone(),
                            danger: *danger,
                            width: item_width,
                            on_select: on_select.clone(),
                            on_close: on_close.clone(),
                        }
                        .into_element(),
                    )
                }
            };
        }

        let list = list.on_sized(move |e: Event<SizedEventData>| {
            let area = e.data().area.size;
            let prev = *size.peek();
            if (area.width - prev.width).abs() > 0.5 || (area.height - prev.height).abs() > 0.5 {
                size.set(area);
            }
        });

        let placed = measured.width > 0.;
        let menu_width = measured.width + PANEL_INSET;
        let menu_height = measured.height + PANEL_INSET;

        let (x, y) = if placed {
            let window = window_logical_size();
            let x = menu_axis(self.x, menu_width, window.width, false);
            let y = menu_axis(self.y, menu_height, window.height, self.upwards);
            clamp_to_window(x, y, menu_width, menu_height)
        } else {
            (self.x, self.y)
        };

        let theme = colors::active_theme();
        let panel =
            crate::theme::glass::popup_panel(rect(), 12., colors::page_elevated().with_a(255))
                .vertical()
                .padding(Gaps::new_all(6.))
                .border(
                    Border::new()
                        .fill(if theme.colors.light {
                            colors::fg_primary().with_a(58)
                        } else {
                            colors::component_border_hover()
                        })
                        .alignment(BorderAlignment::Inner)
                        .width(BorderWidth {
                            top: 1.,
                            right: 1.,
                            bottom: 1.,
                            left: 1.,
                        }),
                )
                .shadow(Shadow::from((
                    0.,
                    6.,
                    17.,
                    0.,
                    Color::from_argb(110, 0, 0, 0),
                )))
                .opacity(if placed { 1. } else { 0. })
                .a11y_role(AccessibilityRole::Menu)
                .map(self.title.clone(), |el, title| el.a11y_alt(title))
                .on_global_key_down({
                    let level = self.overlay_level;
                    move |event: Event<KeyboardEventData>| {
                        if count == 0 || !crate::components::overlay_is_topmost(level) {
                            return;
                        }
                        let current = *keyboard.peek();
                        let next = match event.key {
                            Key::Named(NamedKey::ArrowDown) => {
                                current.map_or(0, |n| (n + 1) % count)
                            }
                            Key::Named(NamedKey::ArrowUp) => {
                                current.map_or(count - 1, |n| (n + count - 1) % count)
                            }
                            Key::Named(NamedKey::Home) => 0,
                            Key::Named(NamedKey::End) => count - 1,
                            _ => return,
                        };
                        event.prevent_default();
                        event.stop_propagation();
                        keyboard.set_if_modified(Some(next));
                    }
                })
                .child(list);

        OverlayPopup::new()
            .backdrop(false)
            .overlay_level(self.overlay_level)
            .position(Position::new_global().top(y).left(x))
            .on_close(move |_| on_close.call(()))
            .child(panel.into_element())
    }
}

fn menu_axis(anchor: f32, length: f32, limit: f32, prefer_before: bool) -> f32 {
    let before = anchor - length - 2.;
    let after = anchor + 2.;
    if before >= EDGE_MARGIN && (prefer_before || after + length > limit - EDGE_MARGIN) {
        before
    } else {
        after
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_flips_clear_of_the_pointer_at_the_edge() {
        assert_eq!(menu_axis(780., 192., 800., false), 586.);
        assert_eq!(menu_axis(12., 290., 600., true), 14.);
        assert_eq!(menu_axis(440., 290., 600., true), 148.);
    }
}

#[derive(PartialEq)]
struct ContextMenuRow {
    index: usize,
    keyboard: State<Option<usize>>,
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
        let mut pressed = use_state(|| false);
        let id = use_a11y();
        let focus = use_focus(id);
        let is_key_target = *self.keyboard.read() == Some(self.index);
        let pixel_icons = colors::active_theme().pixel_icons;
        use_side_effect_with_deps(&is_key_target, move |active| {
            if *active {
                id.request_focus();
            }
        });
        let on_select = self.on_select.clone();
        let on_close = self.on_close.clone();

        let color = if self.danger {
            colors::danger()
        } else {
            colors::fg_primary()
        };
        let highlight = if self.danger {
            colors::danger()
        } else {
            colors::brand()
        };
        let mut keyboard = self.keyboard;
        let highlighted = if keyboard.read().is_some() {
            is_key_target
        } else {
            hovered() || focus() == Focus::Keyboard
        };

        let mut root = rect();
        if let Some(w) = self.width {
            root = root.width(Size::px(w));
        }

        root.horizontal()
            .cross_align(Alignment::Center)
            .spacing(10.)
            .min_height(Size::px(34.))
            .padding(Gaps::new_symmetric(4., 10.))
            .corner_radius(rounded(6.))
            .background(if pressed() {
                highlight.with_a(if colors::is_light() { 85 } else { 110 })
            } else if highlighted {
                highlight.with_a(if colors::is_light() { 46 } else { 66 })
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .maybe(is_key_target, |el| {
                el.border(
                    Border::new()
                        .width(1.)
                        .fill(if colors::is_light() {
                            highlight.with_a(178)
                        } else {
                            colors::fg_primary().with_a(205)
                        })
                        .alignment(BorderAlignment::Inner),
                )
            })
            .a11y_id(id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::MenuItem)
            .a11y_alt(self.label.clone())
            .on_pointer_down(move |_| {
                keyboard.set_if_modified(None);
                pressed.set(true);
            })
            .on_global_pointer_up(move |_| pressed.set_if_modified(false))
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_move(move |_| keyboard.set_if_modified(None))
            .on_pointer_leave(move |_| {
                hovered.set(false);
                pressed.set(false);
            })
            .on_press(move |event: Event<PressEventData>| {
                event.stop_propagation();
                on_select.call(());
                on_close.call(());
            })
            .child(
                Icon::new(self.icon)
                    .size(if pixel_icons { 20. } else { 16. })
                    .color(if pixel_icons || self.danger || highlighted {
                        color
                    } else {
                        color.with_a(205)
                    }),
            )
            .child(
                label()
                    .text(self.label.clone())
                    .font_size(text_size(if pixel_icons { 15. } else { 14. }))
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(1)
                    .color(color),
            )
    }
}
