use std::time::Instant;

use freya::prelude::*;

use crate::motion::use_animations_enabled;

/// Magnify the illustration inside its existing bounds. The caption, focus ring and
/// hit target stay stationary, even when the pointer reverses direction mid-transition.
#[derive(PartialEq)]
pub(super) struct ArtZoom {
    pub highlighted: bool,
    pub art: Element,
}

impl Component for ArtZoom {
    fn render(&self) -> impl IntoElement {
        let scale = use_art_scale(self.highlighted);
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .interactive(false)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .scale(scale)
                    .child(self.art.clone()),
            )
    }
}

pub(super) fn use_art_scale(highlighted: bool) -> f32 {
    let animated = use_animations_enabled();
    let mut scale = use_state(|| 1.0f32);
    let mut task = use_state(|| None::<OwnedTaskHandle>);
    use_side_effect_with_deps(&(animated, highlighted), move |&(animated, highlighted)| {
        task.set(None);
        if !animated {
            scale.set_if_modified(1.);
            return;
        }
        let from = *scale.peek();
        let to = if highlighted { 1.075 } else { 1. };
        if (from - to).abs() < 0.0001 {
            return;
        }
        let duration = if highlighted { 0.22 } else { 0.18 };
        let ticker = RenderingTicker::get();
        let window = Platform::get().current_window();
        let handle = spawn(async move {
            let start = Instant::now();
            loop {
                window.request_redraw();
                ticker.tick().await;
                let t = (start.elapsed().as_secs_f32() / duration).min(1.);
                scale.set(to + (from - to) * (1. - t).powi(3));
                if t >= 1. {
                    break;
                }
            }
        });
        task.set(Some(handle.owned()));
    });
    *scale.read()
}
