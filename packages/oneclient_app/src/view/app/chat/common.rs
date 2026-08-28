use chrono::{DateTime, FixedOffset, Local};
use freya::prelude::*;
use uuid::Uuid;

use crate::components::Avatar;
use crate::hooks::{settled_or_loading, use_player_profile};
use crate::theme::colors;

pub(super) const SIDEBAR_WIDTH_PX: f32 = 280.;
pub(super) const ROW_RADIUS_PX: f32 = 8.;
pub(super) fn short_id(player: Uuid) -> String {
    player.simple().to_string().chars().take(8).collect()
}

pub(super) fn use_player_name(player: Uuid) -> String {
    let profile = use_player_profile(player.to_string(), None::<String>);

    settled_or_loading(&profile)
        .map(|view| view.username)
        .unwrap_or_else(|| short_id(player))
}

pub(super) fn clock(at: DateTime<FixedOffset>) -> String {
    at.with_timezone(&Local).format("%H:%M").to_string()
}

pub(super) fn player_avatar(player: Uuid) -> Avatar {
    Avatar::new(player.to_string())
}

pub(super) fn presence_dot(online: bool) -> impl IntoElement {
    rect()
        .width(Size::px(8.))
        .height(Size::px(8.))
        .corner_radius(CornerRadius::from(4.))
        .background(if online {
            colors::success()
        } else {
            colors::fg_secondary()
        })
}

pub(super) fn section_heading(text: impl Into<String>) -> impl IntoElement {
    label()
        .text(text.into())
        .font_size(11.)
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_secondary())
}

pub(super) fn hint(text: impl Into<String>) -> impl IntoElement {
    label()
        .text(text.into())
        .font_size(12.)
        .color(colors::fg_secondary())
}
