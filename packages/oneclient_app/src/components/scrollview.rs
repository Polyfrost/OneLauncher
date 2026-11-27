#![allow(dead_code)]
use std::time::{Duration, Instant};
use std::{cell::RefCell, collections::HashMap};

use freya::prelude::*;
use freya::torin::geometry::Point2D;

use super::home_stack::{EDGE_SETTLE, WheelEdgeChain, page_key_is_down};
use crate::theme::{colors, rounded};

#[derive(Clone, Copy, PartialEq)]
pub enum PrimaryList {
    Browse,
    Wardrobe,
}

#[derive(Clone, Copy, PartialEq)]
struct PageWheel {
    list: PrimaryList,
    delta: f32,
    smooth: bool,
}

#[derive(Clone, Copy)]
pub struct PageWheelInput(State<Option<PageWheel>>);

impl PageWheelInput {
    pub fn provide() -> Self {
        let input = Self(use_state(|| None));
        use_hook(|| provide_context(input));
        input
    }

    pub fn clear(mut self) {
        self.0.set_if_modified(None);
    }

    pub fn forward(mut self, list: PrimaryList, delta: f32, smooth: bool) {
        if delta != 0. {
            let queued = self
                .0
                .peek()
                .filter(|event| event.list == list)
                .map_or(0., |event| event.delta);
            self.0.set(Some(PageWheel {
                list,
                delta: queued + delta,
                smooth,
            }));
        }
    }
}

const LAZY_OVERSCAN: i64 = 3;
const SCROLLBAR_GUTTER: f32 = 14.;
const BOTTOM_SLOP: f32 = 2.;
const DEFAULT_FADE_HEIGHT: f32 = 18.;

const AUTOSCROLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

#[derive(Clone, Copy)]
struct AutoScroll {
    id: u64,
    anchor_y: f64,
    cursor_y: f64,
}

fn autoscroll_step(offset: f64) -> f32 {
    let displacement = offset as f32;
    let distance = (displacement.abs() - 8.).max(0.);
    -displacement.signum() * (distance * 0.17).min(42.)
}

pub(crate) fn normalize_wheel_delta(delta: f64, source: WheelSource, scale_factor: f64) -> f32 {
    let scale = if scale_factor > 0. { scale_factor } else { 1. };
    (match source {
        // Winit already converts each line to a 53-logical-pixel step.
        WheelSource::Line => delta,
        // Winit applies a 2x gain to physical touchpad pixel deltas.
        WheelSource::Pixel => delta / (2. * scale),
        // Freya's accessibility tree computes this from measured physical
        // layout geometry, without the touchpad gain.
        WheelSource::Custom => delta / scale,
    }) as f32
}

#[derive(Clone, Copy, Default)]
struct WheelRemainder {
    target: i32,
    fraction: f32,
    source: Option<WheelSource>,
    bounds: (f32, f32),
}

impl WheelRemainder {
    fn advance(
        &mut self,
        delta: f32,
        inner: f32,
        viewport: f32,
        current: i32,
        source: WheelSource,
    ) -> i32 {
        let current = corrected_scroll(inner, viewport, current as f32) as i32;
        // A Custom delta is a correction calculated from layout geometry. A
        // correction in the opposite direction supersedes any residual debt.
        let same_direction = source != WheelSource::Custom || self.fraction * delta > 0.;
        let fraction = if current == self.target
            && self.source == Some(source)
            && self.bounds == (inner, viewport)
            && same_direction
        {
            self.fraction
        } else {
            0.
        };
        let bottom = -(inner - viewport).max(0.).ceil();
        let requested = (current as f32 + fraction + delta).clamp(bottom, 0.);
        let target = current + (requested - current as f32).trunc() as i32;
        self.target = target;
        self.fraction = requested - target as f32;
        self.source = Some(source);
        self.bounds = (inner, viewport);
        target
    }
}

fn wheel_is_pinned(delta: f32, inner: f32, viewport: f32, target: i32, displayed: f32) -> bool {
    if delta > 0. {
        target >= 0 && displayed >= -0.5
    } else {
        let end = -(inner - viewport).max(0.).ceil();
        target as f32 <= end && displayed <= end + 0.5
    }
}

pub(crate) fn scroll_pos_from_wheel(wheel: f32, inner: f32, viewport: f32, current: f32) -> i32 {
    if viewport >= inner {
        return 0;
    }
    let new_pos = current + wheel;
    if new_pos >= 0.0 && wheel > 0.0 {
        return 0;
    }
    if new_pos <= -(inner - viewport) && wheel < 0.0 {
        return -(inner - viewport).ceil() as i32;
    }
    new_pos as i32
}

pub(crate) fn corrected_scroll(inner: f32, viewport: f32, pos: f32) -> f32 {
    if pos > 0.0 {
        return 0.0;
    }
    if (-pos + viewport) > inner {
        return if viewport < inner {
            -(inner - viewport).ceil()
        } else {
            0.0
        };
    }
    pos
}

pub(crate) fn scrolled_to_bottom(inner: f32, viewport: f32, target: f32) -> bool {
    -target >= inner - viewport - BOTTOM_SLOP
}

pub(crate) fn scrollbar_pos_and_size(inner: f32, viewport: f32, scroll: f32) -> (f32, f32) {
    if viewport >= inner {
        return (0.0, inner);
    }
    let ratio = viewport / inner;
    let thumb = (viewport * ratio).max(50.0);
    let scroll_range = inner - viewport;
    let thumb_range = viewport - thumb;
    let normalized = -scroll / scroll_range;

    (normalized * thumb_range, thumb)
}

#[derive(Clone, Copy)]
pub struct ScrollAreaCtx {
    pub corrected_x: f32,
    pub corrected_y: f32,
    pub target_y: f32,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub at_bottom: bool,
    pub viewport_top: State<f32>,
    pub viewport_left: State<f32>,
}

#[derive(Clone, Copy)]
pub struct LazySection {
    pub header: bool,
    pub count: usize,
}

enum LazyRow {
    Header(usize),
    Items {
        section: usize,
        row: usize,
        start: usize,
        end: usize,
    },
}

struct SectionRow {
    kind: LazyRow,
    top: f32,
    bottom: f32,
}

pub struct ScrollArea {
    width: Size,
    height: Size,
    max_height: Size,
    padding: Gaps,
    spacing: f32,
    show_scrollbar: bool,
    auto_hide_scrollbar: bool,
    scrollbar_gutter: bool,
    bottom_fade: Option<Color>,
    bottom_fade_height: f32,
    horizontal: bool,
    content_width: f32,
    stick_bottom: bool,
    anchor_key: Option<i64>,
    reset_key: Option<u64>,
    controller: Option<ScrollController>,
    primary_list: Option<PrimaryList>,
    keyboard_paging: bool,
    on_user_scroll: Option<EventHandler<()>>,
    on_overscroll_bottom: Option<EventHandler<()>>,
    on_ctx: Option<EventHandler<ScrollAreaCtx>>,
    children: Vec<Element>,
    builder: Option<Box<dyn Fn(ScrollAreaCtx) -> Element>>,
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollArea {
    pub fn new() -> Self {
        Self {
            width: Size::fill(),
            height: Size::flex(1.0),
            max_height: Size::default(),
            padding: Gaps::default(),
            spacing: 0.,
            show_scrollbar: true,
            auto_hide_scrollbar: false,
            scrollbar_gutter: false,
            bottom_fade: None,
            bottom_fade_height: DEFAULT_FADE_HEIGHT,
            horizontal: false,
            content_width: 0.,
            stick_bottom: false,
            anchor_key: None,
            reset_key: None,
            controller: None,
            primary_list: None,
            keyboard_paging: false,
            on_user_scroll: None,
            on_overscroll_bottom: None,
            on_ctx: None,
            children: Vec::new(),
            builder: None,
        }
    }

    pub fn width(mut self, width: Size) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Size) -> Self {
        self.height = height;
        self
    }

