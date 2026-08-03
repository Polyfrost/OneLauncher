mod app_navbar;
mod onboarding_navbar;

pub use app_navbar::Navbar as AppNavbar;
pub use onboarding_navbar::OnboardingNavbar;

use freya::prelude::*;

use crate::components::{Button, Icon, IconType};

pub(super) fn navbar_button() -> Button {
    Button::new()
        .ghost()
        .icon()
        .width(Size::px(36.))
        .height(Size::px(36.))
}

pub fn window_controls() -> impl IntoElement {
    let minimize = |_| {
        Platform::get().with_window(None, |win| {
            win.set_minimized(true);
        });
    };

    let maximize = |_| {
        Platform::get().with_window(None, |win| {
            win.set_maximized(!win.is_maximized());
        });
    };

    // Not `close_window`: with a tray registered freya keeps the event loop
    // running once the last window goes, so closing it would leave the launcher
    // alive with nothing on screen.
    let close = |_| crate::platform::quit(&Platform::get());

    rect()
        .horizontal()
        .main_align(Alignment::End)
        .cross_align(Alignment::Center)
        .spacing(8.)
        .layer(Layer::OverlayLevel(15))
        .child(
            navbar_button()
                .child(Icon::new(IconType::Minus))
                .on_press(minimize),
        )
        .child(
            navbar_button()
                .child(Icon::new(IconType::Maximize01).size(20.))
                .on_press(maximize),
        )
        .child(
            navbar_button()
                .child(Icon::new(IconType::X))
                .on_press(close),
        )
}
