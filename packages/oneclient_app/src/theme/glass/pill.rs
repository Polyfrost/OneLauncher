use std::time::Instant;

use freya::prelude::*;

use super::{GlassSurface, render_panel};

#[derive(Clone, Copy, Default, PartialEq)]
struct Slot {
    start: f32,
    end: f32,
    cross_start: f32,
    cross_end: f32,
}

impl Slot {
    fn center(self) -> f32 {
        (self.start + self.end) / 2.
    }
}

fn edges_at(slots: &[Slot], x: f32) -> (f32, f32) {
    if slots.len() == 1 {
        return (slots[0].start, slots[0].end);
    }
    let center = x.clamp(slots[0].center(), slots[slots.len() - 1].center());
    let index = slots
        .partition_point(|slot| slot.center() < center)
        .clamp(1, slots.len() - 1);
    let (a, b) = (slots[index - 1], slots[index]);
    let t = (center - a.center()) / (b.center() - a.center());
    (
        a.start + (b.start - a.start) * t,
        a.end + (b.end - a.end) * t,
    )
}

#[derive(Clone, Copy)]
struct Drag {
    start: f32,
    option: usize,
    delta: Option<f32>,
    pointer_cross: f32,
}

/// Measure the real tab bounds in window coordinates, including alignment and display scaling.
#[derive(Clone, Copy, PartialEq)]
pub struct PillTrack {
    slots: State<Vec<Slot>>,
    vertical: bool,
    viewport: State<Option<(f32, f32)>>,
    hover: State<Option<(usize, f32, f32)>>,
    drag: State<Option<Drag>>,
    /// Option under a dragged lens. Changes only when the lens crosses into another option,
    /// so labels can follow the drag without re-rendering on every pointer move.
    over: State<Option<usize>>,
}

pub fn use_pill_track(count: usize) -> PillTrack {
    use_track(count, false)
}

pub fn use_vertical_pill_track(count: usize) -> PillTrack {
    use_track(count, true)
}

fn use_track(count: usize, vertical: bool) -> PillTrack {
    let mut slots = use_state(|| vec![Slot::default(); count]);
    if slots.peek().len() != count {
        slots.write().resize(count, Slot::default());
    }
    PillTrack {
        slots,
        vertical,
        viewport: use_state(|| None),
        hover: use_state(|| None),
        drag: use_state(|| None),
        over: use_state(|| None),
    }
}

fn nearest(slots: &[Slot], center: f32, viewport: Option<(f32, f32)>) -> Option<usize> {
    slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| {
            viewport.is_none_or(|(start, end)| {
                slot.end > slot.start && slot.end > start && slot.start < end
            })
        })
        .min_by(|(_, a), (_, b)| {
            (a.center() - center)
                .abs()
                .total_cmp(&(b.center() - center).abs())
        })
        .map(|(index, _)| index)
}

impl PillTrack {
    pub fn viewport(self, rect: Rect) -> Rect {
        let mut viewport = self.viewport;
        rect.on_sized(move |event: Event<SizedEventData>| {
            let area = event.data().area;
            viewport.set_if_modified(Some((area.min_y(), area.max_y())));
        })
    }

    /// Whether `index` should look selected: the option under the lens while it is dragged,
    /// the real selection otherwise.
    pub fn highlights(self, index: usize, selected: bool) -> bool {
        (*self.over.read()).map_or(selected, |over| over == index)
    }

    pub fn option(self, rect: Rect, index: usize, enabled: bool) -> Rect {
        let mut slots = self.slots;
        let vertical = self.vertical;
        let mut drag = self.drag;
        let mut hover = self.hover;
        rect.on_sized(move |event: Event<SizedEventData>| {
            let area = event.data().area;
            let slot = if vertical {
                Slot {
                    start: area.min_y(),
                    end: area.max_y(),
                    cross_start: area.min_x(),
                    cross_end: area.max_x(),
                }
            } else {
                Slot {
                    start: area.min_x(),
                    end: area.max_x(),
                    cross_start: area.min_y(),
                    cross_end: area.max_y(),
                }
            };
            if slots.peek().get(index).is_some_and(|old| *old != slot) {
                slots.write()[index] = slot;
            }
        })
        .maybe(enabled, |el| {
            el.child(super::hover::Reflection {
                cursor: hover,
                index,
            })
            .on_pointer_move(move |event: Event<PointerEventData>| {
                if matches!(event.data(), PointerEventData::Mouse(_)) {
                    let p = event.element_location();
                    hover.set_if_modified(Some((index, p.x as f32, p.y as f32)));
                }
            })
            .on_pointer_leave(move |_| {
                if hover.peek().is_some_and(|sample| sample.0 == index) {
                    hover.set(None);
                }
            })
            .on_pointer_down(move |event: Event<PointerEventData>| {
                // Leave touch scrolling to the sidebar's scroll area.
                if event.data().is_primary()
                    && (!vertical || matches!(event.data(), PointerEventData::Mouse(_)))
                {
                    let point = event.global_location();
                    let (start, pointer_cross) =
                        axis_coords(vertical, point.x as f32, point.y as f32);
                    drag.set(Some(Drag {
                        start,
                        option: index,
                        delta: None,
                        pointer_cross,
                    }));
                    event.stop_propagation();
                }
            })
        })
    }

