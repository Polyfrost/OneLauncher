#![allow(dead_code)]
use freya::prelude::*;

use crate::theme::colors;

const LAZY_OVERSCAN: i64 = 3;

pub(crate) fn scroll_pos_from_wheel(wheel: f32, inner: f32, viewport: f32, current: f32) -> i32 {
    if viewport >= inner {
        return 0;
    }
    let new_pos = current + wheel;
    if new_pos >= 0.0 && wheel > 0.0 {
        return 0;
    }
    if new_pos <= -(inner - viewport) && wheel < 0.0 {
        return -(inner - viewport) as i32;
    }
    new_pos as i32
}

pub(crate) fn corrected_scroll(inner: f32, viewport: f32, pos: f32) -> f32 {
    if pos > 0.0 {
        return 0.0;
    }
    if (-pos + viewport) > inner {
        return if viewport < inner {
            -(inner - viewport)
        } else {
            0.0
        };
    }
    pos
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
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub viewport_top: f32,
    pub viewport_left: f32,
}

pub struct ScrollArea {
    width: Size,
    height: Size,
    padding: Gaps,
    spacing: f32,
    show_scrollbar: bool,
    horizontal: bool,
    content_width: f32,
    stick_bottom: bool,
    reset_key: Option<u64>,
    controller: Option<ScrollController>,
    on_user_scroll: Option<EventHandler<()>>,
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
            padding: Gaps::default(),
            spacing: 0.,
            show_scrollbar: true,
            horizontal: false,
            content_width: 0.,
            stick_bottom: false,
            reset_key: None,
            controller: None,
            on_user_scroll: None,
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

    pub fn scroll_controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    pub fn on_user_scroll(mut self, handler: impl FnMut(()) + 'static) -> Self {
        self.on_user_scroll = Some(EventHandler::new(handler));
        self
    }

