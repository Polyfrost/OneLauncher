use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::{Icon, IconType};
use crate::routes::Route;
use crate::theme::{colors, glass, rounded, rounded_px, text_size};
use crate::ui::border_all_color;

pub(super) const WIDTH: f32 = 196.;

pub(super) fn surface() -> Rect {
    let rail = rect()
        .vertical()
        .content(Content::Flex)
        .width(Size::px(WIDTH))
        .min_width(Size::px(WIDTH))
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .padding(Gaps::new_all(6.));
    if colors::active_theme().liquid_glass {
        glass::content(rail, colors::page_elevated(), rounded_px(12.))
    } else {
        rail.corner_radius(rounded(12.))
            .background(colors::page_elevated())
            .border(border_all_color(1., colors::component_border()))
    }
}

pub(super) fn select(route: &Route) {
    let _ = RouterContext::get().replace(route.clone());
}

#[derive(PartialEq)]
pub(super) struct Tab {
    pub label: &'static str,
    pub icon: IconType,
    pub route: Route,
    pub active: bool,
    pub has_dot: bool,
    pub height: f32,
    pub lens: Option<(glass::PillTrack, usize)>,
}

impl Component for Tab {
    fn render(&self) -> impl IntoElement {
        let mut hover = use_state(|| false);
        let id = use_a11y();
        let focus = use_focus(id);
        let keyboard = focus() == Focus::Keyboard;
        let active = self.active;
        let lens = self.lens;
        let highlighted = lens.map_or(active, |(track, index)| track.highlights(index, active));
        let route = self.route.clone();
        let ink = if highlighted || hover() || keyboard {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(self.height))
            .padding(Gaps::new_symmetric(0., 7.))
            .spacing(7.)
            .cross_align(Alignment::Center)
            .corner_radius(if lens.is_some() {
                CornerRadius::new_all(self.height / 2.)
            } else {
                rounded(7.)
            })
            .background(if active && lens.is_none() {
                colors::brand().with_a(if colors::is_light() { 34 } else { 67 })
            } else if hover() && !highlighted && lens.is_none() {
                colors::ghost_overlay_hover()
            } else {
                Color::TRANSPARENT
            })
            .maybe(keyboard, |el| {
                el.border(border_all_color(1., colors::fg_primary()))
            })
            .a11y_id(id)
            .a11y_role(AccessibilityRole::Button)
            .a11y_focusable(true)
            .a11y_alt(self.label)
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hover.set_if_modified(true))
            .on_pointer_leave(move |_| hover.set_if_modified(false))
            .map(lens, |el, (track, index)| track.option(el, index, true))
            .on_press(move |event: Event<PressEventData>| {
                // The vertical track only claims mouse drags. Touch and keyboard still
                // use the button's normal press handling.
                if !active && (lens.is_none() || !matches!(event.data(), PressEventData::Mouse(_)))
                {
                    select(&route);
                }
            })
            .child(Icon::new(self.icon).size(17.).color(ink))
            .child(
                label()
                    .text(self.label)
                    .width(Size::flex(1.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(text_size(13.))
                    .font_weight(if highlighted {
                        FontWeight::SEMI_BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .color(ink),
            )
            .maybe_child(self.has_dot.then(|| {
                rect()
                    .width(Size::px(4.))
                    .height(Size::px(4.))
                    .corner_radius(rounded(2.))
                    .background(colors::brand())
            }))
    }
}
