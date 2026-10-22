use std::any::Any;
use std::borrow::Cow;
use std::rc::Rc;

use freya::elements::image::ImageHandle;
use freya::engine::prelude::{ClipOp, FontMgr, Paint, Picture, PictureRecorder, SkRect, svg};
use freya::prelude::*;
use freya_core::element::{ClipContext, ElementExt, LayoutContext};
use freya_core::tree::DiffModifies;

use super::super::local_image::decode;
use super::svg_compat::prepare_svg;
use crate::hooks::{loaded_image, use_cached_image};
use crate::theme::colors;

const MAX_EDGE: u32 = 1600;

#[derive(PartialEq)]
pub struct MarkdownImage {
    url: String,
    alt: String,
}

impl MarkdownImage {
    pub fn new(url: impl Into<String>, alt: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            alt: alt.into(),
        }
    }
}

impl Component for MarkdownImage {
    fn render(&self) -> impl IntoElement {
        let query = use_cached_image(Some(self.url.clone()), MAX_EDGE);
        let loaded = loaded_image(Some(&self.url), &query);

        let mut cache = use_state(|| None::<(usize, ImageContent)>);
        let content = loaded.and_then(|(_, bytes)| {
            let ptr = bytes.as_ptr() as usize;

            if let Some((cached_ptr, content)) = cache.read().clone()
                && cached_ptr == ptr
            {
                return Some(content);
            }

            let content = decode_any(&bytes)?;
            cache.set(Some((ptr, content.clone())));

            Some(content)
        });

        match content {
            Some(content) => scaled_content(content)
                .a11y_role(AccessibilityRole::Image)
                .a11y_alt(self.alt.clone())
                .into_element(),
            None if self.alt.is_empty() => rect().into_element(),
            None => label()
                .text(self.alt.clone())
                .color(colors::fg_secondary())
                .into_element(),
        }
    }
}

#[derive(Clone, PartialEq)]
enum ImageContent {
    Bitmap(ImageHandle),
    Vector(SvgPicture),
}

/// An svg recorded as drawing commands, so it is drawn straight onto the
/// window's pixel grid instead of being resampled from a bitmap
#[derive(Clone)]
struct SvgPicture {
    picture: Picture,
    /// Logical size, scaled to physical pixels at layout like any `Size::px`
    size: Size2D,
}

impl PartialEq for SvgPicture {
    fn eq(&self, other: &Self) -> bool {
        self.picture.unique_id() == other.picture.unique_id() && self.size == other.size
    }
}

fn decode_any(bytes: &Bytes) -> Option<ImageContent> {
    if let Some(handle) = decode(bytes) {
        return Some(ImageContent::Bitmap(handle));
    }

    // Skia's codecs can't read svg, which is what shields.io badges are
    if !looks_like_svg(bytes) {
        return None;
    }

    record_svg(bytes).map(ImageContent::Vector)
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let head = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    let start = head
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(head.len());
    let head = &head[start..];

    head.starts_with(b"<svg")
        || (head.starts_with(b"<?xml")
            || head.starts_with(b"<!--")
            || head.starts_with(b"<!DOCTYPE"))
            && head.windows(4).any(|w| w == b"<svg")
}

fn svg_size(root: &svg::Svg) -> Option<Size2D> {
    let intrinsic = root.intrinsic_size();
    let size = if intrinsic.width > 0. && intrinsic.height > 0. {
        Size2D::new(intrinsic.width, intrinsic.height)
    } else {
        let view_box = root.view_box()?;
        Size2D::new(view_box.width(), view_box.height())
    };

    let valid = size.width.is_finite() && size.height.is_finite();
    (valid && size.width > 0. && size.height > 0.).then_some(size)
}

/// Badges are mostly text, so the dom gets the system fonts rather than an empty manager
fn record_svg(bytes: &[u8]) -> Option<SvgPicture> {
    let mut dom = svg::Dom::from_bytes(&prepare_svg(bytes), FontMgr::default()).ok()?;
    let declared = svg_size(&dom.root())?;
    dom.set_container_size((declared.width, declared.height));

    // the dom always draws at its declared size, so an oversized svg is scaled down while recording
    let shrink = (MAX_EDGE as f32 / declared.width.max(declared.height)).min(1.);
    let size = declared * shrink;

    let bounds = SkRect::from_wh(size.width, size.height);
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(bounds, false);
    canvas.clip_rect(bounds, None, None);
    canvas.scale((shrink, shrink));
    dom.render(canvas);
    let picture = recorder.finish_recording_as_picture(Some(&bounds))?;

    Some(SvgPicture { picture, size })
}

#[derive(Clone)]
pub struct ScaledImage {
    key: DiffKey,
    element: ScaledImageElement,
}

