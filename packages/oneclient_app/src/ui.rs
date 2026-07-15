#![allow(dead_code)]
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

pub fn border_left(width: f32) -> Border {
    Border::new()
        .fill(theme::colors::component_border())
        .width(BorderWidth {
            left: width,
            ..Default::default()
        })
}

pub fn border_right(width: f32) -> Border {
    Border::new()
        .fill(theme::colors::component_border())
        .width(BorderWidth {
            right: width,
            ..Default::default()
        })
}

pub fn fmt_date(ts: chrono::DateTime<chrono::Utc>) -> String {
    ts.format("%Y-%m-%d %H:%M").to_string()
}

pub fn relative_time(created_at: Instant) -> String {
    let secs = created_at.elapsed().as_secs();
    match secs {
        0..=9 => "Just now".to_string(),
        10..=59 => format!("{secs}s ago"),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86_399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
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
