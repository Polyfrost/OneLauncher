use freya::{
    animation::{
        AnimColor, AnimNum, AnimatedValue, Ease, Function, OnChange, OnCreation, UseAnimation,
        use_animation,
    },
    prelude::*,
};

use crate::{
    i18n::t,
    theme::{colors, rounded, text_size},
    ui,
};

const WIDTH: f32 = 40.;
const HANDLE_SIZE: f32 = 16.;
const TIME: u64 = 180;

fn use_switch_transition<A: AnimatedValue>(
    value: State<bool>,
    mut run: impl 'static + FnMut(bool) -> A,
) -> UseAnimation<A> {
    use_animation(move |conf| {
        conf.on_change(OnChange::Rerun);
        conf.on_creation(OnCreation::Finish);
        run(*value.read())
    })
}

pub fn toggle(value: State<bool>) -> impl IntoElement {
    let mut v = value;
    Switch {
        value,
        on_press: (move |()| v.toggle()).into(),
        disabled: false,
    }
}

#[allow(dead_code)]
pub fn toggle_disabled(value: State<bool>) -> impl IntoElement {
    Switch {
        value,
        on_press: (|()| {}).into(),
        disabled: true,
    }
}

pub fn toggle_controlled(on: bool, on_toggle: EventHandler<()>) -> ToggleControlled {
    ToggleControlled {
        on,
        on_toggle,
        disabled: false,
    }
}

#[derive(PartialEq)]
pub struct ToggleControlled {
    on: bool,
    on_toggle: EventHandler<()>,
    disabled: bool,
}

impl ToggleControlled {
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Component for ToggleControlled {
    fn render(&self) -> impl IntoElement {
        let mut value = use_state(|| self.on);
        value.set_if_modified(self.on);
        Switch {
            value,
            on_press: self.on_toggle.clone(),
            disabled: self.disabled,
        }
    }
}

#[derive(PartialEq)]
struct Switch {
    value: State<bool>,
    on_press: EventHandler<()>,
    disabled: bool,
}

impl Component for Switch {
    fn render(&self) -> impl IntoElement {
        let value = self.value;
        let on_press = self.on_press.clone();
        let on = *value.read();
        let disabled = self.disabled;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let knob_align = if on { Alignment::End } else { Alignment::Start };

        let track_border = if focus().is_focused() && !disabled {
            colors::fg_primary()
        } else {
            colors::control_border()
        };

        let background = use_switch_transition(value, |on| {
            if on {
                AnimColor::new(colors::control_off(), colors::brand())
            } else {
                AnimColor::new(colors::brand(), colors::control_off())
            }
            .time(TIME)
            .function(Function::Expo)
            .ease(Ease::Out)
        });

        let right_offset = use_switch_transition(value, |on| {
            if on {
                AnimNum::new(WIDTH - HANDLE_SIZE - 6., 0.)
            } else {
                AnimNum::new(0., WIDTH - HANDLE_SIZE - 6.)
            }
            .time(TIME)
            .function(Function::Expo)
            .ease(Ease::Out)
        });

        let text = if on {
            t("oneclient.common.on")
        } else {
            t("oneclient.common.off")
        };

        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .a11y_id(a11y_id)
            .a11y_focusable(!disabled)
            .a11y_role(AccessibilityRole::Button)
            .maybe(!disabled, |el| {
                el.cursor(CursorIcon::Pointer).on_press(move |_| {
                    on_press.call(());
                })
            })
            .child(
                label()
                    .text(text)
                    .font_size(text_size(12.))
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .width(Size::px(WIDTH))
                    .height(Size::px(22.))
                    .corner_radius(rounded(11.))
                    .padding(Gaps::new_all(3.))
                    .main_align(knob_align)
                    .cross_align(Alignment::Center)
                    .background(if disabled {
                        colors::component_bg_disabled()
                    } else {
                        background.read().value()
                    })
                    .border(ui::border_all_color(1., track_border))
                    .child(
                        rect()
                            .width(Size::px(HANDLE_SIZE))
                            .height(Size::px(HANDLE_SIZE))
                            .corner_radius(rounded(8.))
                            .background(if disabled {
                                colors::fg_secondary()
                            } else if on {
                                colors::on_brand_strong()
                            } else {
                                Color::WHITE
                            })
                            .position(Position::new_absolute().right(right_offset.read().value())),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use freya_testing::prelude::*;

    use super::*;

    fn app() -> impl IntoElement {
        toggle_controlled(true, (|()| {}).into())
    }

    #[test]
    fn a_switch_mounted_on_shows_on_without_animating() {
        let mut test = launch_test(app);
        test.sync_and_update();

        let brand = Fill::Color(colors::brand());
        let track = test.find(|_, element| {
            Rect::try_downcast(element).filter(|rect| rect.style.background == brand)
        });

        assert!(track.is_some(), "the track should already be the on colour");
    }
}
