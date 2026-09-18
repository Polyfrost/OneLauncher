use freya::prelude::*;
use freya::router::*;

use crate::components::{
    AccountSwitcher, ClusterUpdatePopup, ControlCenter, GenericPromptOverlay, JavaPromptOverlay,
    MicrosoftJavaPromptOverlay, NotificationCenter, OptionalModsPopup, PackageUpdatePopup,
    SplashCurtain, StatusBar, Toasts, TooltipHost, UpdatePromptOverlay, use_provide_tooltips,
};
#[cfg(not(target_os = "macos"))]
use crate::hooks::use_start_maximized;
use crate::hooks::{SplashState, use_provide_overlay_claims, use_provide_splash};
use crate::layout::{HomeArtPrefetch, PendingLaunchDriver};
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
        use_provide_overlay_claims();
        use_provide_tooltips();

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .background(colors::page())
            .color(colors::fg_primary())
            .font_family(theme::DEFAULT_FONT)
            .overflow(Overflow::Clip)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .child(Outlet::<Route>::new()),
            )
            .child(NotificationCenter)
            .child(AccountSwitcher)
            .child(ControlCenter)
            .child(Toasts)
            .child(UpdatePromptOverlay)
            .child(JavaPromptOverlay)
            .child(MicrosoftJavaPromptOverlay)
            // Must stay last it renders whatever the overlays above did not claim
            .child(GenericPromptOverlay)
            .child(ClusterUpdatePopup)
            .child(OptionalModsPopup)
            .child(PackageUpdatePopup)
            .child(StatusBar)
            .child(TooltipHost)
            .child(SplashCurtain)
            .child(AnimationClockDriver)
            .child(HomeArtPrefetch)
            .child(PendingLaunchDriver)
    }
}
