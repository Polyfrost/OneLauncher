use std::cell::Cell;
use std::rc::Rc;

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
/// A divider's line and the margin on either side of it
const DIVIDER_HEIGHT: f32 = 1. + 2. * DIVIDER_MARGIN;
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

/// The scrolling rail. Its thumb sits in the gap beside the rail and only shows while the rail
/// is scrolled or hovered, so it never stands next to the page's own scrollbar.
pub(super) fn navigation(layout: WorkspaceLayout, rail: &RailScroll) -> ScrollArea {
    let target = rail.target.clone();
    let area = ScrollArea::new()
        .width(Size::fill())
        .height(Size::fill())
        .padding(Gaps::new(0., layout.padding, 0., 0.))
        .auto_hide_scrollbar()
        .scroll_controller(rail.controller)
        .on_ctx(move |ctx| target.set(ctx.target_y));
    // Glass dialogs have no solid surface to fade into
    if colors::active_theme().liquid_glass {
        area
    } else {
        area.bottom_fade(colors::page_elevated())
            .bottom_fade_height(FADE_HEIGHT)
    }
}

/// A rail's scroll controller and its last laid-out destination. The target is kept outside the
/// reactive state so the reveal effect can read it without re-running on every scroll.
#[derive(Clone)]
pub(super) struct RailScroll {
    controller: ScrollController,
    target: Rc<Cell<f32>>,
}

pub(super) fn use_rail_scroll() -> RailScroll {
    RailScroll {
        controller: use_scroll_controller(ScrollConfig::default),
        target: use_hook(|| Rc::new(Cell::new(0.))),
    }
}

/// The fade a rail row has to clear to read as a visible row
fn fade_clearance() -> f32 {
    if colors::active_theme().liquid_glass {
        0.
    } else {
        FADE_HEIGHT
    }
}

/// The scroll offset (negative, as the controller stores it) that brings a row spanning
/// `row_top..row_top + row_height` of the rail's content fully into a `viewport` tall rail, clear
/// of the `fade` at its bottom. `None` when the row is already visible at `scrolled`.
pub(super) fn reveal_offset(
    row_top: f32,
    row_height: f32,
    viewport: f32,
    fade: f32,
    scrolled: f32,
) -> Option<i32> {
    if viewport <= 0. {
        return None;
    }
    let shown_top = -scrolled;
    let shown_bottom = shown_top + viewport - fade;
    let row_bottom = row_top + row_height;
    if row_top < shown_top {
        Some(-(row_top.max(0.).round() as i32))
    } else if row_bottom > shown_bottom {
        // The content's end clamps this, and the fade hides there
        Some(-((row_bottom + fade - viewport).max(0.).ceil() as i32))
    } else {
        None
    }
}

/// Like `reveal_offset`, but a rail with section headings scrolls so the active row's section
/// starts at the top whenever that section's heading and the row fit together. The view then
/// opens on a clean section boundary instead of cutting through the heading above it.
pub(super) fn reveal_section_offset(
    section_top: f32,
    row_top: f32,
    row_height: f32,
    viewport: f32,
    fade: f32,
    scrolled: f32,
) -> Option<i32> {
    let offset = reveal_offset(row_top, row_height, viewport, fade, scrolled)?;
    let span = row_top + row_height - section_top;
    if section_top < row_top && span <= viewport - fade {
        Some(-(section_top.max(0.).round() as i32))
    } else {
        Some(offset)
    }
}

/// Keeps the active row of a scrolling rail in view: on mount, when the route changes and when
/// the rail is resized. A rail the user scrolled stays where they left it until then. `row` is
/// the row's section top (the row's own top in a rail without headings), its top and its height.
pub(super) fn use_reveal_active(
    rail: &RailScroll,
    route: &Route,
    row: Option<(f32, f32, f32)>,
    viewport: f32,
) {
    let fade = fade_clearance();
    // The effect keeps its first closure, so everything it needs travels in the deps
    let key = (
        route.clone(),
        row.map(|(section, top, height)| {
            (
                section.round() as i32,
                top.round() as i32,
                height.round() as i32,
            )
        }),
        viewport.round() as i32,
    );
    let mut controller = rail.controller;
    let target = rail.target.clone();
    // Only the route and the geometry re-run this: reading the controller here would subscribe
    // the effect to scrolling and snap a rail the user scrolled back to the active row
    use_side_effect_with_deps(&key, move |(_, row, viewport)| {
        let Some((section, top, height)) = *row else {
            return;
        };
        if let Some(offset) = reveal_section_offset(
            section as f32,
            top as f32,
            height as f32,
            *viewport as f32,
            fade,
            target.get(),
        ) {
            controller.scroll_to_y(offset);
        }
    });
}

/// The height a group heading takes above its first row
pub(super) fn heading_height() -> f32 {
    HEADING_HEIGHT
}

fn rows_height(count: usize, row_height: f32, row_gap: f32) -> f32 {
    count as f32 * row_height + count.saturating_sub(1) as f32 * row_gap
}

/// Where row `index` of `group` starts in a rail whose groups each open with a heading
pub(super) fn headed_row_top(
    groups: &[usize],
    group: usize,
    index: usize,
    row_height: f32,
    row_gap: f32,
) -> f32 {
    let before: f32 = groups[..group]
        .iter()
        .map(|&count| HEADING_HEIGHT + rows_height(count, row_height, row_gap))
        .sum();
    before + HEADING_HEIGHT + index as f32 * (row_height + row_gap)
}