    pub fn events(self, rect: Rect, mut select: impl FnMut(usize) + 'static) -> Rect {
        let mut drag = self.drag;
        let mut over = self.over;
        let slots = self.slots;
        let viewport = self.viewport;
        let vertical = self.vertical;
        rect.on_capture_global_pointer_move(move |event: Event<PointerEventData>| {
            let Some(mut held) = *drag.peek() else {
                return;
            };
            let point = event.global_location();
            let (along, cross) = axis_coords(vertical, point.x as f32, point.y as f32);
            let delta = along - held.start;
            let lateral = (cross - held.pointer_cross).abs();
            if held.delta.is_none() && lateral > 4. && lateral > delta.abs() {
                drag.set(None);
                return;
            }
            if held.delta.is_some() || delta.abs() >= 4. {
                held.delta = Some(delta);
                held.pointer_cross = cross;
                drag.set(Some(held));
                let slots = slots.peek();
                let under = slots
                    .get(held.option)
                    .and_then(|start| nearest(&slots, start.center() + delta, *viewport.peek()));
                if *over.peek() != under {
                    over.set(under);
                }
                event.stop_propagation();
            }
        })
        .on_capture_global_pointer_up(move |event: Event<PointerEventData>| {
            if !event.data().is_primary() {
                return;
            }
            let Some(held) = *drag.peek() else {
                return;
            };
            if vertical && held.delta.is_some() {
                event.stop_propagation();
            }
            drag.set(None);
            if over.peek().is_some() {
                over.set(None);
            }
            let slots = slots.peek();
            let point = event.global_location();
            let (along, cross) = axis_coords(vertical, point.x as f32, point.y as f32);
            let Some(start) = slots.get(held.option) else {
                return;
            };
            let cross_size = start.cross_end - start.cross_start;
            let tolerance = if vertical { 0. } else { cross_size / 2. };
            if cross < start.cross_start - tolerance || cross > start.cross_end + tolerance {
                return;
            }
            let viewport = *viewport.peek();
            if viewport.is_some_and(|(top, bottom)| along < top || along > bottom) {
                return;
            }
            if held.delta.is_some() {
                if let Some(index) = nearest(&slots, start.center() + along - held.start, viewport)
                {
                    select(index);
                }
            } else if (start.start..=start.end).contains(&along) {
                select(held.option);
            }
        })
    }

    pub fn indicator(self, active: Option<usize>) -> Element {
        PillIndicator {
            track: self,
            active,
        }
        .into_element()
    }
}

fn axis_coords(vertical: bool, x: f32, y: f32) -> (f32, f32) {
    if vertical { (y, x) } else { (x, y) }
}

// Subscribe to pointer and spring frames inside the lens. The containing page can be
// expensive (Browse results and theme previews) and does not change as the lens moves.
#[derive(PartialEq)]
struct PillIndicator {
    track: PillTrack,
    active: Option<usize>,
}

