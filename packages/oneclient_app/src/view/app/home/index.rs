use std::time::{Duration, Instant};

use freya::prelude::*;

use crate::components::{
    ActiveClusterPanel, HomeStack, RecentsRow, WheelEdgeChain, normalize_wheel_delta,
};
use crate::hooks::use_overlay_claims;
use crate::motion::use_animations_enabled;
use crate::theme;
use crate::view::app::Clusters;

// --- Home <-> versions scroll tuning ---
/// Multiplier on wheel distance while scrolling between the two pages.
const WHEEL_GAIN: f32 = 1.25;
/// How long the wheel must go quiet before the stack settles onto a page.
const SETTLE_DELAY: Duration = Duration::from_millis(140);
/// Fraction of the way you need to scroll (in your scroll direction) for it to
/// settle on the other page. Lower = easier to switch.
const SWITCH_AT: f32 = 0.15;
/// Spring speed in rad/s (higher = snappier, lower = floatier).
const SPRING_OMEGA: f32 = 19.;
/// Spring damping: 1.0 = critically damped (no wobble), below 1 adds a little bounce.
const SPRING_DAMPING: f32 = 1.0;
/// Physics substep so the spring stays stable at any frame rate.
const SPRING_STEP: f32 = 1. / 480.;

#[derive(PartialEq)]
pub struct Home;

impl Component for Home {
    fn render(&self) -> impl IntoElement {
        let animations = use_animations_enabled();
        let scale_factor = Platform::get().current_window().scale_factor;
        let ticker = use_hook(RenderingTicker::get);

        // Shared with the navbar / outlet backdrop; provided by `AppShell`
        let stack = use_hook(consume_context::<HomeStack>);
        let mut open = stack.open;
        let key_claims = use_overlay_claims();
        // Rendered scroll position of the stack in px, 0 = home, viewport height = versions
        let mut pos = use_state(|| 0f32);
        // Where the stack is easing toward
        let mut target = use_state(|| 0f32);
        let mut viewport_h = use_state(|| 0f32);
        // Window-space [min_x, min_y, max_x, max_y] of the stack, which only holds the
        // versions page once it has settled there
        let mut stack_area = use_state(|| [0f32; 4]);
        let focused_node = Platform::get().current_window().focused_accessibility_node;
        let driving = use_state(|| false);
        let pass_all = use_state(|| false);
        let wheel_tick = use_state(|| 0u64);
        let mut last_dir = use_state(|| 0f32);

        // Touchpad finger on the pad (between its Started and Ended phases)
        let mut finger_down = use_state(|| false);

        let inner_wheel_at = use_state(|| None);
        let coast_at = use_state(|| None);
        let chain = WheelEdgeChain {
            pass_all,
            inner_wheel_at,
            coast_at,
        };
        use_hook(|| provide_context(chain));

        let mover = Mover {
            ticker,
            pos,
            target,
            driving,
            pass_all,
            viewport_h,
            stack,
            animated: animations,
        };

        // Tile / keyboard flips go through `open`
        let is_open = *open.read();
        {
            let mover = mover.clone();
            use_side_effect_with_deps(&is_open, move |&open_now| {
                let vh = *viewport_h.peek();
                let mut target = target;
                target.set(if open_now { vh } else { 0. });
                mover.run();
            });
        }

        let settle = {
            let mover = mover.clone();
            move || {
                let (mut target, mut open) = (target, open);
                let vh = *viewport_h.peek();
                let at = *target.peek() / vh.max(1.);
                let to_versions = if *last_dir.peek() > 0. {
                    at > SWITCH_AT
                } else {
                    at > 1. - SWITCH_AT
                };
                target.set(if to_versions { vh } else { 0. });
                if *open.peek() != to_versions {
                    open.set(to_versions);
                }
                mover.run();
            }
        };

        let wheel_mover = mover.clone();
        let on_wheel = move |e: Event<WheelEventData>| {
            // macOS touchpad: settle the moment the finger lifts, then swallow the fling's
            // momentum so it neither drags the stack again nor scrolls the versions list
            if e.source == WheelSource::Pixel {
                let mut coast_at = chain.coast_at;
                if chain.coasting() {
                    let ended = matches!(e.phase, TouchPhase::Ended | TouchPhase::Cancelled);
                    coast_at.set((!ended).then(Instant::now));
                    return;
                }
                match e.phase {
                    TouchPhase::Started => finger_down.set(true),
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        finger_down.set(false);
                        // Only when the stack owns this gesture, not a list scrolled on versions
                        if *pass_all.peek() {
                            coast_at.set(Some(Instant::now()));
                            settle();
                        }
                        return;
                    }
                    TouchPhase::Moved => {}
                }
            }

            let vh = *viewport_h.peek();
            let delta = normalize_wheel_delta(e.delta_y, *scale_factor.peek());
            if delta == 0. || vh <= 0. {
                return;
            }

            // Freya: negative wheel delta = scrolling down
            let push = -delta * WHEEL_GAIN;
            let next = (*target.peek() + push).clamp(0., vh);
            if next != *target.peek() {
                target.set(next);
                wheel_mover.run();
            }
            last_dir.set(push.signum());

            // Settle onto a page once the wheel goes quiet
            let mut tick = wheel_tick;
            let current = *tick.peek() + 1;
            tick.set(current);
            // A finger resting mid-drag isn't the wheel going quiet; its lift settles instead
            let settle = settle.clone();
            spawn(async move {
                tokio::time::sleep(SETTLE_DELAY).await;
                if *tick.peek() != current || *finger_down.peek() {
                    return;
                }
                settle();
            });
        };

