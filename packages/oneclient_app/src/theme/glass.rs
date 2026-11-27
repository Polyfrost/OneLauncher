//! Backdrop optics for floating PolyGlass surfaces.
use freya::prelude::*;
use skia_safe::canvas::SaveLayerRec;
use skia_safe::runtime_effect::RuntimeShaderBuilder;
use skia_safe::{
    Color4f, ImageFilter, Paint, Rect as SkRect, RuntimeEffect, TileMode, image_filters,
};
use std::cell::OnceCell;

use super::Theme;
mod hover;
mod pill;
pub use pill::{PillTrack, use_pill_track, use_vertical_pill_track};

const BEVEL_WIDTH: f32 = 19.;
const REFRACTION: f32 = 1.8;
const DISPERSION: f32 = 0.065;
const BACKDROP_BLUR: f32 = 0.8;
/// Radius that reads as a capsule on any single-line control and as a soft rectangle on
/// anything taller, so PolyGlass controls share the bar's shape without turning into ovals.
const CONTROL_RADIUS: f32 = 18.;
const NAV_LIGHT_TINT_ALPHA: u8 = 105;
const NAV_BLUR: f32 = 1.1;

#[derive(Clone, Copy)]
enum GlassSurface {
    Panel {
        light: bool,
        blur: f32,
        navigation_backing: bool,
    },
    /// `lift` is 0 at rest and 1 while the lens is held: a held lens clears and bends more.
    Selector {
        light: bool,
        lift: f32,
        finger: Option<(f32, f32)>,
    },
}

thread_local! {
    static LENS: OnceCell<Option<RuntimeEffect>> = const { OnceCell::new() };
}

/// Corners for buttons, fields and menu triggers: the capsule in PolyGlass, and the theme's
/// own `fallback` radius everywhere else.
pub fn control_radius(fallback: f32) -> CornerRadius {
    if super::colors::active_theme().liquid_glass {
        CornerRadius::new_all(CONTROL_RADIUS)
    } else {
        super::rounded(fallback)
    }
}

/// Recessed well a selector lens slides in. It carries no outline of its own, so the lens
/// contour is the only edge drawn inside it.
pub fn track_fill() -> Color {
    if super::colors::is_light() {
        super::colors::component_bg().with_a(130)
    } else {
        super::colors::ghost_overlay_hover()
    }
}

/// Content sits on the already-diffused page art. A translucent face and a single
/// directional hairline give it depth without a separate backdrop filter per row.
pub fn content(rect: Rect, fill: Color, radius: f32) -> Rect {
    use skia_safe::{PaintStyle, gradient};
    let light = super::colors::is_light();
    let fill = Paint::new(
        Color4f::from(fill.with_a(if light { 164 } else { 152 })),
        None,
    );
    let stops = [
        Color4f::from(Color::WHITE.with_a(if light { 175 } else { 68 })),
        Color4f::from(Color::WHITE.with_a(if light { 38 } else { 8 })),
        Color4f::from(Color::BLACK.with_a(if light { 36 } else { 58 })),
    ];
    let mut rim = Paint::default();
    rim.set_anti_alias(true)
        .set_style(PaintStyle::Stroke)
        .set_stroke_width(1.);
    let mut cached_size = (0., 0.);
    rect.corner_radius(CornerRadius::new_all(radius))
        .background(RenderCallback::new(move |ctx: &mut FillRenderContext| {
            let RenderTarget::Background { canvas, .. } = &ctx.target else {
                return;
            };
            let (w, h) = (ctx.size.width, ctx.size.height);
            if w <= 1. || h <= 1. {
                return;
            }
            if cached_size != (w, h) {
                let ramp = gradient::Gradient::new(
                    gradient::Colors::new(&stops, Some(&[0., 0.45, 1.]), TileMode::Clamp, None),
                    gradient::Interpolation::default(),
                );
                rim.set_shader(gradient::shaders::linear_gradient(
                    ((w * 0.25, 0.), (w * 0.75, h)),
                    &ramp,
                    None,
                ));
                cached_size = (w, h);
            }
            canvas.draw_round_rect(SkRect::from_wh(w, h), radius, radius, &fill);
            let edge = SkRect::from_ltrb(0.5, 0.5, w - 0.5, h - 0.5);
            canvas.draw_round_rect(edge, radius - 0.5, radius - 0.5, &rim);
        }))
}

