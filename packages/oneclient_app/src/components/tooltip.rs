use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use freya::animation::{AnimNum, Ease, OnCreation, use_animation};
use freya::prelude::*;

use crate::components::OVERLAY_MAX_LEVEL;
use crate::theme::colors;
use crate::ui::{border_all_color, clamp_to_window, window_logical_size};

const SHOW_DELAY: Duration = Duration::from_millis(400);
const ANCHOR_GAP: f32 = 6.;
const MAX_WIDTH: f32 = 260.;
const FADE_MS: u64 = 110;
const HOVER_SLACK: f32 = 1.;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TooltipPlacement {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct AnchorRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl AnchorRect {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x - HOVER_SLACK
            && x <= self.x + self.width + HOVER_SLACK
            && y >= self.y - HOVER_SLACK
            && y <= self.y + self.height + HOVER_SLACK
    }
}

#[derive(Clone, PartialEq)]
struct ActiveTooltip {
    owner: u64,
    text: Box<str>,
    anchor: AnchorRect,
    placement: TooltipPlacement,
}

#[derive(Clone)]
struct Tooltips {
    active: State<Option<ActiveTooltip>>,
    next_owner: Rc<Cell<u64>>,
}

pub fn use_provide_tooltips() {
    let active = use_state(|| None::<ActiveTooltip>);
    let next_owner = use_hook(|| Rc::new(Cell::new(0u64)));
    use_provide_root_context(move || Tooltips { active, next_owner });
}

#[derive(Clone)]
struct AnchorState {
    tooltips: Tooltips,
    owner: u64,
    area: Rc<Cell<AnchorRect>>,
    generation: Rc<Cell<u64>>,
    text: Box<str>,
    placement: TooltipPlacement,
}

pub struct TooltipAnchor(Option<AnchorState>);

pub fn use_tooltip_anchor(text: Option<Box<str>>, placement: TooltipPlacement) -> TooltipAnchor {
    let tooltips = use_hook(try_consume_root_context::<Tooltips>);
    let owner = use_hook(|| {
        tooltips.as_ref().map_or(0, |tooltips| {
            let owner = tooltips.next_owner.get() + 1;
            tooltips.next_owner.set(owner);
            owner
        })
    });
    let area = use_hook(|| Rc::new(Cell::new(AnchorRect::default())));
    let generation = use_hook(|| Rc::new(Cell::new(0u64)));

    let abandoned = generation.clone();
    use_drop(move || abandoned.set(abandoned.get() + 1));

    TooltipAnchor(tooltips.zip(text).map(|(tooltips, text)| AnchorState {
        tooltips,
        owner,
        area,
        generation,
        text,
        placement,
    }))
}

impl TooltipAnchor {
    pub fn attach(&self, rect: Rect) -> Rect {
        let Some(state) = self.0.clone() else {
            return rect;
        };

        let sized = state.clone();
        let enter = state.clone();
        let leave = state.clone();
        let pressed = state.clone();
        let scrolled = state;

        rect.on_sized(move |e: Event<SizedEventData>| {
            let area = e.area;
            sized.area.set(AnchorRect {
                x: area.min_x(),
                y: area.min_y(),
                width: area.width(),
                height: area.height(),
            });
        })
        .on_pointer_enter(move |e: Event<PointerEventData>| {
            let global = e.global_location();
            let element = e.element_location();

            let mut anchor = enter.area.get();
            anchor.x = (global.x - element.x) as f32;
            anchor.y = (global.y - element.y) as f32;

            let generation = enter.generation.get() + 1;
            enter.generation.set(generation);

            let pending = enter.generation.clone();
            let mut active = enter.tooltips.active;
            let tooltip = ActiveTooltip {
                owner: enter.owner,
                text: enter.text.clone(),
                anchor,
                placement: enter.placement,
            };

            spawn(async move {
                tokio::time::sleep(SHOW_DELAY).await;
                if pending.get() == generation {
                    active.set(Some(tooltip));
                }
            });
        })
        .on_pointer_leave(move |_| hide(&leave))
        .on_mouse_down(move |_| hide(&pressed))
        .on_wheel(move |_| hide(&scrolled))
    }
}