#[derive(Clone, PartialEq)]
pub struct ScaledImageElement {
    accessibility: AccessibilityData,
    layout: LayoutData,
    content: ImageContent,
    sampling_mode: SamplingMode,
    corner_radius: CornerRadius,
}

fn scaled_content(content: ImageContent) -> ScaledImage {
    ScaledImage {
        key: DiffKey::None,
        element: ScaledImageElement {
            accessibility: AccessibilityData::default(),
            layout: LayoutData::default(),
            content,
            sampling_mode: SamplingMode::default(),
            corner_radius: CornerRadius::default(),
        },
    }
}

impl ScaledImage {
    pub fn sampling_mode(mut self, sampling_mode: SamplingMode) -> Self {
        self.element.sampling_mode = sampling_mode;
        self
    }

    pub fn corner_radius(mut self, corner_radius: CornerRadius) -> Self {
        self.element.corner_radius = corner_radius;
        self
    }
}

impl KeyExt for ScaledImage {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl LayoutExt for ScaledImage {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.element.layout
    }
}

impl AccessibilityExt for ScaledImage {
    fn get_accessibility_data(&mut self) -> &mut AccessibilityData {
        &mut self.element.accessibility
    }
}

impl MaybeExt for ScaledImage {}

impl From<ScaledImage> for Element {
    fn from(value: ScaledImage) -> Self {
        Element::Element {
            key: value.key,
            element: Rc::new(value.element),
            elements: Vec::new(),
        }
    }
}

impl ImageContent {
    /// Size in layout (physical) pixels; bitmaps keep their pixel dimensions
    fn size(&self, scale_factor: f32) -> Size2D {
        match self {
            Self::Bitmap(handle) => {
                Size2D::new(handle.image.width() as f32, handle.image.height() as f32)
            }
            Self::Vector(svg) => svg.size * scale_factor,
        }
    }
}

impl ElementExt for ScaledImageElement {
    fn changed(&self, other: &Rc<dyn ElementExt>) -> bool {
        let Some(other) = (other.as_ref() as &dyn Any).downcast_ref::<ScaledImageElement>() else {
            return false;
        };
        self != other
    }

    fn diff(&self, other: &Rc<dyn ElementExt>) -> DiffModifies {
        let Some(other) = (other.as_ref() as &dyn Any).downcast_ref::<ScaledImageElement>() else {
            return DiffModifies::all();
        };

        let mut diff = DiffModifies::empty();

        if self.accessibility != other.accessibility {
            diff.insert(DiffModifies::ACCESSIBILITY);
        }

        if self.layout != other.layout {
            diff.insert(DiffModifies::LAYOUT);
        }

        if self.content != other.content {
            diff.insert(DiffModifies::STYLE);

            if self.content.size(1.) != other.content.size(1.) {
                diff.insert(DiffModifies::LAYOUT);
            }
        }

        if self.sampling_mode != other.sampling_mode || self.corner_radius != other.corner_radius {
            diff.insert(DiffModifies::STYLE);
        }

        diff
    }

