use freya::engine::prelude::{Paint, SkColor};
use freya::prelude::*;

use crate::theme::colors;

const SPACING: f32 = 26.;
const DOT_RADIUS: f32 = 1.6;
/// Peak drift in px at the window edge; a touch stronger than the home art so the
/// effect reads at a glance on a small preview
const DRIFT: f32 = 14.;
/// Peak dot opacity at the centre, fading to nothing at the edge of the field
const PEAK_ALPHA: f32 = 0.7;

/// A faint dot field behind the Appearance illustration. With parallax on it eases toward
/// the cursor the same way the home background does, and with it off it holds still, so
/// the toggle has something visible to compare against
#[derive(PartialEq)]
pub(crate) struct ParallaxGrid {
    pub enabled: bool,
}

impl Component for ParallaxGrid {
    fn render(&self) -> impl IntoElement {
        let mut target = use_state(|| (0f32, 0f32));
        let mut current = use_state(|| (0f32, 0f32));

        // Same easing loop as the home background so the preview feels like the real thing
        let mut anim_task = use_state(|| None::<OwnedTaskHandle>);
        use_side_effect_with_deps(&self.enabled, move |enabled| {
            if *enabled {
                let handle = spawn(async move {
                    let window = Platform::get().current_window();

                    const FRAME: std::time::Duration = std::time::Duration::from_millis(16);
                    const STEP: f32 = 0.12;
                    const SETTLE: f32 = 0.002;

                    loop {
                        let (tx, ty) = *target.peek();
                        let (cx, cy) = *current.peek();

                        if (tx - cx).abs() > SETTLE || (ty - cy).abs() > SETTLE {
                            current.set((cx + (tx - cx) * STEP, cy + (ty - cy) * STEP));
                            window.request_redraw();
                        }

                        tokio::time::sleep(FRAME).await;
                    }
                });
                anim_task.set(Some(handle.owned()));
            } else {
                anim_task.set(None);
                target.set((0.0, 0.0));
                current.set((0.0, 0.0));
            }
        });

        let enabled = self.enabled;
        let dot = colors::fg_secondary();

        let render_cb = RenderCallback::new(move |ctx: &mut FillRenderContext| {
            let RenderTarget::Background { canvas, .. } = &ctx.target else {
                return;
            };
            let (w, h) = (ctx.size.width, ctx.size.height);
            if w <= 0.0 || h <= 0.0 {
                return;
            }

            let (cx, cy) = if enabled { current() } else { (0.0, 0.0) };
            let (ox, oy) = (cx * DRIFT, cy * DRIFT);

            let (mid_x, mid_y) = (w * 0.5, h * 0.5);
            let reach = w.min(h) * 0.5;

            let mut paint = Paint::default();
            paint.set_anti_alias(true);

            let cols = (w / SPACING).ceil() as i32 + 2;
            let rows = (h / SPACING).ceil() as i32 + 2;
            // Anchor the lattice on the centre so it stays symmetric around the illustration
            let start_x = mid_x - (cols / 2) as f32 * SPACING;
            let start_y = mid_y - (rows / 2) as f32 * SPACING;

            for row in 0..=rows {
                for col in 0..=cols {
                    let x = start_x + col as f32 * SPACING + ox;
                    let y = start_y + row as f32 * SPACING + oy;
                    // Fade by the dot's resting spot so the vignette stays put while dots move
                    let rest_dx = x - ox - mid_x;
                    let rest_dy = y - oy - mid_y;
                    let falloff = 1.0 - (rest_dx.hypot(rest_dy) / reach).min(1.0);
                    if falloff <= 0.0 {
                        continue;
                    }
                    let alpha = (PEAK_ALPHA * falloff * falloff * 255.0) as u8;
                    paint.set_color(SkColor::from(dot.with_a(alpha)));
                    canvas.draw_circle((x, y), DOT_RADIUS, &paint);
                }
            }
        });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .interactive(false)
            .maybe(enabled, |el| {
                el.on_capture_global_pointer_move(move |e: Event<PointerEventData>| {
                    let window = Platform::get().current_window();
                    let size = *window.root_size.peek();
                    if size.width > 0.0 && size.height > 0.0 {
                        let loc = e.global_location();
                        target.set((
                            (loc.x as f32 / size.width) * 2.0 - 1.0,
                            (loc.y as f32 / size.height) * 2.0 - 1.0,
                        ));
                    }
                })
            })
            .background(render_cb)
    }
}
