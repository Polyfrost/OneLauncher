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

        // macOS draws the corners natively (rounded window + shadow via the
        // window attributes), so Freya must not round on top of it.
        #[cfg(target_os = "macos")]
        let corner = 0.;

        // Other platforms are borderless: Freya rounds, squared when maximized so
        // there's no gap to the screen edge. No reactive maximized signal exists,
        // so mirror it — root_size changes on every maximize/restore/resize, which
        // re-runs the effect to re-query.
        #[cfg(not(target_os = "macos"))]
        let corner = {
            let root_size = Platform::get().root_size;
            let mut maximized = use_state(|| false);
            let size = *root_size.read();
            let dep = (size.width as i32, size.height as i32);
            use_side_effect_with_deps(&dep, move |_| {
                spawn(async move {
                    let is_max = Platform::get()
                        .post_callback(|id, ctx| {
                            ctx.windows.get(&id).map(|w| w.window().is_maximized())
                        })
                        .await;
                    if let Ok(Some(is_max)) = is_max {
                        if *maximized.peek() != is_max {
                            maximized.set(is_max);
                        }
                    }
                });
            });
            if *maximized.read() { 0. } else { 12. }
        };

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
            .child(ClusterUpdatePopup)
            .child(StatusBar)
            .child(SplashCurtain)
    }
}