    pub fn max_height(mut self, max_height: Size) -> Self {
        self.max_height = max_height;
        self
    }

    pub fn padding(mut self, padding: Gaps) -> Self {
        self.padding = padding;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }

    pub fn auto_hide_scrollbar(mut self) -> Self {
        self.auto_hide_scrollbar = true;
        self
    }

    pub fn keyboard_paging(mut self) -> Self {
        self.keyboard_paging = true;
        self
    }

    pub fn scrollbar_gutter(mut self, gutter: bool) -> Self {
        self.scrollbar_gutter = gutter;
        self
    }

    pub fn bottom_fade(mut self, background: Color) -> Self {
        self.bottom_fade = Some(background);
        self
    }

    pub fn bottom_fade_height(mut self, height: f32) -> Self {
        self.bottom_fade_height = height;
        self
    }

    pub fn horizontal(mut self, content_width: f32) -> Self {
        self.horizontal = true;
        self.content_width = content_width;
        self
    }

    pub fn stick_bottom(mut self, stick: bool) -> Self {
        self.stick_bottom = stick;
        self
    }

    pub fn reset_key(mut self, key: u64) -> Self {
        self.reset_key = Some(key);
        self
    }

    pub fn primary_list(mut self, list: PrimaryList) -> Self {
        self.primary_list = Some(list);
        self
    }

    pub fn scroll_controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    pub fn anchor_bottom(mut self, key: Option<i64>) -> Self {
        self.anchor_key = key;
        self
    }

    pub fn on_user_scroll(mut self, handler: impl FnMut(()) + 'static) -> Self {
        self.on_user_scroll = Some(EventHandler::new(handler));
        self
    }

    /// Wheel scrolled down while already at the bottom
    pub fn on_overscroll_bottom(mut self, handler: impl FnMut(()) + 'static) -> Self {
        self.on_overscroll_bottom = Some(EventHandler::new(handler));
        self
    }

    pub fn on_ctx(mut self, handler: impl FnMut(ScrollAreaCtx) + 'static) -> Self {
        self.on_ctx = Some(EventHandler::new(handler));
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = Element>) -> Self {
        self.children = children.into_iter().collect();
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }

    pub fn append_children(mut self, children: impl IntoIterator<Item = Element>) -> Self {
        self.children.extend(children);
        self
    }

    pub fn content(mut self, builder: impl Fn(ScrollAreaCtx) -> Element + 'static) -> Self {
        self.builder = Some(Box::new(builder));
        self
    }

    pub fn lazy(
        mut self,
        count: usize,
        item_height: f32,
        spacing: f32,
        render: impl Fn(usize) -> Element + 'static,
    ) -> Self {
        let slot = (item_height + spacing).max(1.);
        let rendered = RefCell::new(None::<((usize, usize), Element)>);
        self.builder = Some(Box::new(move |ctx: ScrollAreaCtx| {
            let first =
                (((-ctx.corrected_y) / slot).floor() as i64 - LAZY_OVERSCAN).max(0) as usize;
            let first = first.min(count);
            let span = ((ctx.viewport_h / slot).ceil() as i64 + 2 * LAZY_OVERSCAN).max(0) as usize;
            let last = (first + span).min(count);

            let mut rendered = rendered.borrow_mut();
            if let Some(((last_first, last_end), element)) = rendered.as_ref()
                && (*last_first, *last_end) == (first, last)
            {
                return element.clone();
            }

            let top_pad = first as f32 * slot;
            let bottom_pad = count.saturating_sub(last) as f32 * slot;

            let mut container = rect().vertical().width(Size::fill());
            if top_pad > 0. {
                container = container.child(rect().width(Size::fill()).height(Size::px(top_pad)));
            }
            for i in first..last {
                container = container.child(
                    rect()
                        .key(i)
                        .width(Size::fill())
                        .height(Size::px(slot))
                        .child(
                            rect()
                                .width(Size::fill())
                                .height(Size::px(item_height))
                                .child(render(i)),
                        ),
                );
            }
            if bottom_pad > 0. {
                container =
                    container.child(rect().width(Size::fill()).height(Size::px(bottom_pad)));
            }
            let element = container.into_element();
            *rendered = Some(((first, last), element.clone()));
            element
        }));
        self
    }

