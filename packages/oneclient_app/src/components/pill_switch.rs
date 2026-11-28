use freya::prelude::*;

use crate::theme::{colors, glass, rounded, text_size};

const RADIUS_PX: f32 = 8.;
const INSET_PX: f32 = 3.;

pub fn inset_border(color: Color) -> Border {
    Border::new()
        .fill(color)
        .width(1.)
        .alignment(BorderAlignment::Inner)
}

pub fn pill_switch(sides: [SwitchSide; 2]) -> Rect {
    rect().width(Size::fill()).child(SwitchTrack { sides })
}

#[derive(PartialEq)]
struct SwitchTrack {
    sides: [SwitchSide; 2],
}

impl Component for SwitchTrack {
    fn render(&self) -> impl IntoElement {
        let liquid = colors::active_theme().liquid_glass;
        let track = glass::use_pill_track(2);
        let active = self.sides.iter().position(|side| side.active);
        let actions = self.sides.each_ref().map(|side| side.on_press.clone());
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .padding(Gaps::new_all(INSET_PX))
            .corner_radius(if liquid {
                CornerRadius::new_all(128.)
            } else {
                rounded(RADIUS_PX)
            })
            .background(colors::component_bg())
            .border(inset_border(colors::component_border()))
            .maybe(liquid, |el| {
                track.events(el, move |index| actions[index].call(()))
            })
            .maybe_child(liquid.then(|| track.indicator(active)))
            .children(self.sides.iter().cloned().enumerate().map(|(index, side)| {
                SwitchButton {
                    side,
                    pill: liquid.then_some((track, index)),
                }
                .into_element()
            }))
    }
}

#[derive(Clone, PartialEq)]
pub struct SwitchSide {
    pub label: &'static str,
    pub active: bool,
    pub accent: bool,
    pub on_press: EventHandler<()>,
}

#[derive(PartialEq)]
struct SwitchButton {
    side: SwitchSide,
    pill: Option<(glass::PillTrack, usize)>,
}

impl Component for SwitchButton {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let mut hovered = use_state(|| false);
        let on_press = self.side.on_press.clone();
        let liquid = self.pill.is_some();

        rect()
            .width(Size::flex(1.0))
            .height(Size::fill())
            .center()
            .corner_radius(if liquid {
                CornerRadius::new_all(128.)
            } else {
                rounded(RADIUS_PX - INSET_PX)
            })
            .background(match (self.side.active, hovered()) {
                _ if liquid => Color::TRANSPARENT,
                (true, _) if self.side.accent => colors::brand(),
                (true, _) => colors::component_bg(),
                (false, true) => colors::ghost_overlay(),
                (false, false) => Color::TRANSPARENT,
            })
            .maybe(
                if liquid {
                    focus() == Focus::Keyboard
                } else {
                    focus().is_focused()
                },
                |el| el.border(inset_border(colors::fg_secondary())),
            )
            .cursor(CursorIcon::Pointer)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .map(self.pill, |el, (track, index)| {
                track.option(el, index, true)
            })
            .on_press(move |event: Event<PressEventData>| {
                if !liquid
                    || !matches!(
                        event.data(),
                        PressEventData::Mouse(_) | PressEventData::Touch(_)
                    )
                {
                    on_press.call(());
                }
            })
            .child(
                label()
                    .text(self.side.label)
                    .font_size(text_size(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(1)
                    .color(if self.side.active && self.side.accent && !liquid {
                        colors::on_brand()
                    } else if self.side.active {
                        colors::fg_primary()
                    } else {
                        colors::fg_secondary()
                    }),
            )
    }
}
