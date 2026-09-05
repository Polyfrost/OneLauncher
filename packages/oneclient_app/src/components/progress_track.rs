use freya::prelude::*;

/// The app's one progress meter: a pill-shaped track with a filled portion.
///
/// `pct` is 0-100 and is clamped here, so callers can hand over a raw ratio
/// without guarding against a denominator that grew mid-flight.
pub fn progress_track(pct: f32, height: f32, fill: Color, bg: Color) -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(height))
        .corner_radius(CornerRadius::new_all(height / 2.))
        .background(bg)
        .child(
            rect()
                .width(Size::percent(pct.clamp(0.0, 100.0)))
                .height(Size::fill())
                .corner_radius(CornerRadius::new_all(height / 2.))
                .background(fill),
        )
}
