mod app_navbar;
mod onboarding_navbar;

pub use app_navbar::Navbar as AppNavbar;
pub use onboarding_navbar::OnboardingNavbar;

use freya::prelude::*;

use crate::components::{Button, Icon, IconType};

pub(super) fn navbar_button(foreground: Option<Color>) -> Button {
    Button::new()
        .ghost()
        .foreground(foreground)
        .icon()
        .width(Size::px(36.))
        .height(Size::px(36.))
}

pub fn window_controls() -> impl IntoElement {
    window_controls_with(None)
}

pub fn window_controls_with(foreground: Option<Color>) -> impl IntoElement {
    let minimize = |_| {
        Platform::get().with_window(Platform::window_id(), |win| {
            win.set_minimized(true);
        });
    };

    let maximize = |_| {
        Platform::get().with_window(Platform::window_id(), |win| {
            win.set_maximized(!win.is_maximized());
        });
    };

    let close = |_| crate::platform::close();

    rect()
        .horizontal()
        .main_align(Alignment::End)
        .cross_align(Alignment::Center)
        .spacing(8.)
        .layer(Layer::OverlayLevel(15))
        .child(
            navbar_button(foreground)
                .tooltip("Minimize")
                .child(Icon::new(IconType::Minus))
                .on_press(minimize),
        )
        .child(
            navbar_button(foreground)
                .tooltip("Maximize")
                .child(Icon::new(IconType::Maximize01).size(20.))
                .on_press(maximize),
        )
        .child(
            navbar_button(foreground)
                .tooltip("Close")
                .child(Icon::new(IconType::X))
                .on_press(close),
        )
}
