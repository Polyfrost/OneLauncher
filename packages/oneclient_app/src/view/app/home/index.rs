use std::time::{Duration, Instant};

use freya::prelude::*;

use crate::components::{
    ActiveClusterPanel, HomeStack, RecentsRow, WheelEdgeChain, normalize_wheel_delta,
};
use crate::hooks::{OverlayClaims, use_overlay_claims};
use crate::i18n::t;
use crate::motion::use_animations_enabled;
use crate::theme;
use crate::view::app::Clusters;

// --- Home <-> versions scroll tuning ---
/// Multiplier on wheel distance while scrolling between the two pages.
const WHEEL_GAIN: f32 = 1.25;
/// Freya's winit renderer maps one wheel notch to this many line units.
const WHEEL_NOTCH: f32 = 53.;
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
        let animated = use_reactive(&use_animations_enabled());
        let window = Platform::get().current_window();
        let scale_factor = window.scale_factor;
        let focused_id = window.focused_accessibility_id;
        let ticker = use_hook(RenderingTicker::get);

        // Shared with the navbar / outlet backdrop; provided by `AppShell`
        let stack = use_hook(consume_context::<HomeStack>);
        let mut open = stack.open;
        let wheel_claims = use_overlay_claims();
        let key_claims = wheel_claims.clone();
        let home_focus = use_a11y();
        let versions_focus = use_a11y();
        // Rendered scroll position of the stack in px, 0 = home, viewport height = versions
        let mut pos = use_state(|| 0f32);
        // Where the stack is easing toward
        let mut target = use_state(|| 0f32);
        let mut viewport_h = use_state(|| 0f32);
        let driving = use_state(|| false);
        let pass_all = use_state(|| false);
        let mut wheel_tick = use_state(|| 0u64);
        let mut line_gesture = use_state(LineGesture::default);

        // Touchpad finger on the pad (between its Started and Ended phases)
        let mut finger_down = use_state(|| false);
        let mut toward_versions = use_state(|| false);

        let inner_wheel_at = use_state(|| None);
        let coast_at = use_state(|| None);
        let chain = WheelEdgeChain {
            pass_all,
            inner_wheel_at,
            coast_at,
        };
        use_hook(|| provide_context(chain));
        use_drop(move || stack.set_wheel_pending(false));

        let mover = Mover {
            ticker,
            pos,
            target,
            driving,
            pass_all,
            viewport_h,
            stack,
            animated,
            focus: PageFocus {
                focused_id,
                home: home_focus,
                versions: versions_focus,
                claims: wheel_claims.clone(),
            },
        };

        // A direct request cancels older touchpad gestures, even if it points at the
        // page that was already selected before the gesture arrived.
        let is_open = *open.read();
        {
            let mover = mover.clone();
            use_side_effect_with_deps(&(*stack.navigation.read(), *animated.read()), move |_| {
                let vh = *viewport_h.peek();
                target.set(if *open.peek() { vh } else { 0. });
                mover.run();
            });
        }

        let vh = *viewport_h.read();
        let at = *pos.read();
        let versions_in_view = if vh > 0. {
            past_midpoint(at, vh)
        } else {
            is_open
        };
        let settle = {
            let mover = mover.clone();
            let claims = wheel_claims.clone();
            move || {
                let (mut target, mut open, mut line_gesture) = (target, open, line_gesture);
                stack.set_wheel_pending(false);
                line_gesture.set(LineGesture::default());
                let vh = *viewport_h.peek();
                let versions = if claims.any() {
                    past_midpoint(*pos.peek(), vh)
                } else {
                    settle_page(*target.peek(), vh, *toward_versions.peek())
                };
                target.set(if versions { vh } else { 0. });
                open.set_if_modified(versions);
                mover.run();
            }
        };
        let wheel_mover = mover.clone();
        let on_wheel = move |e: Event<WheelEventData>| {
            if wheel_claims.any() || !e.source.is_device() {
                return;
            }
            // A touchpad fling belongs to the swipe that settled the stack.
            if e.source == WheelSource::Pixel {
                let mut coast_at = chain.coast_at;
                if chain.coasting() {
                    let ended = matches!(e.phase, TouchPhase::Ended | TouchPhase::Cancelled);
                    coast_at.set((!ended).then(Instant::now));
                    e.stop_propagation();
                    return;
                }
                match e.phase {
                    TouchPhase::Started => finger_down.set(true),
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        finger_down.set(false);
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
            let delta = normalize_wheel_delta(e.delta_y, e.source, *scale_factor.peek());
            if delta == 0. || vh <= 0. {
                return;
            }

            let navigation = *stack.navigation.peek();
            let line = e.source == WheelSource::Line;
            if line {
                let now = Instant::now();
                let target_position = *target.peek();
                let elected = *open.peek();
                let can_page = line_page(delta, target_position, vh, elected).is_some();
                if !can_page && line_gesture.peek().amount_at(now, navigation) == 0. {
                    return;
                }
                let mut amount = line_gesture.write().add(delta, now, navigation);
                if !can_page && amount.signum() == delta.signum() {
                    line_gesture.set(LineGesture::default());
                    amount = 0.;
                }
                if amount.abs() >= WHEEL_NOTCH
                    && let Some(versions) = line_page(amount, target_position, vh, elected)
                {
                    e.stop_propagation();
                    stack.set_open(versions);
                    return;
                }
                stack.set_wheel_pending(amount < 0. || *target.peek() > 0.);
            } else {
                let next = (*target.peek() - delta * WHEEL_GAIN).clamp(0., vh);
                if next == *target.peek() {
                    return;
                }
                line_gesture.set(LineGesture::default());
                stack.set_wheel_pending(next > 0.);
                target.set(next);
                wheel_mover.run();
            }
            e.stop_propagation();

            toward_versions.set_if_modified(delta < 0.);
            let sequence = wheel_tick.peek().wrapping_add(1);
            wheel_tick.set(sequence);
            let settle = settle.clone();
            spawn(async move {
                tokio::time::sleep(SETTLE_DELAY).await;
                if *wheel_tick.peek() == sequence
                    && *stack.navigation.peek() == navigation
                    && !*finger_down.peek()
                {
                    settle();
                }
            });
        };

        let offset = -at.clamp(0., vh.max(0.));
        // Keep the outgoing page mounted until its slide is over for visual continuity.
        let settled_on_versions = is_open && vh > 0. && at >= vh - 0.5;
        let show_home = !settled_on_versions;
        let show_versions = is_open || at > 0.5 || *target.read() > 0.5;
        let page_key = EventHandler::from(move |event: Event<KeyboardEventData>| {
            stack.handle_page_key(&event, key_claims.any());
        });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .on_sized(move |event: Event<SizedEventData>| {
                let height = event.data().area.height();
                let previous = *viewport_h.peek();
                if (height - previous).abs() > 0.5 {
                    let elected = *open.peek();
                    let next_pos = resize_position(*pos.peek(), previous, height, elected);
                    let next_target = resize_position(*target.peek(), previous, height, elected);
                    viewport_h.set(height);
                    target.set(next_target);
                    pos.set(next_pos);
                    sync_pass(pass_all, next_pos, height);
                    publish_progress(stack, next_pos, height);
                }
            })
            .on_wheel(on_wheel)
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::percent(200.))
                    .offset_y(offset)
                    .child(home_page(
                        show_home,
                        !versions_in_view,
                        home_focus,
                        page_key.clone(),
                    ))
                    .child(versions_page(
                        show_versions,
                        versions_in_view,
                        versions_focus,
                        page_key,
                    )),
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
    animated: State<bool>,
    focus: PageFocus,
}

