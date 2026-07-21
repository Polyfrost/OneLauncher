use std::time::Duration;

use freya::animation::{AnimNum, Ease, Function, use_animation};
use freya::prelude::*;

use crate::AppAssets;
use crate::hooks::use_splash;
use crate::theme::colors;

/// How long the curtain takes to fade away once home is ready.
const FADE_MS: u64 = 460;
/// Safety net: reveal the app even if the "ready" signal never arrives.
const FALLBACK_MS: u64 = 8000;

#[derive(PartialEq)]
pub struct SplashCurtain;

impl Component for SplashCurtain {
    fn render(&self) -> impl IntoElement {
        let splash = use_splash();

        if !*splash.active.read() {
            return rect().into_element();
        }

        // Fresh mount whenever the curtain is raised, so `CurtainFade`'s
        // animation and timers start from the beginning each time.
        CurtainFade.into_element()
    }
}

#[derive(PartialEq)]
struct CurtainFade;

impl Component for CurtainFade {
    fn render(&self) -> impl IntoElement {
        let splash = use_splash();
        let ready = *splash.home_ready.read();

        let logo = use_memo(|| AppAssets::get_bytes("logo.svg").unwrap_or_default());

        let fade = use_animation(|_| {
            AnimNum::new(1.0, 0.0)
                .time(FADE_MS)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });

        // Force-reveal after a while if home never reports ready.
        use_hook(move || {
            let mut home_ready = splash.home_ready;
            spawn(async move {
                tokio::time::sleep(Duration::from_millis(FALLBACK_MS)).await;
                if !*home_ready.peek() {
                    home_ready.set(true);
                }
            });
        });

        // When home is ready, play the fade-out then drop the curtain.
        use_side_effect_with_deps(&ready, move |&ready| {
            if !ready {
                return;
            }
            let mut fade = fade;
            fade.start();
            let mut active = splash.active;
            spawn(async move {
                tokio::time::sleep(Duration::from_millis(FADE_MS + 40)).await;
                active.set(false);
            });
        });

        let opacity = fade.get().value();

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .position(Position::new_global().top(0.).left(0.))
            .layer(Layer::Overlay)
            .opacity(opacity)
            .background(colors::page())
            .window_drag()
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .center()
                    .vertical()
                    .spacing(14.)
                    .child(
                        svg(logo.read().cloned())
                            .width(Size::px(288.))
                            .height(Size::px(60.))
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text("The only Minecraft launcher you'll need.")
                            .font_size(15.)
                            .color(colors::fg_primary().with_a(140)),
                    ),
            )
            .into_element()
    }
}