pub fn panel(rect: Rect, theme: &Theme, radius: f32, tint: Color) -> Rect {
    panel_with_blur(rect, theme, radius, tint, BACKDROP_BLUR)
}

/// A reading surface over detailed artwork or moving list text needs more diffusion.
pub fn frosted_panel(rect: Rect, theme: &Theme, radius: f32, tint: Color) -> Rect {
    panel_with_blur(rect, theme, radius, tint, 3.)
}

/// Blur under the large workspace dialogs. Strong enough that the Home hero behind them (its
/// title and the blue Launch button) can't be read as part of the dialog.
const WORKSPACE_BLUR: f32 = 22.;
/// Blur under small dialogs and menus
const POPUP_BLUR: f32 = 8.;

/// Diffuse the page behind large dialogs so background text cannot compete with their controls.
pub fn workspace_panel(rect: Rect, theme: &Theme, radius: f32, tint: Color) -> Rect {
    panel_with_blur(rect, theme, radius, tint, WORKSPACE_BLUR)
}

/// Dense glass for small dialogs and menus where fine text sits over busy artwork.
pub fn popup_panel(rect: Rect, radius: f32, fallback: Color) -> Rect {
    let theme = super::colors::active_theme();
    if theme.liquid_glass {
        let tint =
            super::colors::page_elevated().with_a(if theme.colors.light { 226 } else { 212 });
        panel_with_blur(rect, &theme, super::rounded_px(radius), tint, POPUP_BLUR)
    } else {
        rect.corner_radius(super::rounded(radius))
            .background(fallback)
    }
}

pub fn navigation_panel(rect: Rect, theme: &Theme, radius: f32) -> Rect {
    if !theme.colors.light {
        return panel_with_blur(
            rect,
            theme,
            radius,
            super::colors::page().with_a(108),
            NAV_BLUR,
        );
    }
    render_panel(
        rect,
        theme,
        radius,
        Color::WHITE.with_a(NAV_LIGHT_TINT_ALPHA),
        GlassSurface::Panel {
            light: true,
            blur: NAV_BLUR,
            navigation_backing: true,
        },
    )
}

fn panel_with_blur(rect: Rect, theme: &Theme, radius: f32, tint: Color, blur: f32) -> Rect {
    render_panel(
        rect,
        theme,
        radius,
        tint,
        GlassSurface::Panel {
            light: theme.colors.light,
            blur,
            navigation_backing: false,
        },
    )
}

fn render_panel(
    rect: Rect,
    theme: &Theme,
    radius: f32,
    tint: Color,
    surface: GlassSurface,
) -> Rect {
    if !theme.liquid_glass {
        return rect.background(tint);
    }

    let tint = Color4f::from(tint);
    // Retain the filter, never the sampled image: stationary glass still refracts live content.
    let mut filter_size = (0., 0.);
    let mut filter = None;
    rect.corner_radius(CornerRadius::new_all(radius))
        .background(RenderCallback::new(move |ctx: &mut FillRenderContext| {
            let RenderTarget::Background { canvas, .. } = &ctx.target else {
                return;
            };
            let (w, h) = (ctx.size.width, ctx.size.height);
            if w <= 1. || h <= 1. {
                return;
            }
            let area = SkRect::from_wh(w, h);
            let radius = radius.min(w.min(h) * 0.5);
            if filter_size != (w, h) {
                filter = lens(w, h, radius, tint, surface);
                filter_size = (w, h);
            }
            if let Some(filter) = &filter {
                let layer =
                    canvas.save_layer(&SaveLayerRec::default().bounds(&area).backdrop(filter));
                canvas.restore_to_count(layer);
            } else {
                canvas.draw_rect(area, &Paint::new(tint, None));
            }
        }))
}