#[derive(Clone)]
struct PageFocus {
    focused_id: State<AccessibilityId>,
    home: AccessibilityId,
    versions: AccessibilityId,
    claims: OverlayClaims,
}

impl PageFocus {
    fn transfer(&self, before: f32, after: f32, height: f32) {
        let was_versions = past_midpoint(before, height);
        let is_versions = past_midpoint(after, height);
        if was_versions == is_versions || self.claims.any() {
            return;
        }
        let (outgoing, incoming) = if is_versions {
            (self.home, self.versions)
        } else {
            (self.versions, self.home)
        };
        if crate::layout::focus_is_within(outgoing, *self.focused_id.peek()) {
            incoming.request_focus();
        }
    }
}

impl Mover {
    fn move_to(&mut self, position: f32) {
        let height = *self.viewport_h.peek();
        // The focused control may unmount at the endpoint. Check its ancestry
        // before replacing the old pane or changing its accessibility state.
        self.focus.transfer(*self.pos.peek(), position, height);
        self.pos.set(position);
        sync_pass(self.pass_all, position, height);
        publish_progress(self.stack, position, height);
    }

    fn run(&self) {
        let mut mover = self.clone();
        if !*mover.animated.peek() {
            let to = *mover.target.peek();
            mover.move_to(to);
            return;
        }
        if *mover.driving.peek() {
            return;
        }
        mover.driving.set(true);
        sync_pass(mover.pass_all, *mover.pos.peek(), *mover.viewport_h.peek());

        spawn(async move {
            let window = Platform::get().current_window();
            window.request_redraw();
            let mut last = Instant::now();
            let mut vel = 0f32;
            loop {
                mover.ticker.tick().await;
                if !*mover.animated.peek() {
                    let to = *mover.target.peek();
                    mover.move_to(to);
                    break;
                }
                let dt = last.elapsed().as_secs_f32().min(0.05);
                last = Instant::now();

                // Damped spring toward the (possibly moving) target, integrated in small
                // substeps: velocity is continuous, so wheel input and the final settle
                // blend into one motion instead of restarting the ease on every notch
                let mut x = *mover.pos.peek();
                let mut remaining = dt;
                let to = *mover.target.peek();
                while remaining > 0. {
                    let h = remaining.min(SPRING_STEP);
                    let accel = SPRING_OMEGA * SPRING_OMEGA * (to - x)
                        - 2. * SPRING_DAMPING * SPRING_OMEGA * vel;
                    vel += accel * h;
                    x += vel * h;
                    remaining -= h;
                }

                let done = (to - x).abs() < 0.25 && vel.abs() < 4.;
                if done {
                    x = to;
                }
                mover.move_to(x);
                if done {
                    break;
                }
                window.request_redraw();
            }
            mover.driving.set(false);
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

fn past_midpoint(position: f32, viewport: f32) -> bool {
    viewport > 0. && position >= viewport * 0.5
}

#[derive(Default)]
struct LineGesture {
    amount: f32,
    last: Option<(Instant, u64)>,
}

impl LineGesture {
    fn amount_at(&self, now: Instant, navigation: u64) -> f32 {
        if self.last.is_some_and(|(event, revision)| {
            revision == navigation && now.duration_since(event) < SETTLE_DELAY
        }) {
            self.amount
        } else {
            0.
        }
    }

    fn add(&mut self, delta: f32, now: Instant, navigation: u64) -> f32 {
        self.amount = if delta.abs() >= WHEEL_NOTCH {
            delta
        } else {
            self.amount_at(now, navigation) + delta
        };
        self.last = Some((now, navigation));
        self.amount
    }
}

fn line_page(delta: f32, target: f32, viewport: f32, elected: bool) -> Option<bool> {
    let versions = delta < 0.;
    let endpoint = if versions { viewport } else { 0. };
    (versions != elected || target != endpoint).then_some(versions)
}

fn settle_page(target: f32, viewport: f32, heading_down: bool) -> bool {
    let threshold = if heading_down {
        SWITCH_AT
    } else {
        1. - SWITCH_AT
    };
    target / viewport.max(1.) > threshold
}

fn resize_position(position: f32, old_height: f32, new_height: f32, versions: bool) -> f32 {
    let progress = if old_height > 0. {
        (position / old_height).clamp(0., 1.)
    } else if versions {
        1.
    } else {
        0.
    };
    progress * new_height
}

fn page_pane(
    focus: AccessibilityId,
    active: bool,
    name: &str,
    page_key: EventHandler<Event<KeyboardEventData>>,
) -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::percent(50.))
        .content(Content::Flex)
        .overflow(Overflow::Clip)
        .a11y_id(focus)
        .a11y_role(AccessibilityRole::Region)
        .a11y_alt(name)
        .a11y_focusable(active)
        .maybe(active, |el| el.on_key_down(page_key))
        .maybe(!active, |el| el.a11y_builder(|node| node.set_hidden()))
}

// Retain the outgoing picture for the spring without allowing an action on
// the outgoing pane before its last visible pixels have left the viewport.
fn inactive_page_shield() -> impl IntoElement {
    rect()
        .position(Position::new_absolute().top(0.).left(0.))
        .width(Size::fill())
        .height(Size::fill())
        .layer(Layer::Relative(1000))
        .background(Color::from_argb(1, 0, 0, 0))
        .on_all_press(|event: Event<PressEventData>| event.stop_propagation())
}

fn home_page(
    show: bool,
    active: bool,
    focus: AccessibilityId,
    page_key: EventHandler<Event<KeyboardEventData>>,
) -> Element {
    page_pane(
        focus,
        active,
        t("oneclient.components.navbar.home"),
        page_key,
    )
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
        .maybe_child((!active).then(inactive_page_shield))
    })
    .into_element()
}

