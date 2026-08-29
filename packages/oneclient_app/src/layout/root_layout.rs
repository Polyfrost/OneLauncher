use freya::prelude::*;
use freya::router::*;

use crate::components::{
    AccountSwitcher, ClusterUpdatePopup, GenericPromptOverlay, JavaPromptOverlay,
    NotificationCenter, PackageUpdatePopup, SplashCurtain, StatusBar, Toasts,
    UpdatePromptOverlay,
};
use crate::hooks::{SplashState, use_provide_splash};
use crate::layout::HomeArtPrefetch;
use crate::motion::AnimationClockDriver;
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

        let corner = crate::ui::use_window_corner();

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .background(colors::page())
            .color(colors::fg_primary())
            .font_family(theme::DEFAULT_FONT)
            .corner_radius(CornerRadius::new_all(corner))
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
            // Must stay last it renders whatever the overlays above did not claim
            .child(GenericPromptOverlay)
            .child(ClusterUpdatePopup)
            .child(PackageUpdatePopup)
            .child(StatusBar)
            .child(SplashCurtain)
            .child(AnimationClockDriver)
            .child(HomeArtPrefetch)
    }
}