    pub fn lazy_grid(
        mut self,
        count: usize,
        item_height: f32,
        gap: f32,
        min_width: f32,
        max_cols: usize,
        render: impl Fn(usize) -> Element + 'static,
    ) -> Self {
        let slot = (item_height + gap).max(1.);
        self.builder = Some(Box::new(move |ctx: ScrollAreaCtx| {
            let cols = (((ctx.viewport_w + gap) / (min_width + gap)).floor() as usize)
                .clamp(1, max_cols.max(1));
            let rows_total = count.div_ceil(cols);

            let first =
                (((-ctx.corrected_y) / slot).floor() as i64 - LAZY_OVERSCAN).max(0) as usize;
            let span = ((ctx.viewport_h / slot).ceil() as i64 + 2 * LAZY_OVERSCAN).max(0) as usize;
            let last = (first + span).min(rows_total);

            let top_pad = first as f32 * slot;
            let bottom_pad = rows_total.saturating_sub(last) as f32 * slot;

            let mut container = rect().vertical().width(Size::fill());
            if top_pad > 0. {
                container = container.child(rect().width(Size::fill()).height(Size::px(top_pad)));
            }
            for r in first..last {
                let mut row = rect()
                    .key(r)
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::px(slot))
                    .spacing(gap)
                    .content(Content::Flex);
                for c in 0..cols {
                    let idx = r * cols + c;
                    let cell = rect().width(Size::flex(1.0)).height(Size::px(item_height));
                    row = row.child(if idx < count {
                        cell.child(render(idx))
                    } else {
                        cell
                    });
                }
                container = container.child(row);
            }
            if bottom_pad > 0. {
                container =
                    container.child(rect().width(Size::fill()).height(Size::px(bottom_pad)));
            }
            container.into_element()
        }));
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn lazy_sections(
        mut self,
        sections: Vec<LazySection>,
        item_height: f32,
        gap: f32,
        min_width: f32,
        max_cols: usize,
        header_height: f32,
        render: impl Fn(usize) -> Element + 'static,
        render_header: impl Fn(usize) -> Element + 'static,
    ) -> Self {
        let slot = (item_height + gap).max(1.);
        let header_slot = (header_height + gap).max(1.);
        let layout = RefCell::new((0, Vec::<SectionRow>::new()));
        let mounted = RefCell::new(HashMap::<usize, Element>::new());
        let rendered = RefCell::new(None::<((usize, usize, usize), Element)>);
        self.builder = Some(Box::new(move |ctx: ScrollAreaCtx| {
            let cols = (((ctx.viewport_w + gap) / (min_width + gap)).floor() as usize)
                .clamp(1, max_cols.max(1));

            let mut layout = layout.borrow_mut();
            let mut mounted = mounted.borrow_mut();
            if layout.0 != cols {
                mounted.clear();
                let mut offset = 0;
                let mut top = 0.;
                let mut rows = Vec::new();
                let mut push = |kind, height| {
                    let bottom = top + height;
                    rows.push(SectionRow { kind, top, bottom });
                    top = bottom;
                };
                for (section_index, section) in sections.iter().enumerate() {
                    if section.header {
                        push(LazyRow::Header(section_index), header_slot);
                    }
                    for (row, start) in (0..section.count).step_by(cols).enumerate() {
                        push(
                            LazyRow::Items {
                                section: section_index,
                                row,
                                start: offset + start,
                                end: offset + (start + cols).min(section.count),
                            },
                            slot,
                        );
                    }
                    offset += section.count;
                }
                *layout = (cols, rows);
            }
            let rows = &layout.1;

            let overscan = LAZY_OVERSCAN as f32 * slot;
            let visible_top = -ctx.corrected_y - overscan;
            let visible_bottom = -ctx.corrected_y + ctx.viewport_h + overscan;
            let first = rows.partition_point(|row| row.bottom <= visible_top);
            let last = rows
                .partition_point(|row| row.top < visible_bottom)
                .max(first);
            let key = (cols, first, last);

            let mut rendered = rendered.borrow_mut();
            if let Some((prev_key, element)) = rendered.as_ref()
                && *prev_key == key
            {
                return element.clone();
            }

            let total_height = rows.last().map_or(0., |row| row.bottom);
            let top_pad = rows.get(first).map_or(total_height, |row| row.top);
            let bottom_pad = total_height - rows.get(last).map_or(total_height, |row| row.top);
            let mut container = rect()
                .vertical()
                .width(Size::fill())
                .child(rect().width(Size::fill()).height(Size::px(top_pad)));

            // Reuse rows in the intersection. Holding only the current range keeps images
            // and event handlers bounded even after a long scroll.
            mounted.retain(|index, _| (first..last).contains(index));
            for (index, row) in (first..last).zip(&rows[first..last]) {
                let element = mounted.entry(index).or_insert_with(|| match row.kind {
                    LazyRow::Header(section) => rect()
                        .key(("h", section))
                        .width(Size::fill())
                        .height(Size::px(header_slot))
                        .child(
                            rect()
                                .width(Size::fill())
                                .height(Size::px(header_height))
                                .child(render_header(section)),
                        )
                        .into_element(),
                    LazyRow::Items {
                        section,
                        row,
                        start,
                        end,
                    } => {
                        let mut items = rect()
                            .key(("r", section, row))
                            .horizontal()
                            .width(Size::fill())
                            .height(Size::px(slot))
                            .spacing(gap)
                            .content(Content::Flex);
                        for idx in start..start + cols {
                            let cell = rect().width(Size::flex(1.0)).height(Size::px(item_height));
                            items = items.child(if idx < end {
                                cell.child(render(idx))
                            } else {
                                cell
                            });
                        }
                        items.into_element()
                    }
                });
                container = container.child(element.clone());
            }
            if bottom_pad > 0. {
                container =
                    container.child(rect().width(Size::fill()).height(Size::px(bottom_pad)));
            }
            let element = container.into_element();
            *rendered = Some((key, element.clone()));
            element
        }));
        self
    }