impl Component for PillIndicator {
    fn render(&self) -> impl IntoElement {
        let theme = crate::theme::colors::active_theme();
        let animate = crate::motion::use_animations_enabled();
        let slots = self.track.slots.read();
        let active = self.active;
        let selected = slots.get(active.unwrap_or(0)).copied().unwrap_or_default();
        let cross_size = selected.cross_end - selected.cross_start;
        let held = (*self.track.drag.read()).filter(|drag| drag.option < slots.len());
        let ready = !slots.is_empty()
            && slots.iter().all(|slot| slot.end > slot.start)
            && slots
                .windows(2)
                .all(|pair| pair[1].center() > pair[0].center());
        let dragged = held.and_then(|held| {
            let delta = held.delta.filter(|_| ready)?;
            Some(edges_at(&slots, slots[held.option].center() + delta))
        });
        let target = dragged.unwrap_or((selected.start, selected.end));
        // Pressing the lens itself lifts it at once; pressing another option only moves it.
        let raised = dragged.is_some() || held.is_some_and(|held| Some(held.option) == active);
        let (left, right, lift) =
            use_spring_edges(target, animate && ready, dragged.is_some(), raised);
        if ready && (active.is_some() || held.is_some()) {
            let center = (left + right) * 0.5;
            // Unequal labels resize naturally along the row; only elastic strain changes height.
            let (base_start, base_end) = edges_at(&slots, center);
            let resting_size = base_end - base_start;
            let size = (right - left).clamp(resting_size * 0.73, resting_size * 1.38);
            let start = (center - size * 0.5).max(slots[0].start - 2.);
            let end = (center + size * 0.5).min(slots[slots.len() - 1].end + 2.);
            let vertical = self.track.vertical;
            let cross_scale = if vertical {
                // A vertical rail is already wide, so stretch primarily along the drag.
                1. + 0.02 * lift
            } else {
                let strain = ((size / resting_size - 1.) * 0.40).clamp(-0.09, 0.13);
                (1. - strain) * (1. + 0.05 * lift)
            };
            let cross_start = selected.cross_start + cross_size * (1. - cross_scale) * 0.5;
            let (x, y) = axis_coords(vertical, start, cross_start);
            let (width, height) =
                axis_coords(vertical, (end - start).max(1.), cross_size * cross_scale);
            let finger = held.and_then(|held| {
                Some(axis_coords(
                    vertical,
                    held.start + held.delta? - start,
                    held.pointer_cross - cross_start,
                ))
            });
            let light = theme.colors.light;
            let (resting, clear) = if light { (142., 78.) } else { (78., 50.) };
            let alpha = (resting + (clear - resting) * lift.clamp(0., 1.)) as u8;
            let tint = if light {
                Color::from_argb(alpha, 255, 255, 255)
            } else {
                Color::from_argb(alpha, 108, 114, 128)
            };
            render_panel(
                rect()
                    .position(Position::new_global().left(x).top(y))
                    .width(Size::px(width))
                    .height(Size::px(height))
                    .interactive(false),
                &theme,
                height.min(width) / 2.,
                tint,
                GlassSurface::Selector {
                    light,
                    lift,
                    finger,
                },
            )
            .into_element()
        } else {
            rect()
                .position(Position::new_absolute())
                .interactive(false)
                .into_element()
        }
    }
}

#[derive(Clone, Copy)]
struct Edge {
    position: f32,
    velocity: f32,
}

impl Edge {
    fn at(position: f32) -> Self {
        Self {
            position,
            velocity: 0.,
        }
    }

    fn follow(&mut self, goal: f32, omega: f32, damping: f32, dt: f32) {
        let decay = omega * damping;
        let frequency = omega * (1. - damping * damping).sqrt();
        let displacement = self.position - goal;
        let slope = (self.velocity + decay * displacement) / frequency;
        let (sin, cos) = (frequency * dt).sin_cos();
        let envelope = (-decay * dt).exp();
        let offset = displacement * cos + slope * sin;
        self.position = goal + envelope * offset;
        self.velocity =
            envelope * (frequency * (-displacement * sin + slope * cos) - decay * offset);
    }

    fn close(self, goal: f32) -> bool {
        (self.position - goal).abs() < 0.12 && self.velocity.abs() < 3.
    }
}

fn step_edges(pose: &mut (Edge, Edge), goal: Goal, dt: f32) {
    // Keep a click nearly rigid. A dragged leading edge can pull ahead of the trailing edge.
    let (fast, slow, damping) = if goal.holding {
        (63., 29., 0.77)
    } else {
        (34., 32., 0.96)
    };
    pose.0.follow(
        goal.edges.0,
        if goal.forward { slow } else { fast },
        damping,
        dt,
    );
    pose.1.follow(
        goal.edges.1,
        if goal.forward { fast } else { slow },
        damping,
        dt,
    );
}

#[derive(Clone, Copy)]
struct Goal {
    edges: (f32, f32),
    forward: bool,
    holding: bool,
    animate: bool,
}

fn level(raised: bool) -> f32 {
    if raised { 1. } else { 0. }
}

fn lift_settled(lift: Edge, goal: f32) -> bool {
    (lift.position - goal).abs() < 0.01 && lift.velocity.abs() < 0.2
}

