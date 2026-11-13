use freya::prelude::*;

use crate::theme::{colors, rounded, text_size};

const RADIUS_PX: f32 = 8.;
const INSET_PX: f32 = 3.;

pub fn inset_border(color: Color) -> Border {
    Border::new()
        .fill(color)
        .width(1.)
        .alignment(BorderAlignment::Inner)
}

pub fn pill_switch(sides: [SwitchSide; 2]) -> Rect {
    let [left, right] = sides;
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .padding(Gaps::new_all(INSET_PX))
        .corner_radius(rounded(RADIUS_PX))
        .background(colors::component_bg())
        .border(inset_border(colors::component_border()))
        .child(left)
        .child(right)
}

#[derive(PartialEq)]
pub struct SwitchSide {
    pub label: &'static str,
    pub active: bool,
    pub accent: bool,
    pub on_press: EventHandler<()>,
}

impl Component for SwitchSide {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let mut hovered = use_state(|| false);
        let on_press = self.on_press.clone();

        rect()
            .width(Size::flex(1.0))
            .height(Size::fill())
            .center()
            .corner_radius(rounded(RADIUS_PX - INSET_PX))
            .background(match (self.active, hovered()) {
                (true, _) if self.accent => colors::brand(),
                (true, _) => colors::component_bg(),
                (false, true) => colors::ghost_overlay(),
                (false, false) => Color::TRANSPARENT,
            })
            .maybe(focus().is_focused(), |el| {
                el.border(inset_border(colors::fg_secondary()))
            })
            .cursor(CursorIcon::Pointer)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .on_press(move |_| on_press.call(()))
            .child(
                label()
                    .text(self.label)
                    .font_size(text_size(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(1)
                    .color(if self.active && self.accent {
                        colors::on_brand()
                    } else if self.active {
                        colors::fg_primary()
                    } else {
                        colors::fg_secondary()
                    }),
            )
    }
}