fn hide(state: &AnchorState) {
    state.generation.set(state.generation.get() + 1);
    clear_owner(state.tooltips.active, state.owner);
}

fn clear_owner(mut active: State<Option<ActiveTooltip>>, owner: u64) {
    let shown = active.peek().as_ref().map(|tooltip| tooltip.owner);
    if shown == Some(owner) {
        active.set(None);
    }
}

#[derive(PartialEq)]
pub struct TooltipHost;

impl Component for TooltipHost {
    fn render(&self) -> impl IntoElement {
        let Some(tooltips) = use_hook(try_consume_root_context::<Tooltips>) else {
            return rect();
        };

        let state = tooltips.active;
        let active = (*state.read()).clone();

        let mut host = rect()
            .layer(Layer::OverlayLevel(OVERLAY_MAX_LEVEL))
            .position(Position::new_global().top(0.).left(0.))
            .width(Size::window_percent(100.))
            .height(Size::window_percent(100.))
            .interactive(false);

        if let Some((owner, anchor)) = active.as_ref().map(|active| (active.owner, active.anchor)) {
            host = host.on_global_pointer_move(move |e: Event<PointerEventData>| {
                let cursor = e.global_location();
                if !anchor.contains(cursor.x as f32, cursor.y as f32) {
                    clear_owner(state, owner);
                }
            });
        }

        host.maybe_child(active.map(|active| {
            let owner = active.owner;
            TooltipBubble {
                text: active.text,
                anchor: active.anchor,
                placement: active.placement,
                key: DiffKey::None,
            }
            .key(owner)
            .into_element()
        }))
    }
}

#[derive(PartialEq)]
struct TooltipBubble {
    text: Box<str>,
    anchor: AnchorRect,
    placement: TooltipPlacement,
    key: DiffKey,
}

impl KeyExt for TooltipBubble {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for TooltipBubble {
    fn render(&self) -> impl IntoElement {
        let mut size = use_state(Size2D::zero);

        let measured = *size.read();
        let placed = measured.width > 0.;

        let fade = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(FADE_MS).ease(Ease::Out)
        });

        let (x, y) = place(self.anchor, self.placement, measured);

        rect()
            .position(Position::new_global().top(y).left(x))
            .interactive(false)
            .opacity(if placed { fade.read().value() } else { 0. })
            .max_width(Size::px(MAX_WIDTH))
            .padding(Gaps::new_symmetric(5., 9.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(colors::page_elevated())
            .border(border_all_color(1., colors::component_border()))
            .on_sized(move |e: Event<SizedEventData>| {
                let bubble = e.area.size;
                let previous = *size.peek();
                if (bubble.width - previous.width).abs() > 0.5
                    || (bubble.height - previous.height).abs() > 0.5
                {
                    size.set(bubble);
                }
            })
            .child(
                label()
                    .text(self.text.to_string())
                    .font_size(12.)
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(2)
                    .color(colors::fg_primary()),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

fn place(anchor: AnchorRect, placement: TooltipPlacement, size: Size2D) -> (f32, f32) {
    let window = window_logical_size();

    let centered_x = anchor.x + (anchor.width - size.width) / 2.;
    let centered_y = anchor.y + (anchor.height - size.height) / 2.;

    let above = anchor.y - size.height - ANCHOR_GAP;
    let below = anchor.y + anchor.height + ANCHOR_GAP;
    let before = anchor.x - size.width - ANCHOR_GAP;
    let after = anchor.x + anchor.width + ANCHOR_GAP;

    let fits_above = above >= 0.;
    let fits_below = below + size.height <= window.height;
    let fits_before = before >= 0.;
    let fits_after = after + size.width <= window.width;

    let (x, y) = match placement {
        TooltipPlacement::Top => (
            centered_x,
            if fits_above || !fits_below { above } else { below },
        ),
        TooltipPlacement::Bottom => (
            centered_x,
            if fits_below || !fits_above { below } else { above },
        ),
        TooltipPlacement::Left => (
            if fits_before || !fits_after {
                before
            } else {
                after
            },
            centered_y,
        ),
        TooltipPlacement::Right => (
            if fits_after || !fits_before {
                after
            } else {
                before
            },
            centered_y,
        ),
    };

    clamp_to_window(x, y, size.width, size.height)
}
