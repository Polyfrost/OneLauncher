use bytes::Bytes;
use freya::animation::*;
use freya::engine::prelude::{Paint, SamplingOptions, SkData, SkImage, SkRect};
use freya::prelude::*;
use freya::query::QueryStateData;
use freya::router::*;
use skia_safe::canvas::SrcRectConstraint;
use skia_safe::sampling_options::CubicResampler;

use crate::Route;
use crate::components::Button;
use crate::components::{AppNavbar, DynamicArt, Icon, IconType, OverlayPopup, ScrollArea};
use crate::layout::AnimatedAppOutlet;
use crate::theme;
use crate::use_settings_snapshot;
use oneclient_db::models::ClusterId;

use crate::hooks::{
    ActiveClusterState, BrowserCompatState, BrowserStateStore, use_active_cluster_id, use_clusters,
    use_game_snapshot, use_launcher, use_provide_active_cluster, use_provide_browser_compat,
    use_provide_browser_state, use_splash,
};
use crate::theme::colors;
use oneclient_core::notification::LaunchStage;
use std::collections::HashMap;

#[derive(PartialEq)]
pub struct AppShell;

impl Component for AppShell {
    fn render(&self) -> impl IntoElement {
        let active_cluster = use_state(|| None::<ClusterId>);
        use_provide_active_cluster(ActiveClusterState(active_cluster));

        let browser_compat = use_state(|| true);
        use_provide_browser_compat(BrowserCompatState(browser_compat));

        let browser_state = use_state(HashMap::new);
        use_provide_browser_state(BrowserStateStore(browser_state));

        let game = use_game_snapshot();

        let running_cluster = game
            .stages
            .iter()
            .find(|(_, s)| **s == LaunchStage::Running)
            .map(|(id, _)| *id);

        use_side_effect_with_deps(&running_cluster, move |running| {
            if let Some(cluster_id) = *running {
                spawn(async move {
                    let _ = RouterContext::get().push(Route::ProcessLogs { cluster_id });
                });
            }
        });

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .color(colors::fg_primary())
            .overflow(Overflow::Clip)
            .child(AppNavbar)
            .child(AppHomeBackground)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .child(AnimatedRouter::<Route>::new(AnimatedAppOutlet)),
            )
            .maybe_child(
                game.error
                    .clone()
                    .map(|message| LaunchErrorDialog { message }.into_element()),
            )
    }
}

pub fn hides_overlay(route: &Route) -> bool {
    matches!(route, Route::Home {})
}

const DIALOG_BG: Color = Color::from_rgb(21, 28, 34);
const CODE_BG: Color = Color::from_rgb(13, 18, 22);

#[derive(PartialEq)]
struct LaunchErrorDialog {
    message: String,
}

impl Component for LaunchErrorDialog {
    fn render(&self) -> impl IntoElement {
        let dispatch = crate::hooks::use_dispatch();
        let lines: Vec<Element> = self
            .message
            .lines()
            .map(|line| {
                label()
                    .text(line.to_string())
                    .font_family(theme::MONO_FONT)
                    .font_size(12.)
                    .width(Size::fill())
                    .color(colors::fg_primary())
                    .into_element()
            })
            .collect();

        let close = dispatch.clone();
        let outside_close = dispatch.clone();
        let copy_dispatch = dispatch.clone();
        OverlayPopup::new()
            .on_close(move |_| close.dismiss_game_error())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .on_press(move |_| outside_close.dismiss_game_error())
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(560.))
                            .background(DIALOG_BG)
                            .corner_radius(CornerRadius::new_all(16.))
                            .padding(Gaps::new_all(24.))
                            .spacing(16.)
                            .border(crate::ui::border_all_color(1., colors::component_border()))
                            .on_press(|e: Event<PressEventData>| e.stop_propagation())
                            .child(
                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .spacing(10.)
                                    .child(
                                        Icon::new(IconType::AlertTriangle)
                                            .size(20.)
                                            .color(colors::danger()),
                                    )
                                    .child(
                                        label()
                                            .text("Game failed to launch")
                                            .font_size(18.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_primary()),
                                    ),
                            )
                            .child(
                                label()
                                    .text("Minecraft could not be launched. Error details:")
                                    .font_size(13.)
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::px(260.))
                                    .background(CODE_BG)
                                    .corner_radius(CornerRadius::new_all(8.))
                                    .padding(Gaps::new_all(12.))
                                    .overflow(Overflow::Clip)
                                    .child(
                                        ScrollArea::new()
                                            .width(Size::fill())
                                            .height(Size::fill())
                                            .children(lines),
                                    ),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(10.)
                                    .child(copy_error_button(&self.message, copy_dispatch))
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(move |_| dispatch.dismiss_game_error())
                                            .text("Close"),
                                    ),
                            ),
                    ),
            )
    }
}