/// Where row `index` of `group` starts in a rail whose groups are split by dividers
pub(super) fn divided_row_top(
    groups: &[usize],
    group: usize,
    index: usize,
    row_height: f32,
    row_gap: f32,
) -> f32 {
    let before: f32 = groups[..group]
        .iter()
        .map(|&count| rows_height(count, row_height, row_gap) + DIVIDER_HEIGHT)
        .sum();
    before + index as f32 * (row_height + row_gap)
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
    let rows = |count: usize| rows_height(count, row_height, row_gap);
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
    crate::view::app::settings::readable_secondary()
}

pub(super) fn clear_search(mut search: State<String>) {
    if !search.peek().is_empty() {
        search.set(String::new());
    }
}

pub(super) fn select(route: &Route) {
    #[cfg(test)]
    SELECTED.with(|cell| *cell.borrow_mut() = Some(route.clone()));
    if let Some(router) = RouterContext::try_get() {
        let _ = router.replace(route.clone());
    }
}

#[cfg(test)]
thread_local! {
    /// The last route a rail row asked for, for tests that render a rail without a router
    pub(super) static SELECTED: std::cell::RefCell<Option<Route>> =
        const { std::cell::RefCell::new(None) };
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
    /// Cleared when the row is pressed, even the active one, so Settings search results give
    /// way to the page the rail points at
    pub search: Option<State<String>>,
}

impl Component for Tab {
    fn render(&self) -> impl IntoElement {
        // The glass track brings its own hover and claims the row's pointer-leave, so the row
        // tracks hover only without a lens, and forgets it under one: a theme switch can't
        // leave a row looking hovered
        let mut hover = use_state(|| false);
        let id = use_a11y();
        let focus = use_focus(id);
        let keyboard = focus() == Focus::Keyboard;
        let active = self.active;
        let lens = self.lens;
        let highlighted = lens.map_or(active, |(track, index)| track.highlights(index, active));
        if lens.is_some() && *hover.peek() {
            hover.set(false);
        }
        let hovered = lens.is_none() && hover();
        let route = self.route.clone();
        let search = self.search;
        let ink = if highlighted || hovered || keyboard {
            colors::fg_primary()
        } else {
            secondary_ink()
        };

        row(self.height)
            .background(if active && lens.is_none() {
                colors::brand().with_a(if colors::is_light() { 34 } else { 67 })
            } else if hovered && !highlighted {
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
            .maybe(lens.is_none(), |el| {
                el.on_pointer_enter(move |_| hover.set_if_modified(true))
                    .on_pointer_leave(move |_| hover.set_if_modified(false))
            })
            .map(lens, |el, (track, index)| track.option(el, index, true))
            .on_press(move |event: Event<PressEventData>| {
                // The vertical track only claims mouse drags. Touch and keyboard still
                // use the button's normal press handling.
                if lens.is_none() || !matches!(event.data(), PressEventData::Mouse(_)) {
                    if let Some(search) = search {
                        clear_search(search);
                    }
                    if !active {
                        select(&route);
                    }
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

    #[test]
    fn row_tops_follow_headings_and_dividers() {
        // LAUNCHER opens at 0, its first row sits under the 38 px heading
        assert_eq!(headed_row_top(&SETTINGS, 0, 0, 32., 2.), 38.);
        assert_eq!(headed_row_top(&SETTINGS, 0, 5, 32., 2.), 208.);
        // ABOUT opens at 344, so Changelog starts 38 + 34 lower
        assert_eq!(headed_row_top(&SETTINGS, 2, 1, 32., 2.), 416.);
        // Groups of 2, 4 and 3 rows split by 17 px dividers
        assert_eq!(divided_row_top(&[2, 4, 3], 0, 1, 32., 2.), 34.);
        assert_eq!(
            divided_row_top(&[2, 4, 3], 2, 0, 32., 2.),
            66. + 17. + 134. + 17.
        );
    }

    #[test]
    fn the_active_row_is_scrolled_clear_of_the_fade_only_when_hidden() {
        // Visible rows leave the rail where it is
        assert_eq!(reveal_offset(38., 32., 300., 24., 0.), None);
        // A row under the fade scrolls up until it clears it
        assert_eq!(reveal_offset(416., 32., 300., 24., 0.), Some(-172));
        // A row scrolled away above comes back to the top
        assert_eq!(reveal_offset(38., 32., 300., 24., -200.), Some(-38));
        // Nothing to do before the rail is measured
        assert_eq!(reveal_offset(416., 32., 0., 24., 0.), None);
    }

    #[test]
    fn a_hidden_row_brings_its_section_heading_with_it() {
        // Row at 416 under a heading at 380: the view starts at the heading, not mid-way above it
        assert_eq!(
            reveal_section_offset(380., 416., 32., 300., 24., 0.),
            Some(-380)
        );
        // A visible row still leaves the rail alone
        assert_eq!(reveal_section_offset(0., 38., 32., 300., 24., 0.), None);
        // When the heading and the row don't fit together, only the row is brought into view
        assert_eq!(
            reveal_section_offset(100., 416., 32., 300., 24., 0.),
            reveal_offset(416., 32., 300., 24., 0.)
        );
        // A rail without headings passes the row's own top as its section
        assert_eq!(
            reveal_section_offset(416., 416., 32., 300., 24., 0.),
            reveal_offset(416., 32., 300., 24., 0.)
        );
    }

    #[test]
    fn revealing_a_row_accounts_for_the_wheel_scroll_destination() {
        let reveal = |offset| reveal_section_offset(30., 60., 32., 180., 24., offset);
        // The selection is visible now, but the wheel animation will move it above the rail.
        assert_eq!(reveal(-40.), None);
        assert_eq!(reveal(-100.), Some(-30));
        assert_eq!(reveal(-30.), None);
    }
}
