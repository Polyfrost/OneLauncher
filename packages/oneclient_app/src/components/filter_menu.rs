use freya::animation::{AnimNum, Ease, OnCreation, use_animation};
use freya::prelude::*;

use crate::components::{Button, Icon, IconType};
use crate::hooks::use_overlay_claim;
use crate::theme::colors;

const PANEL_W: f32 = 172.;
const BUTTON_W: f32 = 34.;

#[derive(Clone, PartialEq)]
pub struct FilterOption {
    text: String,
    selected: bool,
    on_press: EventHandler<()>,
}

impl FilterOption {
    pub fn new(
        text: impl Into<String>,
        selected: bool,
        on_press: impl Into<EventHandler<()>>,
    ) -> Self {
        Self {
            text: text.into(),
            selected,
            on_press: on_press.into(),
        }
    }
}

#[derive(Clone, PartialEq)]
struct FilterSection {
    title: &'static str,
    options: Vec<FilterOption>,
}

#[derive(PartialEq)]
pub struct FilterMenu {
    active: bool,
    sections: Vec<FilterSection>,
}

impl FilterMenu {
    pub fn new(active: bool) -> Self {
        Self {
            active,
            sections: Vec::new(),
        }
    }

    pub fn section(
        mut self,
        title: &'static str,
        options: impl IntoIterator<Item = FilterOption>,
    ) -> Self {
        self.sections.push(FilterSection {
            title,
            options: options.into_iter().collect(),
        });
        self
    }
}

impl Component for FilterMenu {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);

        let icon_color = if self.active {
            colors::brand()
        } else {
            colors::fg_secondary()
        };

        rect()
            .width(Size::px(BUTTON_W))
            .child(
                Button::new()
                    .secondary()
                    .icon()
                    .tooltip("Sort and filter")
                    .width(Size::px(BUTTON_W))
                    .height(Size::px(34.))
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        open.toggle();
                    })
                    .child(Icon::new(IconType::Sliders04).size(16.).color(icon_color)),
            )
            .maybe_child(open().then(|| {
                FilterPanel {
                    sections: self.sections.clone(),
                    on_close: (move |()| open.set(false)).into(),
                }
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct FilterPanel {
    sections: Vec<FilterSection>,
    on_close: EventHandler<()>,
}

impl Component for FilterPanel {
    fn render(&self) -> impl IntoElement {
        use_overlay_claim();

        let backdrop_close = self.on_close.clone();
        let key_close = self.on_close.clone();

        let a11y_id = use_a11y();

        let fade = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(160).ease(Ease::Out)
        });
        let progress = fade.read().value();

        let mut panel = rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .padding(Gaps::new_all(8.))
            .opacity(progress)
            .offset_y((progress - 1.0) * 6.)
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::page_elevated().with_a(230))
            .backdrop_blur(12.)
            .border(crate::ui::border_all_color(1., colors::component_border()))
            .shadow(Shadow::from((
                0.,
                8.,
                32.,
                0.,
                Color::from_argb(120, 0, 0, 0),
            )))
            .a11y_id(a11y_id)
            .a11y_role(AccessibilityRole::Dialog)
            .on_global_key_down(move |e: Event<KeyboardEventData>| {
                if e.key == Key::Named(NamedKey::Escape) {
                    key_close.call(());
                }
            });

        for section in &self.sections {
            panel = panel.child(section_label(section.title));
            for option in &section.options {
                panel = panel.child(ChoiceRow {
                    option: option.clone(),
                });
            }
        }

        rect()
            .height(Size::px(0.))
            .width(Size::px(PANEL_W))
            .layer(Layer::Overlay)
            .child(
                rect()
                    .layer(Layer::OverlayLevel(10))
                    .position(Position::new_global().top(0.).left(0.))
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .on_press(move |_| backdrop_close.call(())),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .layer(Layer::OverlayLevel(12))
                    .margin(Gaps::new(6., 0., 0., -(PANEL_W - BUTTON_W)))
                    .child(panel),
            )
    }
}

fn section_label(text: &'static str) -> impl IntoElement {
    label()
        .text(text)
        .font_size(10.)
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_secondary())
}

#[derive(PartialEq)]
struct ChoiceRow {
    option: FilterOption,
}

impl Component for ChoiceRow {
    fn render(&self) -> impl IntoElement {
        let selected = self.option.selected;
        let on_press = self.option.on_press.clone();

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let color = if selected {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        let bg = if selected {
            colors::component_bg()
        } else if focused {
            colors::ghost_overlay_hover()
        } else {
            Color::TRANSPARENT
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .padding(Gaps::new_symmetric(5., 8.))
            .corner_radius(CornerRadius::new_all(6.))
            .background(bg)
            .content(Content::Flex)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::MenuItemRadio)
            .maybe(focused, |el| {
                el.border(crate::ui::border_all_color(1., colors::brand()))
            })
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| on_press.call(()))
            .child(
                label()
                    .text(self.option.text.clone())
                    .font_size(12.)
                    .width(Size::flex(1.0))
                    .color(color),
            )
            .maybe_child(selected.then(|| {
                Icon::new(IconType::Check)
                    .size(14.)
                    .color(colors::brand())
                    .into_element()
            }))
    }
}
