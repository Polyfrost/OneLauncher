use crate::theme::rounded;
use freya::animation::{AnimNum, Ease, Function, OnCreation, OnFinish, use_animation};
use freya::prelude::*;

const BUSY_SEGMENT: f32 = 30.;
const BUSY_TIME: u64 = 900;

#[derive(PartialEq)]
pub struct BusyTrack {
    pub height: f32,
    pub fill: Color,
    pub bg: Color,
}

impl Component for BusyTrack {
    fn render(&self) -> impl IntoElement {
        let slide = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            conf.on_finish(OnFinish::reverse());
            AnimNum::new(0., 100. - BUSY_SEGMENT)
                .time(BUSY_TIME)
                .ease(Ease::InOut)
                .function(Function::Sine)
        });
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(self.height))
            .corner_radius(rounded(self.height / 2.))
            .background(self.bg)
            .child(rect().width(Size::percent(slide.get().value())))
            .child(
                rect()
                    .width(Size::percent(BUSY_SEGMENT))
                    .height(Size::fill())
                    .corner_radius(rounded(self.height / 2.))
                    .background(self.fill),
            )
    }
}

pub fn meter(progress: (u64, u64), height: f32, fill: Color, bg: Color) -> Element {
    match progress {
        (_, 0) => BusyTrack { height, fill, bg }.into_element(),
        (current, total) => {
            let frac = (current as f32 / total as f32).clamp(0.0, 1.0);
            progress_track(frac * 100.0, height, fill, bg).into_element()
        }
    }
}

/// The app's one progress meter: a pill-shaped track with a filled portion.
///
/// `pct` is 0-100 and is clamped here, so callers can hand over a raw ratio
/// without guarding against a denominator that grew mid-flight.
pub fn progress_track(pct: f32, height: f32, fill: Color, bg: Color) -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(height))
        .corner_radius(rounded(height / 2.))
        .background(bg)
        .child(
            rect()
                .width(Size::percent(pct.clamp(0.0, 100.0)))
                .height(Size::fill())
                .corner_radius(rounded(height / 2.))
                .background(fill),
        )
}