    fn layout(&'_ self) -> Cow<'_, LayoutData> {
        Cow::Borrowed(&self.layout)
    }

    fn accessibility(&'_ self) -> Cow<'_, AccessibilityData> {
        Cow::Borrowed(&self.accessibility)
    }

    fn style(&'_ self) -> Cow<'_, StyleState> {
        Cow::Owned(StyleState {
            corner_radius: self.corner_radius,
            ..StyleState::default()
        })
    }

    fn should_hook_measurement(&self) -> bool {
        true
    }

    fn measure(&self, context: LayoutContext) -> Option<(Size2D, Rc<dyn Any>)> {
        let natural = self.content.size(context.scale_factor as f32);

        let available = (*context.area_size - context.torin_node.margin.into()).max(Size2D::zero());

        let ratio = (available.width / natural.width).min(1.);
        let ratio = if ratio.is_finite() && ratio > 0. {
            ratio
        } else {
            1.
        };

        let size = Size2D::new(natural.width * ratio, natural.height * ratio);

        Some((size, Rc::new(size)))
    }

    fn clip(&self, context: ClipContext) {
        let rrect = self.render_rect(context.visible_area, context.scale_factor as f32);
        context.canvas.clip_rrect(rrect, ClipOp::Intersect, true);
    }

    fn render(&self, context: RenderContext) {
        let Some(size) = context
            .layout_node
            .data
            .as_ref()
            .and_then(|data| data.downcast_ref::<Size2D>())
        else {
            return;
        };

        let area = context.layout_node.visible_area();
        let rect = SkRect::new(
            area.min_x(),
            area.min_y(),
            area.min_x() + size.width,
            area.min_y() + size.height,
        );

        context.canvas.save();
        let clip_rrect = self.render_rect(&area, context.scale_factor as f32);
        context
            .canvas
            .clip_rrect(clip_rrect, ClipOp::Intersect, true);

        match &self.content {
            ImageContent::Bitmap(handle) => {
                let mut paint = Paint::default();
                paint.set_anti_alias(true);

                context.canvas.draw_image_rect_with_sampling_options(
                    &handle.image,
                    None,
                    rect,
                    self.sampling_mode.sampling_options(),
                    &paint,
                );
            }
            ImageContent::Vector(svg) => {
                context.canvas.translate((rect.left, rect.top));
                context.canvas.scale((
                    rect.width() / svg.size.width,
                    rect.height() / svg.size.height,
                ));
                context.canvas.draw_picture(&svg.picture, None, None);
            }
        }

        context.canvas.restore();
    }
}

#[cfg(test)]
mod tests {
    use freya::engine::prelude::AlphaType;
    use freya::prelude::Bytes;
    use freya_testing::prelude::*;

    use super::*;

    fn handle(width: u32, height: u32) -> ImageHandle {
        let pixels = Bytes::from(vec![200u8; (width * height * 4) as usize]);
        ImageHandle::from_rgba(width, height, pixels, AlphaType::Opaque).expect("rgba handle")
    }

    fn measured(panel: (f32, f32), image: (u32, u32)) -> Size2D {
        let app = move || {
            rect()
                .width(Size::px(panel.0))
                .height(Size::px(panel.1))
                .child(scaled_content(ImageContent::Bitmap(handle(
                    image.0, image.1,
                ))))
        };

        let test = launch_test(app);
        test.find(|node, element| {
            (element as &dyn Any)
                .downcast_ref::<ScaledImageElement>()
                .map(|_| node.layout())
        })
        .expect("image node")
        .area
        .size
    }

    #[test]
    fn scales_an_image_inlined_in_a_paragraph() {
        let app = || {
            rect().width(Size::px(300.)).height(Size::px(400.)).child(
                paragraph()
                    .span(Span::new("before "))
                    .child(scaled_content(ImageContent::Bitmap(handle(600, 200))))
                    .span(Span::new(" after")),
            )
        };

        let test = launch_test(app);
        let size = test
            .find(|node, element| {
                (element as &dyn Any)
                    .downcast_ref::<ScaledImageElement>()
                    .map(|_| node.layout())
            })
            .expect("image node")
            .area
            .size;

        assert_eq!(size, Size2D::new(300., 100.));
    }

    #[test]
    fn scales_a_wide_image_down_to_the_panel() {
        assert_eq!(measured((300., 400.), (600, 200)), Size2D::new(300., 100.));
    }

    #[test]
    fn leaves_a_small_image_at_its_natural_size() {
        assert_eq!(measured((300., 400.), (88, 31)), Size2D::new(88., 31.));
    }

    #[test]
    fn short_panel_does_not_shrink_the_width() {
        assert_eq!(measured((300., 60.), (600, 200)), Size2D::new(300., 100.));
    }

    const BADGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="88" height="20"><rect width="88" height="20" fill="#007ec6"/><text x="10" y="14" font-family="Verdana" font-size="11">v3.0.1</text></svg>"##;

    #[test]
    fn sniffs_svg_documents() {
        assert!(looks_like_svg(BADGE.as_bytes()));
        assert!(looks_like_svg(
            b"\xEF\xBB\xBF  <?xml version=\"1.0\"?>\n<svg width=\"1\" height=\"1\"/>"
        ));
        assert!(!looks_like_svg(b"\x89PNG\r\n\x1a\n"));
        assert!(!looks_like_svg(b"<html><body>not found</body></html>"));
    }

    #[test]
    fn records_a_badge_at_its_declared_size() {
        let svg = record_svg(BADGE.as_bytes()).expect("badge records");
        assert_eq!(svg.size, Size2D::new(88., 20.));
    }

    #[test]
    fn falls_back_to_the_view_box() {
        let svg = record_svg(
            br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10" width="100%"/>"#,
        )
        .expect("view box svg records");

        assert_eq!(svg.size, Size2D::new(40., 10.));
    }

    fn vector_size(scale_factor: f64) -> Size2D {
        let svg = record_svg(BADGE.as_bytes()).expect("badge records");
        let app = move || {
            rect()
                .width(Size::px(300.))
                .height(Size::px(400.))
                .child(scaled_content(ImageContent::Vector(svg.clone())))
        };

        let (test, _) = TestingRunner::new(app, Size2D::new(500., 500.), |_| {}, scale_factor);
        test.find(|node, element| {
            (element as &dyn Any)
                .downcast_ref::<ScaledImageElement>()
                .map(|_| node.layout())
        })
        .expect("image node")
        .area
        .size
    }

    #[test]
    fn a_vector_follows_the_scale_factor_like_text() {
        assert_eq!(vector_size(1.), Size2D::new(88., 20.));
        assert_eq!(vector_size(1.5), Size2D::new(132., 30.));
    }

    #[test]
    fn a_vector_at_a_fractional_offset_is_not_resampled() {
        // grey ends at 39.5, so after the 10.5 offset the colour edge lands on pixel 50
        let svg = record_svg(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="88" height="20"><rect width="39.5" height="20" fill="#555555"/><rect x="39.5" width="48.5" height="20" fill="#007ec6"/></svg>"##,
        )
        .expect("svg records");
        let app = move || {
            rect()
                .horizontal()
                .child(rect().width(Size::px(10.5)).height(Size::px(1.)))
                .child(scaled_content(ImageContent::Vector(svg.clone())))
        };

        let mut test = launch_test(app);
        let png = test.render();
        let snapshot = image::load_from_memory(png.as_bytes())
            .expect("snapshot decodes")
            .to_rgb8();
        let rgb = |x: u32| {
            let [r, g, b] = snapshot.get_pixel(x, 10).0;
            (r, g, b)
        };

        assert_eq!(rgb(49), (0x55, 0x55, 0x55));
        assert_eq!(rgb(50), (0x00, 0x7e, 0xc6));
    }

    /// A grey shields-style badge with a 14px logo slot, rendered at the origin
    fn logo_pixel(href: &str) -> (u8, u8, u8) {
        let badge = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#555555"/><image x="5" y="3" width="14" height="14" href="{href}"/></svg>"##
        );
        let svg = record_svg(badge.as_bytes()).expect("badge records");
        let app = move || rect().child(scaled_content(ImageContent::Vector(svg.clone())));

        let mut test = launch_test(app);
        let png = test.render();
        let snapshot = image::load_from_memory(png.as_bytes())
            .expect("snapshot decodes")
            .to_rgb8();
        let [r, g, b] = snapshot.get_pixel(12, 10).0;
        (r, g, b)
    }

    fn data_url(mime: &str, bytes: &[u8]) -> String {
        use base64::Engine;
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    #[test]
    fn draws_a_png_logo_given_by_href() {
        let mut png = Vec::new();
        image::RgbImage::from_pixel(4, 4, image::Rgb([255, 0, 0]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png encodes");

        assert_eq!(logo_pixel(&data_url("image/png", &png)), (255, 0, 0));
    }

    #[test]
    fn draws_an_svg_logo_given_as_a_data_url() {
        let logo = br##"<svg fill="#5865F2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path d="M0 0h24v24H0z"/></svg>"##;

        assert_eq!(
            logo_pixel(&data_url("image/svg+xml", logo)),
            (0x58, 0x65, 0xf2)
        );
    }

    /// Replays a recorded svg at its own size on white and reads one pixel
    fn picture_pixel(svg: &SvgPicture, x: u32, y: u32) -> (u8, u8, u8) {
        use freya::engine::prelude::{EncodedImageFormat, raster_n32_premul};

        let size = (svg.size.width.ceil() as i32, svg.size.height.ceil() as i32);
        let mut surface = raster_n32_premul(size).expect("surface");
        surface.canvas().clear(Color::WHITE);
        surface.canvas().draw_picture(&svg.picture, None, None);
        let png = surface
            .image_snapshot()
            .encode(None, EncodedImageFormat::PNG, None)
            .expect("png encodes");
        let [r, g, b] = image::load_from_memory(png.as_bytes())
            .expect("png decodes")
            .to_rgb8()
            .get_pixel(x, y)
            .0;
        (r, g, b)
    }

    #[test]
    fn an_oversized_svg_is_scaled_down_rather_than_cropped() {
        let svg = record_svg(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="3200" height="100"><rect width="1600" height="100" fill="#ff0000"/><rect x="1600" width="1600" height="100" fill="#0000ff"/></svg>"##,
        )
        .expect("svg records");

        assert_eq!(svg.size, Size2D::new(1600., 50.));
        assert_eq!(picture_pixel(&svg, 10, 40), (255, 0, 0));
        assert_eq!(picture_pixel(&svg, 1590, 40), (0, 0, 255));
    }

    #[test]
    fn a_logo_reusing_the_badges_ids_leaves_the_badge_intact() {
        let logo = br##"<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><defs><clipPath id="r"><rect width="1" height="1"/></clipPath></defs><rect width="24" height="24" fill="#00ff00" clip-path="url(#r)"/></svg>"##;
        let badge = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="20"><clipPath id="r"><rect width="80" height="20" rx="3"/></clipPath><g clip-path="url(#r)"><rect width="80" height="20" fill="#555555"/></g><image x="5" y="3" width="14" height="14" href="{}"/></svg>"##,
            data_url("image/svg+xml", logo)
        );
        let svg = record_svg(badge.as_bytes()).expect("badge records");

        assert_eq!(picture_pixel(&svg, 60, 10), (0x55, 0x55, 0x55));
    }
}
