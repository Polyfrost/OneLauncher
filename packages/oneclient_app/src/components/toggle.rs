use freya::{
    animation::{AnimColor, AnimNum, Ease, Function, use_animation_transition},
    prelude::*,
};

use crate::{theme::colors, ui};

const WIDTH: f32 = 40.;
const HANDLE_SIZE: f32 = 16.;
const TIME: u64 = 180;

pub fn toggle(value: State<bool>) -> impl IntoElement {
    let mut v = value;
    Switch {
        value,
        on_press: (move |()| v.toggle()).into(),
    }
}

pub fn toggle_controlled(on: bool, on_toggle: EventHandler<()>) -> ToggleControlled {
    ToggleControlled { on, on_toggle }
}

#[derive(PartialEq)]
pub struct ToggleControlled {
    on: bool,
    on_toggle: EventHandler<()>,
}

impl Component for ToggleControlled {
    fn render(&self) -> impl IntoElement {
        let mut value = use_state(|| self.on);
        value.set_if_modified(self.on);
        Switch {
            value,
            on_press: self.on_toggle.clone(),
        }
    }
}

#[derive(PartialEq)]
struct Switch {
    value: State<bool>,
    on_press: EventHandler<()>,
}

impl Component for Switch {
    fn render(&self) -> impl IntoElement {
        let value = self.value;
        let on_press = self.on_press.clone();
        let on = *value.read();

        let knob_align = if on { Alignment::End } else { Alignment::Start };

        let background = use_animation_transition(value, |_, on| {
            if on {
                AnimColor::new(colors::component_bg(), colors::brand())
            } else {
                AnimColor::new(colors::brand(), colors::component_bg())
            }
            .time(TIME)
            .function(Function::Expo)
            .ease(Ease::Out)
        });

        let right_offset = use_animation_transition(value, |_, on| {
            if on {
                AnimNum::new(WIDTH - HANDLE_SIZE - 6., 0.)
            } else {
                AnimNum::new(0., WIDTH - HANDLE_SIZE - 6.)
            }
            .time(TIME)
            .function(Function::Expo)
            .ease(Ease::Out)
        });

        let text = if on { "On" } else { "Off" };

        use_drop(|| {
            Cursor::set(CursorIcon::default());
        });

        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
            .on_press(move |_| {
                on_press.call(());
            })
            .child(
                label()
                    .text(text)
                    .font_size(12.)
                    .color(Color::from_rgb(167, 176, 190)),
            )
            .child(
                rect()
                    .width(Size::px(WIDTH))
                    .height(Size::px(22.))
                    .corner_radius(CornerRadius::new_all(11.))
                    .padding(Gaps::new_all(3.))
                    .main_align(knob_align)
                    .cross_align(Alignment::Center)
                    .background(&*background.read())
                    .border(ui::border_all_color(1., colors::component_border()))
                    .child(
                        rect()
                            .width(Size::px(HANDLE_SIZE))
                            .height(Size::px(HANDLE_SIZE))
                            .corner_radius(CornerRadius::new_all(8.))
                            .background(Color::WHITE)
                            .position(Position::new_absolute().right(right_offset.read().value())),
                    ),
            )
    }
}