    fn view(&self) -> impl IntoElement {
        let horizontal = self.horizontal;
        let content_w = self.content_width;
        let spacing = self.spacing;
        let padding = self.padding;
        let show_scrollbar = self.show_scrollbar;
        let auto_hide_scrollbar = self.auto_hide_scrollbar;
        let scrollbar_gutter = self.scrollbar_gutter;
        let stick_bottom = self.stick_bottom;
        let anchor_key = self.anchor_key;
        let on_user_scroll = self.on_user_scroll.clone();
        let on_overscroll_bottom = self.on_overscroll_bottom.clone();

        let internal = use_scroll_controller(ScrollConfig::default);
        let mut controller = self.controller.unwrap_or(internal);

        let scale_factor = Platform::get().current_window().scale_factor;
        let edge_chain = use_hook(try_consume_context::<WheelEdgeChain>);
        let wheel_claims = use_hook(crate::hooks::use_overlay_claims);
        let mut wheel_x = use_state(WheelRemainder::default);
        let mut wheel_y = use_state(WheelRemainder::default);

        let mut viewport_h = use_state(|| 0f32);
        let mut viewport_w = use_state(|| 0f32);
        let mut viewport_top = use_state(|| 0f32);
        let mut viewport_left = use_state(|| 0f32);
        let mut content_h = use_state(|| 0f32);
        let mut anchored = use_state(|| anchor_key);
        let mut drag_start = use_state::<Option<(f32, i32)>>(|| None);
        let mut drag_start_x = use_state::<Option<(f32, f32)>>(|| None);
        let mut hover_tracks = use_state(|| (false, false));
        let mut shift_held = use_state(|| false);
        let mut auto_scroll = use_state(|| None::<AutoScroll>);
        let mut auto_scroll_id = use_state(|| 0u64);
        let app_focused = Platform::get().current_window().is_app_focused;

        let reset_key = self.reset_key;
        let mut last_reset = use_state(|| reset_key);
        let mut reset_fired = false;
        if reset_key.is_some() && *last_reset.peek() != reset_key {
            last_reset.set(reset_key);
            controller.stop();
            controller.scroll_to_x(0);
            controller.scroll_to_y(0);
            wheel_x.set(WheelRemainder::default());
            wheel_y.set(WheelRemainder::default());
            reset_fired = true;
        }

        let mut was_sticking = use_state(|| stick_bottom);
        if stick_bottom && !*was_sticking.peek() && !reset_fired {
            let target = -((*content_h.peek() - *viewport_h.peek()).max(0.));
            controller.scroll_to_y(target as i32);
        }
        was_sticking.set_if_modified(stick_bottom);

        let (scrolled_x, scrolled_y) = controller.into();
        let vp_h = *viewport_h.read();
        let vp_w = *viewport_w.read();
        let ct_h = *content_h.read();

        let target_y = corrected_scroll(ct_h, vp_h, scrolled_y as f32);
        let target_x = if horizontal {
            corrected_scroll(content_w, vp_w, scrolled_x as f32)
        } else {
            0.
        };
        let animated = controller.animated_position(Point2D::new(target_x, target_y));
        let corrected_y = corrected_scroll(ct_h, vp_h, animated.y);
        let corrected_x = if horizontal {
            corrected_scroll(content_w, vp_w, animated.x)
        } else {
            0.
        };

        let (scrollbar_y, scrollbar_height) = scrollbar_pos_and_size(ct_h, vp_h, corrected_y);
        let show_v = show_scrollbar && vp_h > 0. && vp_h < ct_h;

        let (scrollbar_x, scrollbar_width) = scrollbar_pos_and_size(content_w, vp_w, corrected_x);
        let show_h = show_scrollbar && horizontal && vp_w > 0. && vp_w < content_w;

        let pressing_v = use_memo(move || drag_start.read().is_some());
        let pressing_h = use_memo(move || drag_start_x.read().is_some());
        let (hovering_h, hovering_v) = *hover_tracks.read();

        let gutter = if show_v && scrollbar_gutter {
            SCROLLBAR_GUTTER
        } else {
            0.
        };
        let padding = if gutter > 0. {
            Gaps::new(
                padding.top(),
                padding.right() + gutter,
                padding.bottom(),
                padding.left(),
            )
        } else {
            padding
        };

        let ctx = ScrollAreaCtx {
            corrected_x,
            corrected_y,
            target_y,
            viewport_w: (vp_w - gutter).max(0.),
            viewport_h: vp_h,
            at_bottom: scrolled_to_bottom(ct_h, vp_h, target_y),
            viewport_top,
            viewport_left,
        };

        if let Some(cb) = &self.on_ctx {
            cb.call(ctx);
        }

        let content_el = if let Some(builder) = &self.builder {
            let inner = builder(ctx);
            if padding != Gaps::default() {
                rect()
                    .width(Size::fill())
                    .padding(padding)
                    .child(inner)
                    .into_element()
            } else {
                inner
            }
        } else {
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(spacing)
                .padding(padding)
                .children(self.children.clone())
                .into_element()
        };

        let us_wheel = on_user_scroll.clone();
        let us_thumb = on_user_scroll.clone();
        let us_h_thumb = on_user_scroll.clone();
        let us_key = on_user_scroll.clone();
        let us_auto = on_user_scroll;

        let input = use_hook(try_consume_context::<PageWheelInput>);
        let pending = self.primary_list.and_then(|list| {
            input.and_then(|input| (*input.0.read()).filter(|wheel| wheel.list == list))
        });
        let ready = vp_h > 0. && ct_h > 0.;
        let forwarded_scroll = us_wheel.clone();
        use_side_effect_with_deps(&(pending, ready), move |&(pending, ready)| {
            let (Some(input), Some(event), true) = (input, pending, ready) else {
                return;
            };
            if *input.0.peek() != Some(event) {
                return;
            }
            input.clear();
            // The effect outlives its first render, when neither area has a height yet.
            let vp_h = *viewport_h.peek();
            let ct_h = *content_h.peek();
            let source = if event.smooth {
                WheelSource::Line
            } else {
                controller.stop();
                WheelSource::Pixel
            };
            let (x, y) = controller.into();
            let current = corrected_scroll(ct_h, vp_h, y as f32);
            let next = wheel_y.write().advance(event.delta, ct_h, vp_h, y, source);
            if next == y {
                return;
            }
            if let Some(cb) = &forwarded_scroll {
                cb.call(());
            }
            if event.smooth {
                let position = controller.animated_position(Point2D::new(x as f32, current));
                controller.animate_from(position);
            }
            controller.scroll_to_y(next);
        });

        let can_scroll_v = vp_h > 0. && vp_h < ct_h;
        let can_scroll_h = horizontal && vp_w > 0. && vp_w < content_w;
        let page_key_claims = wheel_claims.clone();
        let on_page_key = move |event: Event<KeyboardEventData>| {
            if page_key_claims.any() || !can_scroll_v {
                return;
            }
            let Some(down) = page_key_is_down(event.data()) else {
                return;
            };
            let distance = if down { -vp_h } else { vp_h };
            let (_, target) = controller.into();
            if wheel_is_pinned(distance, ct_h, vp_h, target, corrected_y) {
                return;
            }
            // Give the owning scroll area priority. At the edge, the event can
            // bubble to the Home/Versions pane and change pages.
            event.prevent_default();
            event.stop_propagation();
            controller.stop();
            wheel_y.set(WheelRemainder::default());
            let (_, current) = controller.into();
            let next = scroll_pos_from_wheel(distance, ct_h, vp_h, current as f32);
            controller.scroll_to_y(next);
            if let Some(cb) = &us_key {
                cb.call(());
            }
        };

        let on_wheel = move |e: Event<WheelEventData>| {
            let shift = e.source.is_device() && *shift_held.read();
            let scale = *scale_factor.peek();

            let delta_x = normalize_wheel_delta(e.delta_x, e.source, scale);
            let delta_y = normalize_wheel_delta(e.delta_y, e.source, scale);

            let sideways = shift || !can_scroll_v;
            let h_delta = if horizontal {
                if sideways && delta_x == 0.0 {
                    delta_y
                } else {
                    delta_x
                }
            } else {
                0.0
            };
            let v_delta = if horizontal && sideways { 0.0 } else { delta_y };

            let do_h = h_delta != 0.0 && can_scroll_h;
            let do_v = v_delta != 0.0 && can_scroll_v;

            // Nested modals and accessibility scrolls own their scroll views; the
            // underlying Home stack may only take an unclaimed physical gesture.
            let edge_chain = edge_chain.filter(|_| e.source.is_device() && !wheel_claims.any());
            if edge_chain.is_some_and(|chain| *chain.pass_all.peek() || chain.coasting()) {
                return;
            }

            if !do_h && !do_v {
                return;
            }
            let (_, target_y) = controller.into();
            if edge_chain.is_some()
                && do_v
                && !do_h
                && wheel_is_pinned(v_delta, ct_h, vp_h, target_y, corrected_y)
            {
                wheel_y.set(WheelRemainder::default());
                if let Some(mut chain) = edge_chain {
                    let settled = chain
                        .inner_wheel_at
                        .peek()
                        .is_none_or(|at| at.elapsed() >= EDGE_SETTLE);
                    if !settled {
                        chain.inner_wheel_at.set(Some(std::time::Instant::now()));
                        e.stop_propagation();
                    }
                }
                return;
            }
            if let Some(mut chain) = edge_chain {
                chain.inner_wheel_at.set(Some(std::time::Instant::now()));
            }

            // Retain fractional touchpad motion even when this event cannot move
            // the integer controller yet, so it never leaks into the Home stack.
            e.stop_propagation();

            if e.source != WheelSource::Line {
                // Stopping an animation commits its live position to the controller.
                // Both the delta and the equality check must use that new position.
                controller.stop();
            }
            let (cur_x, cur_y) = controller.into();

            let next_x = if do_h {
                wheel_x
                    .write()
                    .advance(h_delta, content_w, vp_w, cur_x, e.source)
            } else {
                cur_x
            };
            let next_y = if do_v {
                wheel_y
                    .write()
                    .advance(v_delta, ct_h, vp_h, cur_y, e.source)
            } else {
                cur_y
            };
            if do_v
                && v_delta < 0.
                && e.source.is_device()
                && cur_y as f32 <= -(ct_h - vp_h).ceil() + 0.5
                && let Some(cb) = &on_overscroll_bottom
            {
                cb.call(());
            }
            if next_x == cur_x && next_y == cur_y {
                return;
            }
            if let Some(cb) = &us_wheel {
                cb.call(());
            }

            if e.source == WheelSource::Line {
                controller.animate_from(Point2D::new(corrected_x, corrected_y));
            }

            if next_x != cur_x {
                controller.scroll_to_x(next_x);
            }
            if next_y != cur_y {
                controller.scroll_to_y(next_y);
            }
        };

        let on_thumb_down = move |e: Event<PointerEventData>| {
            wheel_y.set(WheelRemainder::default());
            if let Some(cb) = &us_thumb {
                cb.call(());
            }
            controller.stop();
            let (_, y) = controller.into();
            drag_start.set(Some((e.global_location().y as f32, y)));
        };

        let on_h_thumb_down = move |e: Event<PointerEventData>| {
            wheel_x.set(WheelRemainder::default());
            if let Some(cb) = &us_h_thumb {
                cb.call(());
            }
            controller.stop();
            let (x, _) = controller.into();
            drag_start_x.set(Some((e.global_location().x as f32, x as f32)));
        };

        let on_middle_down = move |e: Event<PointerEventData>| {
            if !can_scroll_v || e.button() != Some(MouseButton::Middle) {
                return;
            }
            e.stop_propagation();
            wheel_y.set(WheelRemainder::default());
            let id = auto_scroll_id.peek().wrapping_add(1);
            auto_scroll_id.set(id);
            let started_focused = *app_focused.peek();
            let anchor_y = e.global_location().y;
            auto_scroll.set(Some(AutoScroll {
                id,
                anchor_y,
                cursor_y: anchor_y,
            }));
            controller.stop();

            let on_user_scroll = us_auto.clone();
            spawn_in_window(async move {
                let mut motion = 0.;
                let mut notified = false;
                loop {
                    tokio::time::sleep(AUTOSCROLL_INTERVAL).await;
                    let Some(held) = auto_scroll.try_peek().and_then(|value| *value) else {
                        break;
                    };
                    if held.id != id {
                        break;
                    }
                    if started_focused && !*app_focused.peek() {
                        auto_scroll.set(None);
                        break;
                    }
                    let step = autoscroll_step(held.cursor_y - held.anchor_y);
                    if step == 0. {
                        motion = 0.;
                        continue;
                    }
                    motion += step;
                    let pixels = motion.trunc();
                    motion -= pixels;
                    if pixels == 0. {
                        continue;
                    }
                    let inner = *content_h.peek();
                    let viewport = *viewport_h.peek();
                    let (_, y) = controller.into();
                    let current = corrected_scroll(inner, viewport, y as f32);
                    let next = scroll_pos_from_wheel(pixels, inner, viewport, current);
                    if next != y {
                        if !notified {
                            if let Some(cb) = &on_user_scroll {
                                cb.call(());
                            }
                            notified = true;
                        }
                        controller.scroll_to_y(next);
                    }
                }
            });
        };

        let on_global_move = move |e: Event<PointerEventData>| {
            let held = *auto_scroll.peek();
            if let Some(mut held) = held {
                held.cursor_y = e.global_location().y;
                auto_scroll.set(Some(held));
                e.stop_propagation();
                return;
            }
            if let Some((grab_y, scroll_at_grab)) = *drag_start.read() {
                e.stop_propagation();

                let thumb_range = (vp_h - scrollbar_height).max(1.);
                let scroll_range = (ct_h - vp_h).max(0.);

                let cursor_dy = e.global_location().y as f32 - grab_y;
                let scroll_dy = -(cursor_dy / thumb_range) * scroll_range;
                let target = scroll_at_grab + scroll_dy as i32;

                let clamped = corrected_scroll(ct_h, vp_h, target as f32) as i32;

                let (_, cur_y) = controller.into();

                if clamped != cur_y {
                    controller.scroll_to_y(clamped);
                }

                return;
            }
            if let Some((grab_x, scroll_at_grab)) = *drag_start_x.read() {
                e.stop_propagation();
                let thumb_range = (vp_w - scrollbar_width).max(1.);
                let scroll_range = (content_w - vp_w).max(0.);
                let cursor_dx = e.global_location().x as f32 - grab_x;
                let scroll_dx = -(cursor_dx / thumb_range) * scroll_range;
                let target = scroll_at_grab + scroll_dx;
                controller.scroll_to_x(corrected_scroll(content_w, vp_w, target) as i32);
            }
        };

        let on_viewport_move = move |e: Event<PointerEventData>| {
            let point = e.global_location();
            let x = point.x as f32 - *viewport_left.peek();
            let y = point.y as f32 - *viewport_top.peek();
            hover_tracks.set_if_modified((
                show_h && y >= vp_h - 10. && y < vp_h,
                show_v && x >= vp_w - SCROLLBAR_GUTTER && x < vp_w,
            ));
        };

        let on_global_release = move |e: Event<PointerEventData>| {
            if e.button() == Some(MouseButton::Middle) && auto_scroll.peek().is_some() {
                auto_scroll.set(None);
                e.stop_propagation();
            }
            if drag_start.read().is_some() {
                drag_start.set(None);
            }
            if drag_start_x.read().is_some() {
                drag_start_x.set(None);
            }
        };

        rect()
            .width(self.width.clone())
            .height(self.height.clone())
            .max_height(self.max_height.clone())
            .scrollable(true)
            .a11y_builder(move |node| {
                node.set_scroll_x(corrected_x as f64);
                node.set_scroll_y(corrected_y as f64);
            })
            .maybe(auto_scroll.read().is_some(), |el| {
                el.cursor(CursorIcon::NsResize)
            })
            .on_pointer_down(on_middle_down)
            .on_wheel(on_wheel)
            .maybe(self.keyboard_paging, |el| el.on_key_down(on_page_key))
            .on_capture_global_pointer_move(on_global_move)
            .on_capture_global_pointer_up(on_global_release)
            .on_global_key_down(move |e: Event<KeyboardEventData>| {
                let held = e.modifiers.contains(Modifiers::SHIFT)
                    || matches!(e.code, Code::ShiftLeft | Code::ShiftRight);
                shift_held.set_if_modified(held);
            })
            .on_global_key_up(move |e: Event<KeyboardEventData>| {
                let held = if matches!(e.code, Code::ShiftLeft | Code::ShiftRight) {
                    false
                } else {
                    e.modifiers.contains(Modifiers::SHIFT)
                };
                shift_held.set_if_modified(held);
            })
            .child(
                rect()
                    .width(Size::fill())
                    .height(if self.height == Size::Inner {
                        Size::Inner
                    } else {
                        Size::flex(1.0)
                    })
                    .max_height(self.max_height.clone())
                    .overflow(Overflow::Clip)
                    .maybe(auto_hide_scrollbar, |el| {
                        el.on_pointer_move(on_viewport_move)
                            .on_pointer_leave(move |_| hover_tracks.set_if_modified((false, false)))
                    })
                    .on_sized(move |e: Event<SizedEventData>| {
                        let h = e.area.height();
                        let w = e.area.width();
                        let top = e.area.min_y();
                        let left = e.area.min_x();
                        if (*viewport_h.read() - h).abs() > 0.5 {
                            viewport_h.set(h);
                            if stick_bottom {
                                let target = -((*content_h.read() - h).max(0.).ceil());
                                controller.scroll_to_y(target as i32);
                            }
                        }
                        if (*viewport_w.read() - w).abs() > 0.5 {
                            viewport_w.set(w);
                        }
                        if (*viewport_top.read() - top).abs() > 0.5 {
                            viewport_top.set(top);
                        }
                        if (*viewport_left.read() - left).abs() > 0.5 {
                            viewport_left.set(left);
                        }
                    })
                    .child(
                        rect()
                            .width(Size::fill())
                            .offset_x(corrected_x)
                            .offset_y(corrected_y)
                            .on_sized(move |e: Event<SizedEventData>| {
                                let h = e.area.height();
                                let grown = h - *content_h.read();
                                if grown.abs() > 0.5 {
                                    content_h.set(h);
                                    if stick_bottom {
                                        let vp = *viewport_h.read();
                                        let target = -((h - vp).max(0.).ceil());
                                        controller.scroll_to_y(target as i32);
                                    } else if *anchored.peek() != anchor_key {
                                        let (_, y) = controller.into();
                                        controller.stop();
                                        controller.scroll_to_y(y - grown.round() as i32);
                                    }
                                }
                                anchored.set_if_modified(anchor_key);
                            })
                            .child(content_el),
                    )
                    .map(
                        self.bottom_fade.filter(|_| {
                            can_scroll_v && !scrolled_to_bottom(ct_h, vp_h, corrected_y)
                        }),
                        |el, surface| {
                            el.child(
                                rect()
                                    .position(
                                        Position::new_absolute()
                                            .bottom(0.)
                                            .left(0.)
                                            .right(if show_v { SCROLLBAR_GUTTER } else { 0. }),
                                    )
                                    .height(Size::px(self.bottom_fade_height))
                                    .width(Size::fill())
                                    .layer(Layer::Relative(9))
                                    .background(
                                        LinearGradient::new()
                                            .stop((surface.with_a(0), 0.))
                                            .stop((surface, 100.)),
                                    ),
                            )
                        },
                    )
                    .maybe_child(show_h.then(|| {
                        ScrollThumb {
                            vertical: false,
                            auto_hide: auto_hide_scrollbar,
                            track_hovered: hovering_h,
                            main_pos: scrollbar_x,
                            length: scrollbar_width,
                            pressing: pressing_h.into_readable(),
                            on_down: on_h_thumb_down.into(),
                        }
                        .into_element()
                    }))
                    .maybe_child(show_v.then(|| {
                        ScrollThumb {
                            vertical: true,
                            auto_hide: auto_hide_scrollbar,
                            track_hovered: hovering_v,
                            main_pos: scrollbar_y,
                            length: scrollbar_height,
                            pressing: pressing_v.into_readable(),
                            on_down: on_thumb_down.into(),
                        }
                        .into_element()
                    })),
            )
    }
}

