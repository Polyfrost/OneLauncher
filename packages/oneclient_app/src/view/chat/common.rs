use chrono::{DateTime, FixedOffset, Local};
use freya::prelude::*;
use uuid::Uuid;

use crate::components::{Avatar, Icon, IconType, overlay_is_topmost, use_open_level};
use crate::i18n::t;
use crate::theme::{colors, rounded, text_size};

pub(super) use crate::components::use_player_name;

pub(super) const LIST_WIDTH_PX: f32 = 320.;
pub(super) const ROW_RADIUS_PX: f32 = 8.;
pub(super) const CARD_RADIUS_PX: f32 = 16.;
pub(super) const CHIP_RADIUS_PX: f32 = 6.;

pub(super) fn popup_shadow() -> Shadow {
    Shadow::from((0., 8., 24., 0., Color::from_argb(89, 0, 0, 0)))
}

pub(super) fn inset_border(color: Color) -> Border {
    Border::new()
        .fill(color)
        .width(1.)
        .alignment(BorderAlignment::Inner)
}

pub(super) fn edge_border(color: Color, top: f32, right: f32, bottom: f32) -> Border {
    Border::new()
        .fill(color)
        .width(BorderWidth {
            top,
            right,
            bottom,
            left: 0.,
        })
        .alignment(BorderAlignment::Inner)
}

const ESCAPE_LEVEL: u8 = 4;

#[derive(PartialEq)]
pub(super) struct EscapeGuard {
    pub on_escape: EventHandler<()>,
}

impl Component for EscapeGuard {
    fn render(&self) -> impl IntoElement {
        use_open_level(ESCAPE_LEVEL);
        let on_escape = self.on_escape.clone();

        rect()
            .position(Position::new_absolute())
            .on_global_key_down(move |e: Event<KeyboardEventData>| {
                if e.key == Key::Named(NamedKey::Escape) && overlay_is_topmost(ESCAPE_LEVEL) {
                    on_escape.call(());
                }
            })
    }
}

pub(super) fn area_contains(area: &Area, x: f32, y: f32) -> bool {
    x >= area.min_x() && x <= area.max_x() && y >= area.min_y() && y <= area.max_y()
}

pub(super) fn clock(at: DateTime<FixedOffset>) -> String {
    at.with_timezone(&Local).format("%H:%M").to_string()
}

pub(super) fn row_time(at: DateTime<FixedOffset>) -> String {
    let local = at.with_timezone(&Local);
    let days = (Local::now().date_naive() - local.date_naive()).num_days();

    match days {
        ..=0 => local.format("%H:%M").to_string(),
        1 => t("oneclient.chat.time.yesterday").to_string(),
        2..=6 => local.format("%a").to_string(),
        _ => local.format("%d/%m").to_string(),
    }
}

pub(super) fn day_of(at: DateTime<FixedOffset>) -> chrono::NaiveDate {
    at.with_timezone(&Local).date_naive()
}

pub(super) fn day_label(at: DateTime<FixedOffset>) -> String {
    let local = at.with_timezone(&Local);
    let days = (Local::now().date_naive() - local.date_naive()).num_days();

    match days {
        ..=0 => t("oneclient.chat.time.today").to_string(),
        1 => t("oneclient.chat.time.yesterday").to_string(),
        _ => local.format("%B %-d").to_string(),
    }
}

pub(super) fn presence_color(online: bool) -> Color {
    if online {
        colors::success()
    } else {
        colors::fg_secondary()
    }
}

pub(super) fn head_with_dot(player: Uuid, size: f32, dot: Color) -> impl IntoElement {
    rect()
        .width(Size::px(size))
        .height(Size::px(size))
        .child(
            Avatar::new(player.to_string())
                .size(size)
                .radius(CHIP_RADIUS_PX),
        )
        .child(
            rect()
                .position(Position::new_absolute().right(-2.).bottom(-2.))
                .layer(Layer::Relative(2))
                .width(Size::px(10.))
                .height(Size::px(10.))
                .corner_radius(rounded(5.))
                .background(dot)
                .border(
                    Border::new()
                        .fill(colors::page())
                        .width(2.)
                        .alignment(BorderAlignment::Outer),
                ),
        )
}

pub(super) fn group_heads(members: &[Uuid]) -> impl IntoElement {
    let cell = |player: Option<&Uuid>| {
        rect()
            .width(Size::px(16.))
            .height(Size::px(16.))
            .maybe_child(player.map(|player| Avatar::new(player.to_string()).size(16.).radius(3.)))
    };

    rect()
        .vertical()
        .width(Size::px(32.))
        .height(Size::px(32.))
        .child(
            rect()
                .horizontal()
                .child(cell(members.first()))
                .child(cell(members.get(1))),
        )
        .child(
            rect()
                .horizontal()
                .child(cell(members.get(2)))
                .child(cell(members.get(3))),
        )
}

pub(super) fn section_heading(text: impl Into<String>) -> impl IntoElement {
    rect().padding(Gaps::new(12., 8., 6., 8.)).child(
        label()
            .text(text.into().to_uppercase())
            .font_size(text_size(11.))
            .font_weight(FontWeight::MEDIUM)
            .letter_spacing(0.44)
            .color(colors::fg_secondary()),
    )
}

pub(super) fn hint(text: impl Into<String>) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_symmetric(24., 8.))
        .main_align(Alignment::Center)
        .child(
            label()
                .text(text.into())
                .font_size(text_size(12.))
                .text_align(TextAlign::Center)
                .width(Size::fill())
                .color(colors::fg_secondary()),
        )
}

pub(super) fn count_badge(count: usize, background: Color, foreground: Color) -> impl IntoElement {
    rect()
        .min_width(Size::px(16.))
        .height(Size::px(16.))
        .padding(Gaps::new_symmetric(0., 4.))
        .center()
        .corner_radius(rounded(CHIP_RADIUS_PX))
        .background(background)
        .child(
            label()
                .text(count.to_string())
                .font_size(text_size(10.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(foreground),
        )
}

#[derive(PartialEq)]
pub(super) struct HeaderButton {
    pub icon: IconType,
    pub tooltip: &'static str,
    pub on_press: EventHandler<()>,
}

impl Component for HeaderButton {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_press = self.on_press.clone();
        let hot = hovered();

        rect()
            .width(Size::px(30.))
            .height(Size::px(30.))
            .center()
            .corner_radius(rounded(ROW_RADIUS_PX))
            .background(if hot {
                colors::ghost_overlay()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .on_press(move |_| on_press.call(()))
            .a11y_alt(self.tooltip)
            .child(
                Icon::new(self.icon)
                    .size(16.)
                    .color(colors::fg_primary().with_a(if hot { 255 } else { 178 })),
            )
    }
}

pub(super) fn chip_button(
    icon: IconType,
    text: &'static str,
    on_press: impl Into<EventHandler<Event<PressEventData>>>,
) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(32.))
        .center()
        .spacing(8.)
        .corner_radius(rounded(ROW_RADIUS_PX))
        .background(colors::component_bg())
        .border(inset_border(colors::component_border()))
        .cursor(CursorIcon::Pointer)
        .on_press(on_press)
        .child(Icon::new(icon).size(14.))
        .child(
            label()
                .text(text)
                .font_size(text_size(12.))
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
}
