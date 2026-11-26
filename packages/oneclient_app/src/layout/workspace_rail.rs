use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::{Icon, IconType, ScrollArea};
use crate::routes::Route;
use crate::theme::{colors, glass, rounded, text_size};
use crate::ui::border_all_color;

use super::workspace_dialog::WorkspaceLayout;

const TAB_INSET: f32 = 10.;
/// Space above a group heading's label
const HEADING_TOP: f32 = 16.;
/// The heading label's line box
const HEADING_LABEL: f32 = 16.;
const HEADING_BOTTOM: f32 = 6.;
const HEADING_HEIGHT: f32 = HEADING_TOP + HEADING_LABEL + HEADING_BOTTOM;
const DIVIDER_MARGIN: f32 = 8.;
/// The fade over the bottom of a rail that scrolls
pub(super) const FADE_HEIGHT: f32 = crate::view::app::settings::SCROLL_FADE_HEIGHT;

/// The rail plus the gap to the page. The gap holds the rail's scrollbar, so the thumb never
/// sits over a row.
pub(super) fn column(layout: WorkspaceLayout) -> Rect {
    rect()
        .vertical()
        .content(Content::Flex)
        .width(Size::px(layout.rail_width + layout.padding))
        .height(Size::fill())
        .overflow(Overflow::Clip)
}

/// Fixed items under the scrolling part of a rail, kept to the rail's width
pub(super) fn footer(layout: WorkspaceLayout) -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .padding(Gaps::new(0., layout.padding, 0., 0.))
}

pub(super) fn scroll(lens: Option<glass::PillTrack>) -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::flex(1.))
        .map(lens, |el, track| track.viewport(el))
}

/// The scrolling rail. Its thumb stays visible while it overflows and sits in the gap
/// beside the rail.
pub(super) fn navigation(layout: WorkspaceLayout) -> ScrollArea {
    let area = ScrollArea::new()
        .width(Size::fill())
        .height(Size::fill())
        .padding(Gaps::new(0., layout.padding, 0., 0.));
    // Glass dialogs have no solid surface to fade into
    if colors::active_theme().liquid_glass {
        area
    } else {
        area.bottom_fade(colors::page_elevated())
            .bottom_fade_height(FADE_HEIGHT)
    }
}

/// How tall the scrolling part of a headed rail may be within `available`, given each group's
/// row count, so the visible area never ends on a heading without the first row after it. When it
/// would, the area stops just above the heading's label instead and leaves the rest blank.
pub(super) fn viewport_height(
    available: f32,
    groups: &[usize],
    row_height: f32,
    row_gap: f32,
) -> f32 {
    let rows = |count: usize| count as f32 * row_height + count.saturating_sub(1) as f32 * row_gap;
    let mut top = 0.;
    for (index, &count) in groups.iter().enumerate() {
        let label_top = top + HEADING_TOP;
        // The first row has to clear the fade by half of it to read as a row
        let first_row_clear = top + HEADING_HEIGHT + row_height + FADE_HEIGHT / 2.;
        if index > 0 && count > 0 && available > label_top && available < first_row_clear {
            return label_top;
        }
        top += HEADING_HEIGHT + rows(count);
    }
    available
}

pub(super) fn row(height: f32) -> Rect {
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .height(Size::px(height))
        .padding(Gaps::new_symmetric(0., TAB_INSET))
        .spacing(10.)
        .cross_align(Alignment::Center)
        .corner_radius(rounded(8.))
}

pub(super) fn divider() -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .margin(Gaps::new_symmetric(DIVIDER_MARGIN, 0.))
        .background(colors::component_border())
}

pub(super) fn heading(label_text: &'static str) -> Rect {
    rect()
        .padding(Gaps::new(HEADING_TOP, 0., HEADING_BOTTOM, TAB_INSET))
        .child(
            rect()
                .height(Size::px(HEADING_LABEL))
                .main_align(Alignment::Center)
                .child(
                    label()
                        .text(label_text)
                        .font_size(text_size(11.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(secondary_ink()),
                ),
        )
}

pub(super) fn secondary_ink() -> Color {
    let theme = colors::active_theme();
    if theme.id == "dark" || theme.id == "minecraft-dark" {
        colors::fg_primary().with_a(185)
    } else {
        colors::fg_secondary()
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
            secondary_ink()
        };

        row(self.height)
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
            .child(Icon::new(self.icon).size(16.).color(ink))
            .child(
                label()
                    .text(self.label)
                    .width(Size::flex(1.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(text_size(13.))
                    .font_weight(FontWeight::MEDIUM)
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

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: [usize; 3] = [6, 2, 2];

    #[test]
    fn a_rail_that_fits_keeps_its_whole_height() {
        // 3 headings, 10 rows of 32 and 7 gaps of 2
        assert_eq!(viewport_height(448., &SETTINGS, 32., 2.), 448.);
        assert_eq!(viewport_height(600., &SETTINGS, 32., 2.), 600.);
    }

    #[test]
    fn the_visible_rail_never_ends_on_a_heading_without_its_first_row() {
        // ABOUT opens at 344; its label starts 16 lower and its first row ends at 414
        for available in [361., 380., 400., 425.] {
            assert_eq!(viewport_height(available, &SETTINGS, 32., 2.), 360.);
        }
        // Inside the blank space above the label, and once the first row clears the fade
        assert_eq!(viewport_height(354., &SETTINGS, 32., 2.), 354.);
        assert_eq!(viewport_height(426., &SETTINGS, 32., 2.), 426.);
        // GAME opens at 240 and its first row clears the fade at 322
        assert_eq!(viewport_height(300., &SETTINGS, 32., 2.), 256.);
        assert_eq!(viewport_height(330., &SETTINGS, 32., 2.), 330.);
    }

    #[test]
    fn the_first_heading_never_collapses_the_rail() {
        assert_eq!(viewport_height(30., &SETTINGS, 32., 2.), 30.);
    }
}