impl PartialEq for ScrollArea {
    fn eq(&self, other: &Self) -> bool {
        if self.builder.is_some() || other.builder.is_some() {
            return false;
        }
        self.width == other.width
            && self.height == other.height
            && self.max_height == other.max_height
            && self.padding == other.padding
            && self.spacing == other.spacing
            && self.show_scrollbar == other.show_scrollbar
            && self.auto_hide_scrollbar == other.auto_hide_scrollbar
            && self.scrollbar_gutter == other.scrollbar_gutter
            && self.bottom_fade == other.bottom_fade
            && self.bottom_fade_height == other.bottom_fade_height
            && self.horizontal == other.horizontal
            && self.content_width == other.content_width
            && self.stick_bottom == other.stick_bottom
            && self.anchor_key == other.anchor_key
            && self.primary_list == other.primary_list
            && self.reset_key == other.reset_key
            && self.children == other.children
    }
}

impl Component for ScrollArea {
    fn render(&self) -> impl IntoElement {
        self.view()
    }
}

#[derive(PartialEq)]
struct ScrollThumb {
    vertical: bool,
    auto_hide: bool,
    track_hovered: bool,
    main_pos: f32,
    length: f32,
    pressing: Readable<bool>,
    on_down: EventHandler<Event<PointerEventData>>,
}