/// Transparent: the shell's page backdrop (art fading into the page color at the top,
/// same as on the standalone versions route) fades in behind it as it scrolls up
fn versions_page(
    show: bool,
    active: bool,
    focus: AccessibilityId,
    page_key: EventHandler<Event<KeyboardEventData>>,
) -> Element {
    page_pane(focus, active, t("oneclient.routes.clusters"), page_key)
        .maybe(show, |page| {
            page.child(Clusters)
                .maybe_child((!active).then(inactive_page_shield))
        })
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paging_works_from_the_focus_anchor_and_scrolls_the_list_before_leaving() {
        use accesskit::{Action, ActionRequest, TreeId};
        use std::{cell::Cell, rc::Rc};

        #[derive(Clone)]
        struct TestState {
            pane: AccessibilityId,
            item: AccessibilityId,
            open: Rc<Cell<bool>>,
            offset: Rc<Cell<f32>>,
        }

        fn app() -> impl IntoElement {
            let test = use_consume::<TestState>();
            let stack = crate::components::use_provide_home_stack();
            crate::hooks::use_provide_overlay_claims();
            let mut menu_open = use_state(|| false);
            crate::hooks::use_overlay_claim_when(*menu_open.read());
            let claims = use_overlay_claims();
            test.open.set(*stack.open.read());

            page_pane(
                test.pane,
                true,
                "Versions",
                EventHandler::from(move |event| stack.handle_page_key(&event, claims.any())),
            )
            .child(
                crate::components::ScrollArea::new()
                    .height(Size::px(120.))
                    .keyboard_paging()
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(420.))
                            .a11y_id(test.item)
                            .a11y_focusable(true)
                            .a11y_role(AccessibilityRole::Button)
                            .on_key_down(move |event: Event<KeyboardEventData>| {
                                if event.key == Key::Named(NamedKey::F4) {
                                    let open = *menu_open.peek();
                                    menu_open.set(!open);
                                    event.stop_propagation();
                                }
                            }),
                    )
                    .on_ctx(move |ctx| test.offset.set(ctx.corrected_y)),
            )
        }

        let (mut runner, test) = freya_testing::TestingRunner::new(
            app,
            Size2D::new(600., 440.),
            |runner| {
                runner.provide_root_context(|| TestState {
                    pane: AccessibilityId(42),
                    item: AccessibilityId(43),
                    open: Rc::default(),
                    offset: Rc::default(),
                })
            },
            1.,
        );
        runner.poll_n(Duration::ZERO, 5);
        let focus_request = |target_node| ActionRequest {
            action: Action::Focus,
            target_tree: TreeId::ROOT,
            target_node,
            data: None,
        };
        assert!(runner.send_accessibility_action(focus_request(test.pane)));
        runner.poll_n(Duration::ZERO, 5);
        runner.press_key(Key::Named(NamedKey::PageDown));
        assert!(test.open.get());

        assert!(runner.send_accessibility_action(focus_request(test.item)));
        runner.poll_n(Duration::ZERO, 5);
        runner.press_key(Key::Named(NamedKey::PageDown));
        assert_eq!(test.offset.get(), -120.);
        runner.press_key(Key::Named(NamedKey::F4));
        runner.press_key(Key::Named(NamedKey::PageUp));
        assert_eq!(test.offset.get(), -120.);
        assert!(test.open.get());
        runner.press_key(Key::Named(NamedKey::F4));
        runner.press_key(Key::Named(NamedKey::PageUp));
        assert_eq!(test.offset.get(), 0.);
        assert!(test.open.get());

        runner.press_key(Key::Named(NamedKey::PageUp));
        assert!(!test.open.get());
    }

    #[test]
    fn resizing_keeps_partial_motion_and_the_selected_page() {
        assert_eq!(resize_position(0., 500., 800., false), 0.);
        assert_eq!(resize_position(125., 500., 800., false), 200.);
        assert_eq!(resize_position(500., 500., 800., true), 800.);
        assert_eq!(resize_position(0., 0., 800., true), 800.);
    }

    #[test]
    fn a_single_wheel_notch_can_step_at_any_viewport_height() {
        for viewport in [520., 800., 1200.] {
            assert_eq!(line_page(-53., 0., viewport, false), Some(true));
            assert_eq!(line_page(53., viewport, viewport, true), Some(false));
            assert_eq!(line_page(53., 40., viewport, false), Some(false));
            assert_eq!(line_page(53., 0., viewport, false), None);
        }
    }

    #[test]
    fn partial_line_deltas_only_page_after_one_notch_in_the_same_gesture() {
        let mut gesture = LineGesture::default();
        let start = Instant::now();
        let later = start + Duration::from_millis(70);
        assert!(gesture.add(-WHEEL_NOTCH / 2., start, 0).abs() < WHEEL_NOTCH);
        assert_eq!(gesture.add(-WHEEL_NOTCH / 2., later, 0), -WHEEL_NOTCH);

        gesture = LineGesture::default();
        gesture.add(-WHEEL_NOTCH / 2., start, 0);
        assert_eq!(
            gesture.add(-WHEEL_NOTCH / 2., start + SETTLE_DELAY, 0),
            -WHEEL_NOTCH / 2.
        );
        assert_eq!(
            gesture.add(-WHEEL_NOTCH / 2., start + SETTLE_DELAY, 1),
            -WHEEL_NOTCH / 2.
        );
        assert_eq!(
            gesture.add(WHEEL_NOTCH, start + SETTLE_DELAY, 1),
            WHEEL_NOTCH
        );
    }

    #[test]
    fn a_touchpad_gesture_settles_in_its_direction() {
        assert!(settle_page(100., 500., true));
        assert!(!settle_page(40., 500., true));
        assert!(!settle_page(400., 500., false));
        assert!(settle_page(460., 500., false));
    }
}