        let vh = *viewport_h.read();
        let at = *pos.read();
        let offset = -at.clamp(0., vh.max(0.));
        // Only keep a page mounted while it can be seen, so keyboard focus (Tab) never
        // lands on controls that are scrolled out of view
        let settled_on_versions = is_open && vh > 0. && at >= vh - 0.5;
        let show_home = !settled_on_versions;
        // Home unmounts here, so drop its focus (e.g. the clicked Versions tile) or Page Up
        // would mistake it for focus inside the versions list
        let focused_id = Platform::get().current_window().focused_accessibility_id;
        use_side_effect_with_deps(&settled_on_versions, move |settled| {
            if *settled {
                focused_id.peek().request_unfocus();
            }
        });
        let show_versions = is_open || at > 0.5 || *target.read() > 0.5;

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .on_sized(move |event: Event<SizedEventData>| {
                let area = event.data().area;
                stack_area.set_if_modified([area.min_x(), area.min_y(), area.max_x(), area.max_y()]);
                let height = area.height();
                if (height - *viewport_h.peek()).abs() > 0.5 {
                    viewport_h.set(height);
                    // Keep the settled page pinned through window resizes
                    let to = if *open.peek() { height } else { 0. };
                    target.set(to);
                    pos.set(to);
                    publish_progress(stack, to, height);
                }
            })
            .on_wheel(on_wheel)
            .on_global_key_down(move |e: Event<KeyboardEventData>| {
                // A popup (e.g. create instance) owns Escape while it's open
                if key_claims.any() {
                    return;
                }
                let showing_versions = *open.peek();
                // Page Up scrolls the versions list while something in it has focus
                let focus_in_versions = {
                    let node = focused_node.peek();
                    let [x0, y0, x1, y1] = *stack_area.peek();
                    node.role() != AccessibilityRole::Window
                        && node.bounds().is_some_and(|b| {
                            let (cx, cy) = ((b.x0 + b.x1) / 2., (b.y0 + b.y1) / 2.);
                            (x0 as f64..x1 as f64).contains(&cx) && (y0 as f64..y1 as f64).contains(&cy)
                        })
                };
                match e.key {
                    Key::Named(NamedKey::Escape) if showing_versions => open.set(false),
                    Key::Named(NamedKey::PageUp) if showing_versions && !focus_in_versions => {
                        open.set(false)
                    }
                    Key::Named(NamedKey::PageDown) if !showing_versions => open.set(true),
                    _ => {}
                }
            })
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::percent(200.))
                    .offset_y(offset)
                    .child(home_page(show_home))
                    .child(versions_page(show_versions)),
            )
    }
}