const SCROLLBAR_IDLE: Duration = Duration::from_millis(650);
const SCROLLBAR_FADE: Duration = Duration::from_millis(160);

fn use_scrollbar_visibility(auto_hide: bool, position: f32, held: bool) -> f32 {
    let mut previous = use_state(|| position);
    let mut opacity = use_state(|| if auto_hide { 0. } else { 1. });
    let mut pending = use_state(|| None::<OwnedTaskHandle>);
    use_side_effect_with_deps(
        &(auto_hide, position, held),
        move |&(auto_hide, position, held)| {
            pending.set(None);
            let moved = *previous.peek() != position;
            previous.set_if_modified(position);
            if !auto_hide || held || moved {
                opacity.set_if_modified(1.);
            }
            if !auto_hide || held || *opacity.peek() == 0. {
                return;
            }

            let clock = AnimationClock::get();
            let task = spawn(async move {
                tokio::time::sleep(SCROLLBAR_IDLE).await;
                let start = Instant::now();
                let from = *opacity.peek();
                let ticker = RenderingTicker::get();
                let window = Platform::get().current_window();
                loop {
                    let elapsed = clock.correct_elapsed_duration(start.elapsed());
                    let remaining =
                        (1. - elapsed.as_secs_f32() / SCROLLBAR_FADE.as_secs_f32()).max(0.);
                    opacity.set_if_modified(from * remaining);
                    if remaining == 0. {
                        break;
                    }
                    window.request_redraw();
                    ticker.tick().await;
                }
            });
            pending.set(Some(task.owned()));
        },
    );
    *opacity.read()
}

impl Component for ScrollThumb {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let on_down = self.on_down.clone();
        let pressing = *self.pressing.read();
        let hovered = self.track_hovered || *hovering.read();
        let visibility =
            use_scrollbar_visibility(self.auto_hide, self.main_pos, pressing || hovered);

        let color = if pressing {
            colors::fg_secondary_pressed()
        } else if hovered {
            colors::fg_secondary_hover()
        } else {
            colors::fg_secondary()
        };
        let (thickness, opacity) = if pressing || hovered {
            (8., 160)
        } else if colors::active_theme().liquid_glass {
            (3., 96)
        } else {
            (6., 160)
        };

        // Stay inside a modal's paint band so a scrollbar cannot overdraw a newer dialog.
        let layer = Layer::Relative(if use_hook(super::overlay_popup::is_inside_popup) {
            48
        } else {
            999
        });
        let (position, width, height) = if self.vertical {
            (
                Position::new_absolute().right(0.).top(self.main_pos),
                Size::px(14.),
                Size::px(self.length),
            )
        } else {
            (
                Position::new_absolute().left(self.main_pos).bottom(2.),
                Size::px(self.length),
                Size::px(8.),
            )
        };

        let (bar_w, bar_h) = if self.vertical {
            (Size::px(thickness), Size::fill())
        } else {
            (Size::fill(), Size::px(thickness))
        };