/// The same springs follow the pointer and settle onto the selection. Retargets keep velocity.
/// The third value is the lens lift, eased between 0 (resting) and 1 (held).
fn use_spring_edges(
    target: (f32, f32),
    animate: bool,
    holding: bool,
    raised: bool,
) -> (f32, f32, f32) {
    let ticker = use_hook(RenderingTicker::get);
    let mut pose = use_state(|| (Edge::at(target.0), Edge::at(target.1)));
    let mut lift = use_state(|| Edge::at(level(raised)));
    let mut lift_goal = use_state(|| level(raised));
    let mut current = use_state(|| Goal {
        edges: target,
        forward: true,
        holding,
        animate,
    });
    let mut running = use_state(|| false);

    use_side_effect_with_deps(
        &(target, holding, animate, raised),
        move |&(target, holding, animate, raised)| {
            let previous = *current.peek();
            let shift = target.0 + target.1 - previous.edges.0 - previous.edges.1;
            let forward = if shift.abs() > 0.01 {
                shift > 0.
            } else {
                previous.forward
            };
            current.set(Goal {
                edges: target,
                forward,
                holding,
                animate,
            });
            let raise = level(raised);
            if *lift_goal.peek() != raise {
                lift_goal.set(raise);
            }
            if !animate || pose.peek().1.position <= pose.peek().0.position {
                pose.set((Edge::at(target.0), Edge::at(target.1)));
                lift.set(Edge::at(raise));
                return;
            }
            if *running.peek()
                || (pose.peek().0.close(target.0)
                    && pose.peek().1.close(target.1)
                    && lift_settled(*lift.peek(), raise))
            {
                return;
            }
            running.set(true);
            let ticker = ticker.clone();
            spawn(async move {
                let window = Platform::get().current_window();
                let mut last = Instant::now();
                loop {
                    window.request_redraw();
                    ticker.tick().await;
                    let dt = last.elapsed().as_secs_f32();
                    last = Instant::now();
                    let goal = *current.peek();
                    let raise = *lift_goal.peek();
                    let (start, end) = goal.edges;
                    let mut next = *pose.peek();
                    step_edges(&mut next, goal, dt);
                    let mut raised = *lift.peek();
                    raised.follow(raise, 30., 0.9, dt);
                    if !goal.animate
                        || (next.0.close(start) && next.1.close(end) && lift_settled(raised, raise))
                    {
                        pose.set((Edge::at(start), Edge::at(end)));
                        lift.set(Edge::at(raise));
                        break;
                    }
                    pose.set(next);
                    lift.set(raised);
                }
                running.set(false);
            });
        },
    );

    let (left, right) = *pose.read();
    if animate && right.position > left.position {
        (left.position, right.position, lift.read().position)
    } else {
        (target.0, target.1, level(raised))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_remaining_option_has_fixed_edges() {
        let slot = Slot {
            start: 10.,
            end: 50.,
            ..Slot::default()
        };
        for pointer in [-20., 30., 100.] {
            assert_eq!(edges_at(&[slot], pointer), (10., 50.));
        }
    }

    #[test]
    fn vertical_release_excludes_tabs_outside_the_scroll_view() {
        let slots = [0., 34., 68.].map(|start| Slot {
            start,
            end: start + 32.,
            cross_start: 20.,
            cross_end: 160.,
        });
        let viewport = Some((33., 104.));

        assert_eq!(nearest(&slots, 28., viewport), Some(1));
        assert_eq!(nearest(&slots, 90., viewport), Some(2));
        assert_eq!(nearest(&slots, 28., None), Some(0));
        assert_eq!(axis_coords(true, 20., 34.), (34., 20.));
    }

    #[test]
    fn taps_stay_composed_and_drag_release_settles_from_either_direction() {
        for travel in [-180., 180.] {
            for holding in [false, true] {
                let mut pose = (Edge::at(0.), Edge::at(90.));
                let mut goal = Goal {
                    edges: (travel, travel + 90.),
                    forward: travel > 0.,
                    holding,
                    animate: true,
                };
                let mut strain = 0_f32;
                for _ in 0..8 {
                    step_edges(&mut pose, goal, 1. / 120.);
                    strain = strain.max((pose.1.position - pose.0.position - 90.).abs());
                }
                if holding {
                    assert!(strain > 20.);
                } else {
                    assert!(strain < 7.);
                }
                goal.holding = false;
                for _ in 0..100 {
                    step_edges(&mut pose, goal, 1. / 120.);
                    if !holding {
                        assert!((pose.0.position * travel.signum()) <= travel.abs() + 0.1);
                    }
                }
                assert!(pose.0.close(travel) && pose.1.close(travel + 90.));
            }
        }
    }
}
