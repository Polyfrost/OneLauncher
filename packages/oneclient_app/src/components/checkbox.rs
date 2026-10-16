use freya::{
    animation::{AnimColor, AnimNum, AnimatedValue, Ease, Function, use_animation_transition},
    prelude::*,
};

use crate::{
    components::{Icon, IconType},
    theme::colors,
    ui,
};

const BOX_SIZE: f32 = 18.;
const MARK_SIZE: f32 = 12.;
const TIME: u64 = 180;

pub fn checkbox_labeled(value: State<bool>, text: impl Into<Box<str>>) -> impl IntoElement {
    let mut v = value;
    Check {
        value,
        label: Some(text.into()),
        on_press: (move |()| v.toggle()).into(),
        disabled: false,
    }
}

#[derive(PartialEq)]
struct Check {
    value: State<bool>,
    label: Option<Box<str>>,
    on_press: EventHandler<()>,
    disabled: bool,
}

impl Component for Check {
    fn render(&self) -> impl IntoElement {
        let value = self.value;
        let on_press = self.on_press.clone();
        let checked = *value.read();
        let disabled = self.disabled;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let box_border = if focus().is_focused() && !disabled {
            colors::fg_primary()
        } else if checked && !disabled {
            colors::brand()
        } else {
            colors::component_border()
        };

        let background = use_animation_transition(value, |_, checked| {
            if checked {
                AnimColor::new(colors::component_bg(), colors::brand())
            } else {
                AnimColor::new(colors::brand(), colors::component_bg())
            }
            .time(TIME)
            .function(Function::Expo)
            .ease(Ease::Out)
        });

        let mark = use_animation_transition(value, |_, checked| {
            let scale = AnimNum::new(0.5, 1.)
                .time(TIME)
                .function(Function::Expo)
                .ease(Ease::Out);
            let opacity = AnimNum::new(0., 1.)
                .time(TIME)
                .function(Function::Expo)
                .ease(Ease::Out);

            if checked {
                (scale, opacity)
            } else {
                (scale.into_reversed(), opacity.into_reversed())
            }
        });

        let (mark_scale, mark_opacity) = {
            let mark = mark.read();
            (mark.0.value(), mark.1.value())
        };

        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .a11y_id(a11y_id)
            .a11y_focusable(!disabled)
            .a11y_role(AccessibilityRole::CheckBox)
            .maybe(!disabled, |el| {
                el.cursor(CursorIcon::Pointer).on_press(move |_| {
                    on_press.call(());
                })
            })
            .child(
                rect()
                    .width(Size::px(BOX_SIZE))
                    .height(Size::px(BOX_SIZE))
                    .corner_radius(CornerRadius::new_all(5.))
                    .main_align(Alignment::Center)
                    .cross_align(Alignment::Center)
                    .background(if disabled {
                        colors::component_bg_disabled()
                    } else {
                        background.read().value()
                    })
                    .border(ui::border_all_color(1., box_border))
                    .maybe_child((checked || mark_opacity > 0.).then(|| {
                        rect().opacity(mark_opacity).scale(mark_scale).child(
                            Icon::new(IconType::Check)
                                .size(MARK_SIZE)
                                .color(if disabled {
                                    colors::fg_secondary()
                                } else {
                                    Color::WHITE
                                }),
                        )
                    })),
            )
            .maybe_child(self.label.as_ref().map(|text| {
                label()
                    .text(text.to_string())
                    .font_size(12.)
                    .color(if disabled {
                        colors::fg_secondary()
                    } else {
                        colors::fg_primary()
                    })
            }))
    }
}

/// A checkbox whose value belongs to a larger selection model.
pub fn checkbox_controlled(
    checked: bool,
    text: impl Into<Box<str>>,
    on_press: impl FnMut(()) + 'static,
) -> impl IntoElement {
    ControlledCheck {
        checked,
        text: text.into(),
        on_press: EventHandler::new(on_press),
    }
}
#[derive(PartialEq)]
struct ControlledCheck {
    checked: bool,
    text: Box<str>,
    on_press: EventHandler<()>,
}
impl Component for ControlledCheck {
    fn render(&self) -> impl IntoElement {
        // The selection model is the sole source of truth. Mirroring props into
        // hook state during render briefly shows a recycled row's previous value.
        let checked = self.checked;
        let on_press = self.on_press.clone();
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::CheckBox)
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| on_press.call(()))
            .child(
                rect()
                    .width(Size::px(BOX_SIZE))
                    .height(Size::px(BOX_SIZE))
                    .corner_radius(CornerRadius::new_all(5.))
                    .center()
                    .background(if checked {
                        colors::brand()
                    } else {
                        colors::component_bg()
                    })
                    .border(ui::border_all_color(
                        1.,
                        if focus().is_focused() {
                            colors::fg_primary()
                        } else if checked {
                            colors::brand()
                        } else {
                            colors::component_border()
                        },
                    ))
                    .maybe_child(checked.then(|| {
                        Icon::new(IconType::Check)
                            .size(MARK_SIZE)
                            .color(Color::WHITE)
                    })),
            )
            .child(
                label()
                    .text(self.text.to_string())
                    .font_size(12.)
                    .color(colors::fg_primary()),
            )
    }
}
