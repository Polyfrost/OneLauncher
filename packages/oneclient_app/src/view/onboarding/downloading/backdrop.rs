use super::*;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use oneclient_core::clusters::Cluster;

use crate::components::{ART_PREVIEW_EDGE, DynamicArt};
use crate::hooks::use_settings_snapshot;
use crate::layout::gradient_overlay_radial;
use crate::theme::colors;

pub(super) const BACKDROP_INTERVAL_SECS: u64 = 12;

const ROTATION_PERIOD: Duration = Duration::from_secs(BACKDROP_INTERVAL_SECS);

fn since_epoch() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
}

pub(super) fn rotation_tick() -> usize {
    (since_epoch().as_secs() / BACKDROP_INTERVAL_SECS) as usize
}

pub(super) fn next_rotation_delay() -> Duration {
    let into_period = since_epoch().as_nanos() % ROTATION_PERIOD.as_nanos();
    ROTATION_PERIOD - Duration::from_nanos(into_period as u64)
}

const PARALLAX_SCALE: f32 = 1.10;
const PARALLAX_STRENGTH: f32 = 0.008;
const PARALLAX_STEP: f32 = 0.12;
const PARALLAX_SETTLE: f32 = 0.002;
const PARALLAX_FRAME: Duration = Duration::from_millis(33);

#[derive(PartialEq)]
pub struct LoadingBackdrop {
    pub clusters: Vec<Cluster>,
}

impl Component for LoadingBackdrop {
    fn render(&self) -> impl IntoElement {
        let clusters = &self.clusters;
        let count = clusters.len().max(1);
        // Offset against the shared tick so the first art shown is still the
        // first cluster
        let start = use_hook(rotation_tick);
        let mut tick = use_state(rotation_tick);

        let stop = use_hook(|| Arc::new(AtomicBool::new(false)));

        use_drop({
            let stop = stop.clone();
            move || stop.store(true, Ordering::Relaxed)
        });

        use_hook({
            let stop = stop.clone();
            move || {
                if count <= 1 {
                    return;
                }

                spawn(async move {
                    loop {
                        tokio::time::sleep(next_rotation_delay()).await;

                        if stop.load(Ordering::Relaxed) {
                            break;
                        }

                        tick.set(rotation_tick());
                    }
                });
            }
        });

        let current = (*tick.read()).wrapping_sub(start) % count;

        let parallax_enabled = use_settings_snapshot().settings.dynamic_background_enabled;

        let mut size = use_state(|| (0f32, 0f32));
        let mut target = use_state(|| (0f32, 0f32));
        let mut pan = use_state(|| (0f32, 0f32));

        let mut anim_task = use_state(|| None::<OwnedTaskHandle>);
        use_side_effect_with_deps(&parallax_enabled, move |enabled| {
            if *enabled {
                let handle = spawn(async move {
                    loop {
                        let (tx, ty) = *target.peek();
                        let (cx, cy) = *pan.peek();

                        if (tx - cx).abs() > PARALLAX_SETTLE || (ty - cy).abs() > PARALLAX_SETTLE {
                            pan.set((
                                cx + (tx - cx) * PARALLAX_STEP,
                                cy + (ty - cy) * PARALLAX_STEP,
                            ));
                        }

                        tokio::time::sleep(PARALLAX_FRAME).await;
                    }
                });
                anim_task.set(Some(handle.owned()));
            } else {
                anim_task.set(None);
                target.set((0.0, 0.0));
                pan.set((0.0, 0.0));
            }
        });

        let (cx, cy) = if parallax_enabled {
            *pan.read()
        } else {
            (0.0, 0.0)
        };
        let (sw, sh) = *size.read();
        let pan_x = cx * sw * PARALLAX_STRENGTH;
        let pan_y = cy * sh * PARALLAX_STRENGTH;

        // Default edge on purpose home shows the same art next so the same variant avoids a second fetch
        let art = match clusters.get(current) {
            Some(cluster) => DynamicArt::for_cluster(cluster),
            None => DynamicArt::fallback(),
        }
        .preview_edge(ART_PREVIEW_EDGE);

        let fade = use_animation_with_dependencies(&current, |conf, _| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            AnimNum::new(0.0, 1.0)
                .time(900)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });

        rect()
            .position(Position::new_absolute().top(0.).left(0.))
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .interactive(false)
            .background(colors::page())
            .on_sized(move |e: Event<SizedEventData>| {
                let (nw, nh) = (e.area.width(), e.area.height());
                let (pw, ph) = *size.peek();
                if (pw - nw).abs() > 0.5 || (ph - nh).abs() > 0.5 {
                    size.set((nw, nh));
                }
            })
            .maybe(parallax_enabled, |el| {
                el.on_capture_global_pointer_move(move |e: Event<PointerEventData>| {
                    let (sw, sh) = *size.peek();
                    if sw > 0.0 && sh > 0.0 {
                        let loc = e.global_location();
                        target.set((
                            (loc.x as f32 / sw) * 2.0 - 1.0,
                            (loc.y as f32 / sh) * 2.0 - 1.0,
                        ));
                    }
                })
            })
            .child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .width(Size::fill())
                    .height(Size::fill())
                    .layer(Layer::Relative(0))
                    .scale(PARALLAX_SCALE)
                    .offset_x(pan_x)
                    .offset_y(pan_y)
                    .opacity(fade.get().value())
                    .child(art),
            )
            .child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .width(Size::fill())
                    .height(Size::fill())
                    .layer(Layer::Relative(1))
                    .child(gradient_overlay_radial()),
            )
    }
}