fn lens(w: f32, h: f32, radius: f32, tint: Color4f, surface: GlassSurface) -> Option<ImageFilter> {
    let effect = LENS.with(|effect| {
        effect
            .get_or_init(|| {
                RuntimeEffect::make_for_shader(include_str!("liquid_glass.sksl"), None)
                    .inspect_err(|error| tracing::error!("Could not compile PolyGlass: {error}"))
                    .ok()
            })
            .clone()
    })?;
    let mut builder = RuntimeShaderBuilder::new(effect);
    let (band, refraction, dispersion, blur, finger) = match surface {
        GlassSurface::Panel { blur, .. } => (
            BEVEL_WIDTH.min(h.min(w) * 0.30),
            REFRACTION,
            DISPERSION,
            blur,
            None,
        ),
        // The bar or track already diffuses the scene. Keep small details for the moving lens.
        GlassSurface::Selector { lift, finger, .. } => (
            h.min(w) * 0.30,
            1.9 + 0.4 * lift.clamp(0., 1.),
            0.035,
            0.,
            finger,
        ),
    };
    builder.set_uniform_float("size", &[w, h]).ok()?;
    builder.set_uniform_float("radius", &[radius]).ok()?;
    builder.set_uniform_float("band", &[band]).ok()?;
    let ink = match surface {
        GlassSurface::Panel { light: true, .. } => -1.,
        GlassSurface::Panel { light: false, .. } => 1.,
        GlassSurface::Selector { .. } => 0.,
    };
    builder.set_uniform_float("ink", &[ink]).ok()?;
    let navigation_backing = matches!(
        surface,
        GlassSurface::Panel {
            navigation_backing: true,
            ..
        }
    );
    builder
        .set_uniform_float("navbarBacking", &[if navigation_backing { 1. } else { 0. }])
        .ok()?;
    // Highlight and shade strength of the one edge hairline. A white line vanishes on pale
    // surfaces, so light themes lean on the shade to keep the shape defined.
    let (contour, vibrance) = match surface {
        GlassSurface::Panel {
            navigation_backing: true,
            ..
        } => ([0.94, 0.18], 0.83),
        GlassSurface::Panel { light: true, .. } => ([0.74, 0.11], 1.42),
        GlassSurface::Panel { light: false, .. } => ([0.36, 0.], 1.14),
        GlassSurface::Selector { light: true, .. } => ([0.75, 0.14], 1.),
        GlassSurface::Selector { light: false, .. } => ([0.40, 0.], 1.),
    };
    builder.set_uniform_float("contour", &contour).ok()?;
    builder.set_uniform_float("vibrance", &[vibrance]).ok()?;
    builder
        .set_uniform_float("refraction", &[refraction])
        .ok()?;
    builder
        .set_uniform_float("dispersion", &[dispersion])
        .ok()?;
    builder
        .set_uniform_float("tint", &[tint.r, tint.g, tint.b, tint.a])
        .ok()?;
    let finger = finger.map_or([0., 0., 0.], |(x, y)| [x, y, 1.]);
    builder.set_uniform_float("finger", &finger).ok()?;
    let blur = if blur > 0. {
        image_filters::blur((blur, blur), TileMode::Clamp, None, None)
    } else {
        None
    };
    image_filters::runtime_shader_with_options(
        &builder,
        band * refraction * (1. + dispersion),
        "backdrop",
        blur,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use skia_safe::{ClipOp, Color as SkColor, RRect, surfaces};

    #[test]
    fn selector_refracts_side_detail_without_duplicating_the_track_rim() {
        let mut surface = surfaces::raster_n32_premul((128, 72)).unwrap();
        let canvas = surface.canvas();
        canvas.clear(SkColor::BLACK);
        let white = Paint::new(Color4f::from(SkColor::WHITE), None);
        canvas.draw_rect(SkRect::from_ltrb(0., 0., 128., 16.), &white);
        canvas.draw_rect(SkRect::from_ltrb(0., 52., 128., 72.), &white);
        canvas.draw_rect(SkRect::from_ltrb(0., 20., 16., 48.), &white);
        canvas.draw_rect(SkRect::from_ltrb(63., 25., 64., 45.), &white);
        let saved = canvas.save();
        canvas.translate((16., 16.));
        let area = SkRect::from_wh(96., 36.);
        canvas.clip_rrect(RRect::new_rect_xy(area, 18., 18.), ClipOp::Intersect, true);
        let filter = lens(
            96.,
            36.,
            18.,
            Color4f::from(SkColor::TRANSPARENT),
            GlassSurface::Selector {
                light: false,
                lift: 0.,
                finger: None,
            },
        )
        .unwrap();
        let layer = canvas.save_layer(&SaveLayerRec::default().bounds(&area).backdrop(&filter));
        canvas.restore_to_count(layer);
        canvas.restore_to_count(saved);
        let pixels = surface.peek_pixels().unwrap();
        assert!(pixels.get_color((64, 18)).g() < 50);
        assert!(pixels.get_color((64, 49)).g() < 50);
        assert!(pixels.get_color((18, 34)).g() > 170);
        assert_eq!(pixels.get_color((63, 34)), SkColor::WHITE);
        assert_eq!(pixels.get_color((64, 34)), SkColor::BLACK);
    }

    #[test]
    fn light_navigation_backing_is_even_across_the_face_and_stays_translucent() {
        let mut surface = surfaces::raster_n32_premul((288, 168)).unwrap();
        let canvas = surface.canvas();
        canvas.clear(SkColor::from_rgb(40, 40, 40));

        let saved = canvas.save();
        canvas.translate((24., 24.));
        let area = SkRect::from_wh(240., 120.);
        canvas.clip_rrect(RRect::new_rect_xy(area, 24., 24.), ClipOp::Intersect, true);
        let filter = lens(
            240.,
            120.,
            24.,
            Color4f::from(SkColor::from_argb(NAV_LIGHT_TINT_ALPHA, 255, 255, 255)),
            GlassSurface::Panel {
                light: true,
                blur: NAV_BLUR,
                navigation_backing: true,
            },
        )
        .unwrap();
        let layer = canvas.save_layer(&SaveLayerRec::default().bounds(&area).backdrop(&filter));
        canvas.restore_to_count(layer);
        canvas.restore_to_count(saved);

        let pixels = surface.peek_pixels().unwrap();
        let title = pixels.get_color((24 + 60, 24 + 40)).g() as i32;
        let right = pixels.get_color((24 + 200, 24 + 40)).g() as i32;
        let bottom = pixels.get_color((24 + 60, 24 + 96)).g() as i32;
        assert!((title - right).abs() <= 2, "left {title} right {right}");
        assert!((title - bottom).abs() <= 2, "top {title} bottom {bottom}");
        // Keep dark navigation ink legible while retaining visible backdrop color.
        assert!(title > 145 && title < 190, "face {title}");
    }

    #[test]
    fn lens_bends_backdrop_at_the_bevel_and_leaves_the_center_in_place() {
        let mut surface = surfaces::raster_n32_premul((208, 128)).unwrap();
        let canvas = surface.canvas();
        canvas.clear(SkColor::BLACK);
        let white = Paint::new(Color4f::from(SkColor::WHITE), None);
        canvas.draw_rect(SkRect::from_ltrb(0., 0., 208., 26.), &white);
        canvas.draw_rect(SkRect::from_ltrb(0., 105., 208., 128.), &white);

        let saved = canvas.save();
        canvas.translate((24., 32.));
        let area = SkRect::from_wh(160., 64.);
        canvas.clip_rrect(RRect::new_rect_xy(area, 14., 14.), ClipOp::Intersect, true);
        let filter = lens(
            160.,
            64.,
            14.,
            Color4f::from(SkColor::TRANSPARENT),
            GlassSurface::Panel {
                light: false,
                blur: BACKDROP_BLUR,
                navigation_backing: false,
            },
        )
        .unwrap();
        let layer = canvas.save_layer(&SaveLayerRec::default().bounds(&area).backdrop(&filter));
        canvas.restore_to_count(layer);
        canvas.restore_to_count(saved);

        let pixels = surface.peek_pixels().unwrap();
        assert!(pixels.get_color((104, 32)).g() > 190);
        assert!(pixels.get_color((104, 95)).g() > 190);
        assert_eq!(pixels.get_color((104, 64)), SkColor::BLACK);
        assert_eq!(pixels.get_color((24, 32)), SkColor::BLACK);
    }
}