    /// Called every render
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
        self.builder = Some(Box::new(move |ctx: ScrollAreaCtx| {
            let first =
                (((-ctx.corrected_y) / slot).floor() as i64 - LAZY_OVERSCAN).max(0) as usize;
            let span = ((ctx.viewport_h / slot).ceil() as i64 + 2 * LAZY_OVERSCAN).max(0) as usize;
            let last = (first + span).min(count);

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
            container.into_element()
        }));
        self
    }

    fn view(&self) -> impl IntoElement {
        let horizontal = self.horizontal;
        let content_w = self.content_width;
        let spacing = self.spacing;
        let padding = self.padding;
        let show_scrollbar = self.show_scrollbar;
        let stick_bottom = self.stick_bottom;
        let on_user_scroll = self.on_user_scroll.clone();

        let internal = use_scroll_controller(ScrollConfig::default);
        let mut controller = self.controller.unwrap_or(internal);

        let mut viewport_h = use_state(|| 0f32);
        let mut viewport_w = use_state(|| 0f32);
        let mut viewport_top = use_state(|| 0f32);
        let mut viewport_left = use_state(|| 0f32);
        let mut content_h = use_state(|| 0f32);
        let mut scroll_x = use_state(|| 0f32);
        let mut drag_start = use_state::<Option<(f32, i32)>>(|| None);
        let mut drag_start_x = use_state::<Option<(f32, f32)>>(|| None);
        let mut shift_held = use_state(|| false);

        // used to jump to the top when the reset key changes
        let reset_key = self.reset_key;
        let mut last_reset = use_state(|| reset_key);
        if reset_key.is_some() && *last_reset.peek() != reset_key {
            last_reset.set(reset_key);
            controller.scroll_to_y(0);
            scroll_x.set_if_modified(0.);
        }

        let (_, scrolled_y) = controller.into();
        let vp_h = *viewport_h.read();
        let vp_w = *viewport_w.read();
        let vp_top = *viewport_top.read();
        let vp_left = *viewport_left.read();
        let ct_h = *content_h.read();

        let corrected_y = corrected_scroll(ct_h, vp_h, scrolled_y as f32);
        let corrected_x = if horizontal {
            corrected_scroll(content_w, vp_w, *scroll_x.read())
        } else {
            0.
        };

        let (scrollbar_y, scrollbar_height) = scrollbar_pos_and_size(ct_h, vp_h, corrected_y);
        let show_v = show_scrollbar && vp_h > 0. && vp_h < ct_h;

        let (scrollbar_x, scrollbar_width) = scrollbar_pos_and_size(content_w, vp_w, corrected_x);
        let show_h = horizontal && vp_w > 0. && vp_w < content_w;

        let pressing_v = use_memo(move || drag_start.read().is_some());
        let pressing_h = use_memo(move || drag_start_x.read().is_some());

        let ctx = ScrollAreaCtx {
            corrected_x,
            corrected_y,
            viewport_w: vp_w,
            viewport_h: vp_h,
            viewport_top: vp_top,
            viewport_left: vp_left,
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
        let us_h_thumb = on_user_scroll;

        let can_scroll_v = vp_h > 0. && vp_h < ct_h;
        let can_scroll_h = horizontal && vp_w > 0. && vp_w < content_w;

        let on_wheel = move |e: Event<WheelEventData>| {
            let wants_h = horizontal && *shift_held.read();

            // check if scrollable
            if (wants_h && !can_scroll_h) || !can_scroll_v {
                return;
            }

            e.stop_propagation();

            if (e.delta_y != 0.0 || e.delta_x != 0.0)
                && let Some(cb) = &us_wheel
            {
                cb.call(());
            }

            if wants_h {
                let delta = if e.delta_x != 0.0 { e.delta_x } else { e.delta_y };
                if delta != 0.0 {
                    let cur_x = corrected_scroll(content_w, vp_w, *scroll_x.read());
                    let new_x = scroll_pos_from_wheel(delta as f32, content_w, vp_w, cur_x);
                    scroll_x.set(new_x as f32);
                }
                return;
            }

            let (_, cur_y) = controller.into();
            let current = corrected_scroll(ct_h, vp_h, cur_y as f32);
            let new_y = scroll_pos_from_wheel(e.delta_y as f32, ct_h, vp_h, current);
            if new_y != cur_y {
                controller.scroll_to_y(new_y);
            }
        };

        let on_thumb_down = move |e: Event<PointerEventData>| {
            if let Some(cb) = &us_thumb {
                cb.call(());
            }
            let (_, y) = controller.into();
            drag_start.set(Some((e.global_location().y as f32, y)));
        };

        let on_h_thumb_down = move |e: Event<PointerEventData>| {
            if let Some(cb) = &us_h_thumb {
                cb.call(());
            }
            drag_start_x.set(Some((e.global_location().x as f32, *scroll_x.read())));
        };

        let on_global_move = move |e: Event<PointerEventData>| {
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
                scroll_x.set_if_modified(corrected_scroll(content_w, vp_w, target));
            }
        };

        let on_global_release = move |_: Event<PointerEventData>| {
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
            .on_wheel(on_wheel)
            .on_capture_global_pointer_move(on_global_move)
            .on_capture_global_pointer_press(on_global_release)
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
                    .height(Size::flex(1.0))
                    .overflow(Overflow::Clip)
                    .on_sized(move |e: Event<SizedEventData>| {
                        let h = e.area.height();
                        let w = e.area.width();
                        let top = e.area.min_y();
                        let left = e.area.min_x();
                        if (*viewport_h.read() - h).abs() > 0.5 {
                            viewport_h.set(h);
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
                                if (*content_h.read() - h).abs() > 0.5 {
                                    content_h.set(h);
                                    if stick_bottom {
                                        let vp = *viewport_h.read();
                                        let target = -((h - vp).max(0.));
                                        controller.scroll_to_y(target as i32);
                                    }
                                }
                            })
                            .child(content_el),
                    )
                    .maybe_child(show_h.then(|| {
                        ScrollThumb {
                            vertical: false,
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
            && self.padding == other.padding
            && self.spacing == other.spacing
            && self.show_scrollbar == other.show_scrollbar
            && self.horizontal == other.horizontal
            && self.content_width == other.content_width
            && self.stick_bottom == other.stick_bottom
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
    main_pos: f32,
    length: f32,
    pressing: Readable<bool>,
    on_down: EventHandler<Event<PointerEventData>>,
}

impl Component for ScrollThumb {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let on_down = self.on_down.clone();
        let pressing = *self.pressing.read();
        let hovered = *hovering.read();

        let color = if pressing {
            colors::fg_secondary_pressed()
        } else if hovered {
            colors::fg_secondary_hover()
        } else {
            colors::fg_secondary()
        };
        let thickness = if pressing || hovered { 8. } else { 6. };

        let (position, width, height, layer) = if self.vertical {
            (
                Position::new_absolute().right(0.).top(self.main_pos),
                Size::px(14.),
                Size::px(self.length),
                Layer::Relative(999),
            )
        } else {
            (
                Position::new_absolute().left(self.main_pos).bottom(2.),
                Size::px(self.length),
                Size::px(8.),
                Layer::Relative(2),
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
            .layer(layer)
            .on_pointer_down(move |e| on_down.call(e))
            .on_pointer_over(move |_| hovering.set(true))
            .on_pointer_out(move |_| hovering.set(false))
            .child(
                rect()
                    .width(bar_w)
                    .height(bar_h)
                    .corner_radius(CornerRadius::new_all(12.))
                    .background(color.with_a(160)),
            )
    }
}
