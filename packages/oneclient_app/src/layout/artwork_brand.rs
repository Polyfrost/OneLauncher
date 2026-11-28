use freya::prelude::*;
use skia_safe::{Canvas, Color4f, Data, Paint, Rect as SkRect, RuntimeEffect, TileMode, gradient};

#[derive(Clone, Copy)]
pub(crate) struct ArtworkBrand {
    pub bounds: State<Option<SkRect>>,
}

impl ArtworkBrand {
    pub fn provide() {
        let bounds = use_state(|| None);
        use_hook(|| provide_context(Self { bounds }));
    }
    pub fn get() -> Self {
        use_hook(consume_context::<Self>)
    }
}

// Compress just the bright part of the illustration under the white mark. Dark art
// retains its original blacks. The source alpha is the soft corner mask.
const TONE_CURVE: &str = r#"
half4 main(half4 src, half4 dst) {
    float3 rgb = dst.rgb / max(float(dst.a), 0.001);
    float lum = dot(pow(rgb, float3(2.2)), float3(0.2126, 0.7152, 0.0722));
    float knee = 0.026;
    float excess = max(lum - knee, 0.0);
    float shoulder = 0.050;
    float target = min(lum, knee) + shoulder * (1.0 - exp(-excess / shoulder));
    float gain = pow(min(target / max(lum, 0.0001), 1.0), 1.0 / 2.2);
    return half4(dst.rgb * mix(1.0, gain, float(src.a)), dst.a);
}
"#;

pub(super) fn contrast_paint() -> Paint {
    let blender = RuntimeEffect::make_for_blender(TONE_CURVE, None)
        .expect("brand tone curve")
        .make_blender(Data::new_empty(), None)
        .expect("brand blender");
    let stops = [
        Color4f::new(0., 0., 0., 1.),
        Color4f::new(0., 0., 0., 1.),
        Color4f::new(0., 0., 0., 0.5),
        Color4f::new(0., 0., 0., 0.),
    ];
    let ramp = gradient::Gradient::new(
        gradient::Colors::new(&stops, Some(&[0., 0.62, 0.8, 1.]), TileMode::Clamp, None),
        gradient::Interpolation::default(),
    );
    let mut paint = Paint::default();
    paint
        .set_shader(gradient::shaders::radial_gradient(
            ((0., 0.), 1.),
            &ramp,
            None,
        ))
        .set_blender(blender);
    paint
}

pub(super) fn draw_contrast(canvas: &Canvas, area: SkRect, paint: &Paint) {
    let saved = canvas.save();
    // Keep the falloff close to the symbol when the wordmark is collapsed.
    let width = if area.width() < 80. {
        area.right * 1.55 + 18.
    } else {
        area.right * 1.85 + 42.
    };
    canvas.scale((width, area.bottom * 2.5 + 20.));
    canvas.draw_rect(SkRect::from_wh(1., 1.), paint);
    canvas.restore_to_count(saved);
}

pub(super) fn artwork_crop(iw: f32, ih: f32, w: f32, h: f32, pan: (f32, f32)) -> SkRect {
    let scale = (iw / w).min(ih / h) / 1.10;
    let (cw, ch) = (w * scale, h * scale);
    let x = (iw - cw) * 0.5 - pan.0 * cw * 0.005;
    let y = (ih - ch) * 0.5 - pan.1 * ch * 0.005;
    SkRect::from_xywh(x, y, cw, ch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use skia_safe::{Color, surfaces};

    #[test]
    fn compact_mask_keeps_the_star_readable_and_leaves_nearby_art_intact() {
        let bounds = SkRect::from_xywh(24., 18., 40., 40.);
        let mut surface = surfaces::raster_n32_premul((250, 150)).unwrap();
        surface.canvas().clear(Color::WHITE);
        draw_contrast(surface.canvas(), bounds, &contrast_paint());
        let pixels = surface.peek_pixels().unwrap();
        assert!(pixels.get_color((58, 50)).r() <= 115);
        assert_eq!(pixels.get_color((120, 50)), Color::WHITE);
    }

    #[test]
    fn protects_the_wordmark_on_white_and_keeps_dark_art_and_the_far_edge_intact() {
        let area = SkRect::from_xywh(24., 22., 206., 35.);
        let paint = contrast_paint();
        for base in [Color::WHITE, Color::from_rgb(20, 25, 30)] {
            let mut surface = surfaces::raster_n32_premul((600, 200)).unwrap();
            surface.canvas().clear(base);
            draw_contrast(surface.canvas(), area, &paint);
            let pix = surface.peek_pixels().unwrap();
            let under = pix.get_color((226, 52));
            if base == Color::WHITE {
                assert!(under.r() <= 100);
            } else {
                assert_eq!(under, base);
            }
            assert_eq!(pix.get_color((550, 40)), base);
        }
    }
}
