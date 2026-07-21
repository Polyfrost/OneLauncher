use freya::prelude::*;
use freya::router::*;

use crate::components::{
    AccountSwitcher, ClusterUpdatePopup, JavaPromptOverlay, NotificationCenter, SplashCurtain,
    StatusBar, Toasts, UpdatePromptOverlay,
};
use crate::hooks::{SplashState, use_provide_splash};
use crate::routes::Route;
use crate::theme;
use crate::theme::colors;

#[derive(PartialEq)]
pub struct RootLayout;

impl Component for RootLayout {
    fn render(&self) -> impl IntoElement {
        let active = use_state(|| false);
        let home_ready = use_state(|| false);
        use_provide_splash(SplashState { active, home_ready });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .background(colors::page())
            .color(colors::fg_primary())
            .font_family(theme::DEFAULT_FONT)
            .corner_radius(CornerRadius::new_all(12.))
            .overflow(Overflow::Clip)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .child(Outlet::<Route>::new()),
            )
            .child(NotificationCenter)
            .child(AccountSwitcher)
            .child(Toasts)
            .child(UpdatePromptOverlay)
            .child(JavaPromptOverlay)
            .child(ClusterUpdatePopup)
            .child(StatusBar)
            .child(SplashCurtain)
    }
}