        rect()
            .position(position)
            .width(width)
            .height(height)
            .center()
            .interactive(visibility > 0.)
            .layer(layer)
            .on_pointer_down(move |e| on_down.call(e))
            .on_pointer_over(move |_| hovering.set_if_modified(true))
            .on_pointer_out(move |_| hovering.set_if_modified(false))
            .child(
                rect()
                    .width(bar_w)
                    .height(bar_h)
                    .interactive(false)
                    .corner_radius(rounded(12.))
                    .background(color.with_a((opacity as f32 * visibility).round() as u8)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_auto_hidden_scrollbar_reappears_on_motion_and_stays_visible_while_held() {
        use freya_testing::prelude::{
            MouseEventName, PlatformEvent, TestingRunner, WheelEventName,
        };
        use std::{cell::Cell, rc::Rc};

        fn app() -> impl IntoElement {
            let observed = use_consume::<Rc<Cell<f32>>>();
            let mut position = use_state(|| 0.);
            let mut held = use_state(|| false);
            observed.set(use_scrollbar_visibility(true, position(), held()));
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .on_wheel(move |_: Event<WheelEventData>| position.set(25.))
                .on_pointer_down(move |_| held.set(true))
                .on_global_pointer_up(move |_| held.set(false))
        }

        let (mut runner, observed) = TestingRunner::new(
            app,
            Size2D::new(320., 240.),
            |runner| runner.provide_root_context(|| Rc::new(Cell::new(0f32))),
            1.,
        );
        runner.poll_n(Duration::ZERO, 5);
        assert_eq!(observed.get(), 0.);

        runner.send_event(PlatformEvent::Wheel {
            name: WheelEventName::Wheel,
            source: WheelSource::Line,
            phase: TouchPhase::Moved,
            scroll: (0., -53.).into(),
            cursor: (40., 40.).into(),
        });
        runner.poll_n(Duration::ZERO, 5);
        assert_eq!(observed.get(), 1.);

        let mouse = |name| PlatformEvent::Mouse {
            name,
            cursor: (40., 40.).into(),
            button: Some(MouseButton::Left),
        };
        runner.send_event(mouse(MouseEventName::MouseDown));
        runner.poll_n(Duration::ZERO, 5);
        tokio::time::sleep(SCROLLBAR_IDLE + SCROLLBAR_FADE).await;
        runner.poll_n(Duration::ZERO, 5);
        assert_eq!(observed.get(), 1.);

        runner.send_event(mouse(MouseEventName::MouseUp));
        runner.poll_n(Duration::ZERO, 5);
        for _ in 0..12 {
            tokio::time::sleep(Duration::from_millis(80)).await;
            runner.poll_n(Duration::ZERO, 2);
            if observed.get() == 0. {
                break;
            }
        }
        assert_eq!(observed.get(), 0.);
    }

    #[test]
    fn wheel_source_controls_the_units() {
        assert_eq!(normalize_wheel_delta(53., WheelSource::Line, 2.), 53.);
        assert_eq!(normalize_wheel_delta(53., WheelSource::Pixel, 2.), 13.25);
        assert_eq!(normalize_wheel_delta(53., WheelSource::Custom, 2.), 26.5);
    }

    #[test]
    fn custom_wheel_keeps_fractions_until_direction_or_bounds_change() {
        let mut wheel = WheelRemainder::default();
        let source = WheelSource::Custom;
        assert_eq!(wheel.advance(-0.5, 400., 100., -20, source), -20);
        assert_eq!(wheel.advance(-0.5, 400., 100., -20, source), -21);
        assert_eq!(wheel.advance(-0.5, 400., 100., -21, source), -21);
        assert_eq!(wheel.advance(1.25, 400., 100., -21, source), -20);
        assert_eq!(wheel.advance(0.5, 400., 100., -20, source), -20);
        assert_eq!(wheel.advance(0.5, 400., 120., -20, source), -20);
    }

    #[test]
    fn immediate_scroll_interrupts_a_line_wheel_from_its_live_position() {
        use freya_testing::prelude::{PlatformEvent, TestingRunner, WheelEventName};
        use std::{cell::Cell, rc::Rc};

        fn app() -> impl IntoElement {
            let measurement = use_consume::<Rc<Cell<f32>>>();
            ScrollArea::new()
                .height(Size::px(120.))
                .show_scrollbar(false)
                .child(rect().width(Size::fill()).height(Size::px(600.)))
                .on_ctx(move |ctx| measurement.set(ctx.corrected_y))
        }

        for (source, raw_delta) in [(WheelSource::Pixel, -106.), (WheelSource::Custom, -53.)] {
            let (mut runner, measurement) = TestingRunner::new(
                app,
                Size2D::new(600., 440.),
                |runner| runner.provide_root_context(|| Rc::new(Cell::new(0f32))),
                1.,
            );
            let event = |source, y| PlatformEvent::Wheel {
                name: WheelEventName::Wheel,
                source,
                phase: TouchPhase::Moved,
                scroll: (0., y).into(),
                cursor: (20., 20.).into(),
            };
            runner.poll_n(std::time::Duration::ZERO, 5);
            runner.send_event(event(WheelSource::Line, -53.));
            runner.sync_and_update();
            let live = measurement.get();
            assert_eq!(live, 0.);

            runner.send_event(event(source, raw_delta));
            runner.sync_and_update();
            assert_eq!(measurement.get(), -53.);
        }
    }

    #[test]
    fn fractional_wheel_input_is_owned_by_the_list() {
        use freya_testing::prelude::{PlatformEvent, TestingRunner, WheelEventName};
        use std::{cell::Cell, rc::Rc};

        fn app() -> impl IntoElement {
            let measurement = use_consume::<Rc<Cell<f32>>>();
            let pass_all = use_state(|| false);
            let inner_wheel_at = use_state(|| None);
            let coast_at = use_state(|| None);
            use_hook(|| {
                provide_context(WheelEdgeChain {
                    pass_all,
                    inner_wheel_at,
                    coast_at,
                })
            });
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .on_wheel(|_: Event<WheelEventData>| panic!("fractional wheel escaped the list"))
                .child(
                    ScrollArea::new()
                        .height(Size::px(120.))
                        .show_scrollbar(false)
                        .child(rect().width(Size::fill()).height(Size::px(600.)))
                        .on_ctx(move |ctx| measurement.set(ctx.corrected_y)),
                )
        }
        let (mut runner, measurement) = TestingRunner::new(
            app,
            Size2D::new(600., 440.),
            |runner| runner.provide_root_context(|| Rc::new(Cell::new(0f32))),
            2.,
        );
        runner.poll_n(std::time::Duration::ZERO, 5);
        for _ in 0..4 {
            runner.send_event(PlatformEvent::Wheel {
                name: WheelEventName::Wheel,
                source: WheelSource::Pixel,
                phase: TouchPhase::Moved,
                scroll: (0., -1.).into(),
                cursor: (80., 80.).into(),
            });
            runner.sync_and_update();
        }
        assert_eq!(measurement.get(), -1.);
    }

    #[test]
    fn accessibility_scroll_reaches_the_real_target_at_hidpi() {
        use accesskit::{Action, ActionRequest, TreeId};
        use std::{cell::Cell, rc::Rc};

        fn app() -> impl IntoElement {
            let (measurement, id) = use_consume::<(Rc<Cell<f32>>, AccessibilityId)>();
            ScrollArea::new()
                .height(Size::px(120.))
                .show_scrollbar(false)
                .child(rect().width(Size::fill()).height(Size::px(400.)))
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::px(40.))
                        .a11y_id(id)
                        .a11y_focusable(true),
                )
                .child(rect().width(Size::fill()).height(Size::px(300.)))
                .on_ctx(move |ctx| measurement.set(ctx.corrected_y))
        }
        let (mut runner, (measurement, id)) = freya_testing::TestingRunner::new(
            app,
            Size2D::new(600., 440.),
            |runner| {
                runner.provide_root_context(|| (Rc::new(Cell::new(0f32)), AccessibilityId(987)))
            },
            2.,
        );
        runner.poll_n(std::time::Duration::ZERO, 5);
        assert!(runner.send_accessibility_action(ActionRequest {
            action: Action::ScrollIntoView,
            target_tree: TreeId::ROOT,
            target_node: id,
            data: None,
        }));
        runner.poll_n(std::time::Duration::ZERO, 5);
        assert_eq!(measurement.get(), -400.);
    }

