use std::time::Instant;

use freya::prelude::*;
use skia_safe::{Color4f, Paint, TileMode, gradient};

#[derive(PartialEq)]
pub(super) struct Reflection {
    pub cursor: State<Option<(usize, f32, f32)>>,
    pub index: usize,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Light {
    x: f32,
    y: f32,
    alpha: f32,
}

impl Light {
    fn approach(self, goal: Self, dt: f32) -> Self {
        let follow = 1. - (-24. * dt).exp();
        let fade = 1. - (-16. * dt).exp();
        Self {
            x: self.x + (goal.x - self.x) * follow,
            y: self.y + (goal.y - self.y) * follow,
            alpha: self.alpha + (goal.alpha - self.alpha) * fade,
        }
    }

    fn settled(self, goal: Self) -> bool {
        (self.x - goal.x).abs() < 0.12
            && (self.y - goal.y).abs() < 0.12
            && (self.alpha - goal.alpha).abs() < 0.005
    }
}

impl Component for Reflection {
    fn render(&self) -> impl IntoElement {
        let light_theme = crate::theme::colors::is_light();
        let animate = crate::motion::use_animations_enabled();
        let point = self
            .cursor
            .read()
            .filter(|p| p.0 == self.index)
            .map(|(_, x, y)| (x, y));
        let mut pose = use_state(Light::default);
        let mut target = use_state(Light::default);
        let mut running = use_state(|| false);
        let ticker = use_hook(RenderingTicker::get);
        use_side_effect_with_deps(&(point, animate), move |&(point, animate)| {
            let current = *pose.peek();
            let goal = match point {
                Some((x, y)) => Light { x, y, alpha: 1. },
                None => Light {
                    alpha: 0.,
                    ..current
                },
            };
            target.set(goal);
            if !animate || current.settled(goal) {
                pose.set_if_modified(goal);
                return;
            }
            if current.alpha == 0. {
                pose.set(Light { alpha: 0., ..goal });
            }
            if *running.peek() {
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
                    let now = Instant::now();
                    let goal = *target.peek();
                    let next = pose.peek().approach(goal, (now - last).as_secs_f32());
                    last = now;
                    if next.settled(goal) {
                        pose.set_if_modified(goal);
                        break;
                    }
                    pose.set(next);
                }
                running.set(false);
            });
        });
        let light = *pose.read();
        rect()
            .interactive(false)
            .position(Position::new_absolute().left(0.).top(0.))
            .width(Size::fill())
            .height(Size::fill())
            .maybe(light.alpha > 0., |el| {
                el.background(RenderCallback::new(move |ctx: &mut FillRenderContext| {
                    let RenderTarget::Background { canvas, .. } = &ctx.target else {
                        return;
                    };
                    let (w, h) = (ctx.size.width, ctx.size.height);
                    if w <= 1. || h <= 1. {
                        return;
                    }
                    // Let the reflection fade completely before the tab boundary. The moving
                    // selection lens owns the only silhouette, even halfway through a drag.
                    let rx = (h * 0.95).min(w * 0.41);
                    let ry = h * 0.44;
                    let x = light.x.clamp(rx, w - rx);
                    let y = light.y.clamp(ry, h - ry);
                    let color = if light_theme {
                        Color::BLACK
                    } else {
                        Color::WHITE
                    };
                    let mut tint = Color4f::from(color);
                    tint.a = light.alpha * if light_theme { 0.09 } else { 0.20 };
                    let stops = [tint, Color4f::new(tint.r, tint.g, tint.b, 0.)];
                    let ramp = gradient::Gradient::new(
                        gradient::Colors::new(&stops, None, TileMode::Clamp, None),
                        gradient::Interpolation::default(),
                    );
                    let mut paint = Paint::default();
                    paint.set_shader(gradient::shaders::radial_gradient(
                        ((0., 0.), 1.),
                        &ramp,
                        None,
                    ));
                    let saved = canvas.save();
                    canvas.translate((x, y));
                    canvas.scale((rx, ry));
                    canvas.draw_circle((0., 0.), 1., &paint);
                    canvas.restore_to_count(saved);
                }))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflection_tracks_elapsed_time_across_refresh_rates() {
        let goal = Light {
            x: 90.,
            y: 12.,
            alpha: 1.,
        };
        let sample =
            |hz| (0..hz / 4).fold(Light::default(), |p, _| p.approach(goal, 1. / hz as f32));
        assert!(sample(60).settled(sample(144)));
        assert!(sample(60).alpha < 1. && sample(60).alpha > 0.97);
    }
}
