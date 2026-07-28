use std::time::Instant;

use freya::prelude::*;

use crate::theme;

pub fn border_all(width: f32) -> Border {
    Border::new()
        .fill(theme::colors::component_border())
        .width(BorderWidth {
            top: width,
            right: width,
            bottom: width,
            left: width,
        })
}

pub fn border_all_color(width: f32, color: Color) -> Border {
    Border::new().fill(color).width(BorderWidth {
        top: width,
        right: width,
        bottom: width,
        left: width,
    })
}

pub fn fmt_date(ts: chrono::DateTime<chrono::Utc>) -> String {
    ts.format("%Y-%m-%d %H:%M").to_string()
}

/// How long ago, for a timestamp shown next to something that happened.
///
/// Nothing re-renders on a timer, so the shortest band is a whole minute: a
/// seconds counter would freeze at whatever it read when the row was drawn and
/// quietly lie from then on. The version this replaced had a seconds band and
/// no callers.
pub fn relative_time(created_at: Instant) -> String {
    let secs = created_at.elapsed().as_secs();
    match secs {
        0..=59 => "Just now".to_string(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86_399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

/// A one-pixel rule in the component-border colour.
///
/// Returns the `Rect` rather than an `Element` so a caller that wants it inset
/// or rounded can say so without copying the whole thing out again.
pub fn divider() -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .background(theme::colors::component_border())
}

/// A quiet line of text centred in a fixed block: "nothing here yet", or a
/// failure where a chart would otherwise be.
pub fn centered_note(text: &str) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::px(240.))
        .center()
        .child(
            label()
                .text(text.to_string())
                .font_size(14.)
                .color(theme::colors::fg_secondary()),
        )
        .into_element()
}

pub fn entrance_motion_layer(
    slide_x: f32,
    slide_y: f32,
    opacity: f32,
    child: impl IntoElement,
) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .position(Position::new_absolute().top(slide_y).left(slide_x))
                .opacity(opacity)
                .child(child),
        )
        .into_element()
}