    #[test]
    fn middle_scroll_uses_a_dead_zone_and_caps_speed() {
        assert_eq!(autoscroll_step(8.), 0.);
        assert_eq!(autoscroll_step(-6.), 0.);
        assert!(autoscroll_step(20.) < 0.);
        assert!(autoscroll_step(-20.) > 0.);
        assert_eq!(autoscroll_step(5000.), -42.);
    }

    #[tokio::test]
    async fn holding_middle_button_scrolls_until_released() {
        use freya_testing::prelude::{MouseEventName, PlatformEvent, TestingRunner};
        use std::{cell::Cell, rc::Rc, time::Duration};

        fn app() -> impl IntoElement {
            let offset = use_consume::<Rc<Cell<f32>>>();
            ScrollArea::new()
                .height(Size::px(180.))
                .show_scrollbar(false)
                .child(rect().width(Size::fill()).height(Size::px(900.)))
                .on_ctx(move |ctx| offset.set(ctx.corrected_y))
        }

        let (mut runner, offset) = TestingRunner::new(
            app,
            Size2D::new(320., 240.),
            |runner| runner.provide_root_context(|| Rc::new(Cell::new(0f32))),
            1.,
        );
        runner.poll_n(Duration::ZERO, 5);

        let middle = |name, y| PlatformEvent::Mouse {
            name,
            cursor: (50., y).into(),
            button: Some(MouseButton::Middle),
        };
        runner.send_event(middle(MouseEventName::MouseDown, 36.));
        runner.sync_and_update();
        runner.send_event(middle(MouseEventName::MouseMove, 136.));
        runner.sync_and_update();

        for _ in 0..8 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            runner.sync_and_update();
        }
        let moved = offset.get();
        assert!(moved < -15., "scroll position while held: {moved}");

        runner.send_event(middle(MouseEventName::MouseUp, 136.));
        runner.sync_and_update();
        for _ in 0..4 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            runner.sync_and_update();
        }
        assert_eq!(offset.get(), moved);
    }

    #[test]
    fn section_items_reuse_overlapping_rows_as_the_viewport_moves() {
        use std::{cell::Cell, rc::Rc};

        fn app() -> impl IntoElement {
            use_hook(|| {
                let rendered = Rc::new(Cell::new(0));
                let count = rendered.clone();
                let area = ScrollArea::new().lazy_sections(
                    vec![LazySection {
                        header: false,
                        count: 50,
                    }],
                    100.,
                    0.,
                    300.,
                    2,
                    20.,
                    move |_| {
                        count.set(count.get() + 1);
                        rect().into_element()
                    },
                    |_| rect().into_element(),
                );
                let build = area.builder.unwrap();
                let mut ctx = ScrollAreaCtx {
                    corrected_x: 0.,
                    corrected_y: 0.,
                    target_y: 0.,
                    viewport_w: 320.,
                    viewport_h: 250.,
                    at_bottom: false,
                    viewport_top: use_state(|| 0.),
                    viewport_left: use_state(|| 0.),
                };
                build(ctx);
                let initial = rendered.get();
                assert_eq!(initial, 6);
                ctx.corrected_y = -10.;
                build(ctx);
                assert_eq!(rendered.get(), initial);
                ctx.corrected_y = -100.;
                build(ctx);
                assert_eq!(rendered.get(), initial + 1);
                ctx.corrected_y = -1400.;
                build(ctx);
                assert_eq!(rendered.get(), initial + 10);
                ctx.corrected_y = 0.;
                build(ctx);
                assert_eq!(rendered.get(), initial + 16);
            });
            rect()
        }

        let (mut runner, ()) =
            freya_testing::TestingRunner::new(app, Size2D::new(320., 240.), |_| (), 1.);
        runner.sync_and_update();
    }

    #[test]
    fn a_wheel_event_scrolls_virtual_sections() {
        use std::{cell::Cell, rc::Rc};

        fn app() -> impl IntoElement {
            let offset = use_consume::<Rc<Cell<f32>>>();
            ScrollArea::new()
                .height(Size::px(180.))
                .on_ctx(move |ctx| offset.set(ctx.corrected_y))
                .lazy_sections(
                    vec![LazySection {
                        header: false,
                        count: 40,
                    }],
                    70.,
                    8.,
                    120.,
                    2,
                    24.,
                    |_| rect().into_element(),
                    |_| rect().into_element(),
                )
        }

        let (mut runner, offset) = freya_testing::TestingRunner::new(
            app,
            Size2D::new(320., 240.),
            |runner| runner.provide_root_context(|| Rc::new(Cell::new(0f32))),
            1.,
        );
        runner.poll_n(std::time::Duration::ZERO, 5);
        runner.scroll((70., 100.), (0., -200.));
        runner.poll_n(std::time::Duration::ZERO, 5);
        assert!(offset.get() < 0.);
    }

    #[test]
    fn forwarded_input_scrolls_the_list_using_its_measured_size() {
        use std::{cell::Cell, rc::Rc};

        fn app() -> impl IntoElement {
            let offset = use_consume::<Rc<Cell<f32>>>();
            let input = PageWheelInput::provide();
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .vertical()
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::px(36.))
                        .on_press(move |_| input.forward(PrimaryList::Browse, -53., false)),
                )
                .child(
                    ScrollArea::new()
                        .primary_list(PrimaryList::Browse)
                        .height(Size::px(100.))
                        .show_scrollbar(false)
                        .child(rect().height(Size::px(800.)))
                        .on_ctx(move |ctx| offset.set(ctx.corrected_y)),
                )
        }
        let (mut runner, offset) = freya_testing::TestingRunner::new(
            app,
            Size2D::new(320., 240.),
            |runner| runner.provide_root_context(|| Rc::new(Cell::new(0f32))),
            1.,
        );
        runner.poll_n(std::time::Duration::ZERO, 5);
        runner.click_cursor((16., 16.));
        runner.poll_n(std::time::Duration::ZERO, 5);
        assert_eq!(offset.get(), -53.);
    }

    #[test]
    fn header_wheel_combines_input_only_for_the_current_page() {
        fn app() -> impl IntoElement {
            let input = PageWheelInput::provide();
            use_hook(|| {
                input.forward(PrimaryList::Browse, -53., true);
                input.forward(PrimaryList::Browse, -10., false);
                assert_eq!(input.0.peek().unwrap().delta, -63.);
                input.forward(PrimaryList::Wardrobe, 53., true);
                assert_eq!(input.0.peek().unwrap().delta, 53.);
                input.clear();
                assert!(input.0.peek().is_none());
            });
            rect()
        }
        let (mut runner, ()) =
            freya_testing::TestingRunner::new(app, Size2D::new(320., 240.), |_| (), 1.);
        runner.sync_and_update();
    }

    #[test]
    fn being_within_the_slop_of_the_end_counts_as_the_bottom() {
        assert!(scrolled_to_bottom(1000., 300., -700.));
        assert!(scrolled_to_bottom(1000., 300., -699.));
        assert!(!scrolled_to_bottom(1000., 300., -600.));
        assert!(scrolled_to_bottom(200., 300., 0.));
    }
}