fn copy_error_button(message: &str, dispatch: crate::BridgeDispatch) -> impl IntoElement {
    let message = message.to_string();
    Button::new()
        .secondary()
        .on_press(move |_| {
            if let Err(err) = freya::text_edit::Clipboard::set(message.clone()) {
                tracing::warn!("clipboard copy failed: {err:?}");
                dispatch
                    .notify("Copy failed")
                    .body("Could not copy the error to the clipboard.")
                    .error()
                    .send();
            } else {
                dispatch
                    .notify("Copied to clipboard")
                    .body("Error message copied to your clipboard.")
                    .info()
                    .icon(IconType::ClipboardCheck)
                    .send();
            }
        })
        .child(Icon::new(IconType::Copy01).size(14.))
        .text("Copy")
        .into_element()
}

pub(crate) fn appshell_overlay() -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .position(Position::new_absolute())
        .interactive(false)
        .layer(Layer::Relative(2))
        .child(
            rect()
                .height(Size::px(200.))
                .width(Size::fill())
                .background(
                    LinearGradient::new()
                        .stop((Color::BLACK.with_a(100), 0.))
                        .stop((colors::page().with_a(255), 95.0)),
                ),
        )
        .child(
            rect()
                .height(Size::fill())
                .width(Size::fill())
                .background(colors::page()),
        )
}

pub const HOME_BACKGROUND_ASSET: &str = "backgrounds/CavesAndCliffs.jpg";

#[derive(PartialEq, Clone, Copy)]
pub struct AppHomeBackground;