/// Eases `pos` toward `target` once per rendered frame (not on a timer, so it stays smooth)
#[derive(Clone)]
struct Mover {
    ticker: RenderingTicker,
    pos: State<f32>,
    target: State<f32>,
    driving: State<bool>,
    pass_all: State<bool>,
    viewport_h: State<f32>,
    stack: HomeStack,
    animated: bool,
}

impl Mover {
    fn run(&self) {
        let Mover {
            ticker,
            mut pos,
            target,
            mut driving,
            pass_all,
            viewport_h,
            stack,
            animated,
        } = self.clone();

        if !animated {
            let to = *target.peek();
            pos.set(to);
            sync_pass(pass_all, to, *viewport_h.peek());
            publish_progress(stack, to, *viewport_h.peek());
            return;
        }
        if *driving.peek() {
            return;
        }
        driving.set(true);
        sync_pass(pass_all, *pos.peek(), *viewport_h.peek());

        spawn(async move {
            let window = Platform::get().current_window();
            window.request_redraw();
            let mut last = Instant::now();
            let mut vel = 0f32;
            loop {
                ticker.tick().await;
                let dt = last.elapsed().as_secs_f32().min(0.05);
                last = Instant::now();

                // Damped spring toward the (possibly moving) target, integrated in small
                // substeps: velocity is continuous, so wheel input and the final settle
                // blend into one motion instead of restarting the ease on every notch
                let mut x = *pos.peek();
                let mut remaining = dt;
                while remaining > 0. {
                    let h = remaining.min(SPRING_STEP);
                    let to = *target.peek();
                    let accel = SPRING_OMEGA * SPRING_OMEGA * (to - x)
                        - 2. * SPRING_DAMPING * SPRING_OMEGA * vel;
                    vel += accel * h;
                    x += vel * h;
                    remaining -= h;
                }

                let to = *target.peek();
                let done = (to - x).abs() < 0.25 && vel.abs() < 4.;
                if done {
                    x = to;
                }
                pos.set(x);
                sync_pass(pass_all, x, *viewport_h.peek());
                publish_progress(stack, x, *viewport_h.peek());
                if done {
                    break;
                }
                window.request_redraw();
            }
            driving.set(false);
        });
    }
}

/// Navbar styling and the shell backdrop follow the stack's position
fn publish_progress(stack: HomeStack, pos: f32, vh: f32) {
    // Not measured yet: keep whatever was published so a remount doesn't flicker
    if vh <= 0. {
        return;
    }
    stack.publish_progress((pos / vh).clamp(0., 1.));
}

/// The versions list only takes the wheel once the stack is fully settled on it
fn sync_pass(mut pass_all: State<bool>, pos: f32, vh: f32) {
    let pass = vh <= 0. || pos < vh - 0.5;
    if *pass_all.peek() != pass {
        pass_all.set(pass);
    }
}

fn home_page(show: bool) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::percent(50.))
        .content(Content::Flex)
        .overflow(Overflow::Clip)
        .padding(theme::HOME_PADDING_PX)
        .main_align(Alignment::Center)
        .maybe(show, |page| {
            page.child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .main_align(Alignment::Center)
                    .child(ActiveClusterPanel),
            )
            .child(RecentsRow)
        })
        .into_element()
}

/// Transparent: the shell's page backdrop (art fading into the page color at the top,
/// same as on the standalone versions route) fades in behind it as it scrolls up
fn versions_page(show: bool) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::percent(50.))
        .content(Content::Flex)
        .overflow(Overflow::Clip)
        .maybe(show, |page| page.child(Clusters))
        .into_element()
}
