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

/// Shortest band is a minute nothing re-renders on a timer so a seconds counter
/// would freeze at whatever it read when the row was drawn
pub fn relative_time(created_at: Instant) -> String {
    let secs = created_at.elapsed().as_secs();
    match secs {
        0..=59 => "Just now".to_string(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86_399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

/// Returns the `Rect` not an `Element` so callers can inset or round it
pub fn divider() -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .background(theme::colors::component_border())
}

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

/// Short final rows are padded with empty flex cells so tiles keep the column width
pub fn flow_grid(items: Vec<Element>, cols: usize, mut width: State<f32>, gap: f32) -> Element {
    let cols = cols.max(1);

    let mut root = rect().vertical().width(Size::fill()).spacing(gap);
    let mut iter = items.into_iter();
    let mut remaining = true;
    while remaining {
        let mut row = rect()
            .horizontal()
            .width(Size::fill())
            .spacing(gap)
            .content(Content::Flex);
        let mut filled = 0;
        for _ in 0..cols {
            if let Some(item) = iter.next() {
                row = row.child(item);
                filled += 1;
            } else {
                row = row.child(rect().width(Size::flex(1.0)));
            }
        }
        if filled == 0 {
            break;
        }
        remaining = filled == cols;
        root = root.child(row.into_element());
    }

    root.on_sized(move |event: Event<SizedEventData>| {
        let w = event.data().area.width();
        if (w - *width.peek()).abs() > 0.5 {
            *width.write() = w;
        }
    })
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