impl Component for AppHomeBackground {
    fn render(&self) -> impl IntoElement {
        let parallax_enabled = use_settings_snapshot().settings.dynamic_background_enabled;
        let clusters_query = use_clusters();
        let active_id = use_active_cluster_id();
        let launcher = use_launcher();
        let splash = use_splash();

        // Once the cluster list has settled and the startup fetch is done, the
        // home view is populated — let the splash curtain fade out.
        let clusters_settled = matches!(
            &*clusters_query.read().state(),
            QueryStateData::Settled { .. }
        );
        let home_ready = clusters_settled && !launcher.fetching;
        use_side_effect_with_deps(&home_ready, move |&ready| {
            if ready {
                let mut flag = splash.home_ready;
                if !*flag.peek() {
                    flag.set(true);
                }
            }
        });

        let (art, art_dep) = {
            let reader = clusters_query.read();
            let state = reader.state();
            let clusters = match &*state {
                QueryStateData::Settled { res: Ok(list), .. } => list.as_slice(),
                QueryStateData::Loading {
                    res: Some(Ok(list)),
                } => list.as_slice(),
                _ => &[],
            };
            let chosen = (*active_id.read())
                .and_then(|id| clusters.iter().find(|c| c.id == id))
                .or_else(|| clusters.first());
            let dep = chosen.map(|c| c.id);
            let art = chosen.map_or_else(DynamicArt::fallback, DynamicArt::for_cluster);
            (art, dep)
        };

        let (_, art_bytes) = art.use_bytes();

        let fade = use_animation_with_dependencies(&art_dep, |conf, _| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            AnimNum::new(0., 1.)
                .time(520)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let fade_opacity = fade.get().value();

        ParallaxArt {
            art_bytes,
            fade_opacity,
            parallax_enabled,
        }
        .into_element()
    }
}

#[derive(PartialEq, Clone)]
pub struct ParallaxArt {
    pub art_bytes: Bytes,
    pub fade_opacity: f32,
    pub parallax_enabled: bool,
}

impl Component for ParallaxArt {
    fn render(&self) -> impl IntoElement {
        let mut size = use_state(|| (0f32, 0f32));
        let mut target = use_state(|| (0f32, 0f32));
        let mut current = use_state(|| (0f32, 0f32));

        let mut anim_task = use_state(|| None::<OwnedTaskHandle>);
        use_side_effect_with_deps(&self.parallax_enabled, move |enabled| {
            if *enabled {
                let handle = spawn(async move {
                    let platform = Platform::get();

                    const FRAME: std::time::Duration = std::time::Duration::from_millis(33);
                    const STEP: f32 = 0.12;
                    const SETTLE: f32 = 0.002;

                    loop {
                        let (tx, ty) = *target.peek();
                        let (cx, cy) = *current.peek();

                        if (tx - cx).abs() > SETTLE || (ty - cy).abs() > SETTLE {
                            current.set((cx + (tx - cx) * STEP, cy + (ty - cy) * STEP));
                            platform.send(UserEvent::RequestRedraw);
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

        let mut img_cache = use_state(|| None::<(usize, SkImage)>);
        let src_ptr = self.art_bytes.as_ptr() as usize;
        let cached = img_cache.peek().clone();
        let image = match cached {
            Some((ptr, img)) if ptr == src_ptr => Some(img),
            _ => {
                let decoded = decode_background_image(&self.art_bytes);
                if let Some(img) = &decoded {
                    img_cache.set(Some((src_ptr, img.clone())));
                }
                decoded
            }
        };

        let parallax_enabled = self.parallax_enabled;
        let fade_opacity = self.fade_opacity;

        let render_cb = RenderCallback::new(move |ctx: &mut CanvasContext| {
            let Some(image) = &image else {
                return;
            };
            let (w, h) = (ctx.size.width, ctx.size.height);
            if w <= 0.0 || h <= 0.0 {
                return;
            }
            let iw = image.width() as f32;
            let ih = image.height() as f32;
            if iw <= 0.0 || ih <= 0.0 {
                return;
            }

            let (cx, cy) = if parallax_enabled {
                current()
            } else {
                (0.0, 0.0)
            };

            let canvas_aspect = w / h;
            let img_aspect = iw / ih;
            let (mut crop_w, mut crop_h) = if img_aspect > canvas_aspect {
                (ih * canvas_aspect, ih)
            } else {
                (iw, iw / canvas_aspect)
            };

            const SCALE: f32 = 1.10;
            crop_w /= SCALE;
            crop_h /= SCALE;

            const STRENGTH: f32 = 0.01;
            let pan_x = cx * w * 0.5 * STRENGTH * (crop_w / w);
            let pan_y = cy * h * 0.5 * STRENGTH * (crop_h / h);
            let ccx = iw * 0.5 - pan_x;
            let ccy = ih * 0.5 - pan_y;

            let src = SkRect::new(
                ccx - crop_w * 0.5,
                ccy - crop_h * 0.5,
                ccx + crop_w * 0.5,
                ccy + crop_h * 0.5,
            );
            let dst = SkRect::new(0.0, 0.0, w, h);

            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            let sampling = SamplingOptions::from(CubicResampler::mitchell());
            ctx.canvas.draw_image_rect_with_sampling_options(
                image,
                Some((&src, SrcRectConstraint::Strict)),
                dst,
                sampling,
                &paint,
            );
        });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .position(Position::new_absolute())
            .interactive(false)
            .on_sized(move |e: Event<SizedEventData>| {
                let (sw, sh) = (e.area.width(), e.area.height());
                let (pw, ph) = *size.peek();
                if (pw - sw).abs() > 0.5 || (ph - sh).abs() > 0.5 {
                    size.set((sw, sh));
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
                    .expanded()
                    .position(Position::new_absolute())
                    .layer(Layer::Relative(1))
                    .child(gradient_overlay_radial()),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .layer(Layer::Relative(0))
                    .opacity(fade_opacity)
                    .child(
                        canvas(render_cb)
                            .key(src_ptr as u64)
                            .width(Size::fill())
                            .height(Size::fill()),
                    ),
            )
    }
}

fn decode_background_image(bytes: &Bytes) -> Option<SkImage> {
    let data = unsafe { SkData::new_bytes(bytes) };
    let img = SkImage::from_encoded(data)?;
    Some(img.make_raster_image(None, None).unwrap_or(img))
}

const VIGNETTE_SPOTLIGHT_SHADER: &str = r#"
uniform float2 iResolution;

half4 main(float2 fragCoord) {
    float2 uv = fragCoord / iResolution;

    float2 center = float2(0.80, 0.45);
    float2 distVec = uv - center;

    float2 dir = normalize(distVec + float2(0.00001));

    float leftScale = 1.8;
    float rightScale = 4.5;
    float scaleX = mix(leftScale, rightScale, smoothstep(-1.0, 1.0, dir.x));

    float topScale = 2.2;
    float bottomScale = 2.2;
    float scaleY = mix(topScale, bottomScale, smoothstep(-1.0, 1.0, dir.y));

    distVec.x *= scaleX;
    distVec.y *= scaleY;

    float dist = length(distVec);

    float vignette = smoothstep(0.0, 1.2, dist);
    vignette = pow(vignette, 1.3);

    float maxDarkness = 0.8;
    float alpha = vignette * maxDarkness;

    return half4(0.0, 0.0, 0.0, alpha);
}
"#;

pub(crate) fn gradient_overlay_radial() -> impl IntoElement {
    let effect = use_hook(|| {
        freya::engine::prelude::RuntimeEffect::make_for_shader(VIGNETTE_SPOTLIGHT_SHADER, None)
            .expect("Failed to compile vignette shader")
    });

    rect()
        .width(Size::fill())
        .height(Size::fill())
        .position(Position::new_absolute())
        .background(ShaderFill::new(
            VIGNETTE_SPOTLIGHT_SHADER,
            effect,
            move |effect, bounds| {
                let w = bounds.width();
                let h = bounds.height();

                #[rustfmt::skip]
                let uniforms: [f32; 2] = [
                    w, h,
                ];

                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        uniforms.as_ptr() as *const u8,
                        std::mem::size_of_val(&uniforms),
                    )
                };

                effect.make_shader(freya::engine::prelude::Data::new_copy(bytes), &[], None)
            },
        ))
}

pub(crate) fn back_button(destination: &str) -> impl IntoElement {
    Button::new()
        .ghost()
        .small()
        .on_press(|_| {
            RouterContext::get().go_back();
        })
        .margin(Gaps::new(0., 0., 0., 32.))
        .child(Icon::new(IconType::ArrowLeft).size(12.))
        .text(format!("Back to {destination}"))
}
