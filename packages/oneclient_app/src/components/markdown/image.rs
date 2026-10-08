use std::any::Any;
use std::borrow::Cow;
use std::rc::Rc;

use freya::elements::image::ImageHandle;
use freya::engine::prelude::{ClipOp, FontMgr, Paint, SkRect, raster_n32_premul, svg};
use freya::prelude::*;
use freya_core::element::{ClipContext, ElementExt, LayoutContext};
use freya_core::tree::DiffModifies;

use super::super::local_image::decode;
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

        let mut cache = use_state(|| None::<(usize, ImageHandle)>);
        let holder = loaded.and_then(|(_, bytes)| {
            let ptr = bytes.as_ptr() as usize;

            if let Some((cached_ptr, holder)) = cache.read().clone()
                && cached_ptr == ptr
            {
                return Some(holder);
            }

            // Descriptions carry badges as SVG, which the raster decoder rejects
            let holder = decode(&bytes).or_else(|| decode_svg(&bytes))?;
            cache.set(Some((ptr, holder.clone())));

            Some(holder)
        });

        match holder {
            Some(holder) => scaled_image(holder)
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

/// The size lives in the root element, so only the opening tag has to be read
const SVG_HEAD: usize = 4096;

/// A description must not be able to ask for a huge surface
const SVG_MAX_EDGE: u32 = 2048;

/// The size an SVG asks to be drawn at, so it lays out like any other image
fn svg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let end = bytes.len().min(SVG_HEAD);
    let head = String::from_utf8_lossy(&bytes[..end]);
    let start = head.find("<svg")?;
    let tag = head.get(start..start + head[start..].find('>')? + 1)?;

    let declared = svg_attr(tag, "width")
        .and_then(svg_px)
        .zip(svg_attr(tag, "height").and_then(svg_px));
    let (width, height) = declared.or_else(|| {
        // Without usable lengths the coordinate system is all there is
        let view_box = svg_attr(tag, "viewBox")?;
        let mut numbers = view_box.split_whitespace().filter_map(|n| n.parse::<f32>().ok());
        let _origin = (numbers.next()?, numbers.next()?);
        Some((numbers.next()?, numbers.next()?))
    })?;

    let width = width.round().clamp(1., SVG_MAX_EDGE as f32) as u32;
    let height = height.round().clamp(1., SVG_MAX_EDGE as f32) as u32;
    Some((width, height))
}

/// Finds `name="value"` on the tag, skipping a name that only ends the same way
fn svg_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(found) = tag[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        if !tag[..at].ends_with(char::is_whitespace) {
            continue;
        }
        let rest = tag.get(at + name.len()..)?.strip_prefix('=')?;
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let rest = &rest[quote.len_utf8()..];
        return Some(rest.get(..rest.find(quote)?)?.trim());
    }
    None
}

/// Lengths in any other unit are not a size this renderer can honour
fn svg_px(value: &str) -> Option<f32> {
    let value = value.trim();
    let value = value.strip_suffix("px").unwrap_or(value);
    let value: f32 = value.parse().ok()?;
    value.is_finite().then_some(value)
}

/// Rasterize an SVG so the rest of the pipeline only ever sees pixels
fn decode_svg(bytes: &Bytes) -> Option<ImageHandle> {
    let (width, height) = svg_size(bytes)?;
    let (width, height) = (width as i32, height as i32);

    let mut dom = svg::Dom::from_bytes(bytes, FontMgr::empty()).ok()?;
    dom.set_container_size((width, height));
    let mut root = dom.root();
    root.set_width(svg::Length::new(width as f32, svg::LengthUnit::PX));
    root.set_height(svg::Length::new(height as f32, svg::LengthUnit::PX));

    let mut surface = raster_n32_premul((width, height))?;
    dom.render(surface.canvas());
    Some(ImageHandle::new(surface.image_snapshot(), bytes.clone()))
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
    image_handle: ImageHandle,
    sampling_mode: SamplingMode,
    corner_radius: CornerRadius,
}

pub fn scaled_image(image_handle: ImageHandle) -> ScaledImage {
    ScaledImage {
        key: DiffKey::None,
        element: ScaledImageElement {
            accessibility: AccessibilityData::default(),
            layout: LayoutData::default(),
            image_handle,
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

        if self.image_handle != other.image_handle {
            diff.insert(DiffModifies::STYLE);

            if self.image_handle.image.dimensions() != other.image_handle.image.dimensions() {
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
        let image = &self.image_handle.image;
        let natural = Size2D::new(image.width() as f32, image.height() as f32);

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

        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        context.canvas.draw_image_rect_with_sampling_options(
            &self.image_handle.image,
            None,
            rect,
            self.sampling_mode.sampling_options(),
            &paint,
        );

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
                .child(scaled_image(handle(image.0, image.1)))
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
                    .child(scaled_image(handle(600, 200)))
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

    const BADGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="88" height="20" role="img" aria-label="build: passing"><rect width="88" height="20"/></svg>"#;

    #[test]
    fn reads_the_size_a_badge_declares() {
        assert_eq!(svg_size(BADGE.as_bytes()), Some((88, 20)));
        assert_eq!(
            svg_size(r#"<svg width="88px" height="20px"/>"#.as_bytes()),
            Some((88, 20))
        );
    }

    #[test]
    fn falls_back_to_the_drawing_coordinates() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 113 20"><rect/></svg>"#;
        assert_eq!(svg_size(svg.as_bytes()), Some((113, 20)));
    }

    #[test]
    fn a_width_that_only_ends_the_same_way_is_not_one() {
        let svg = r#"<svg max-width="900" height="20" viewBox="0 0 100 20"><rect/></svg>"#;
        assert_eq!(svg_size(svg.as_bytes()), Some((100, 20)));
    }

    #[test]
    fn payloads_that_are_not_svg_are_left_alone() {
        assert_eq!(svg_size(b"\x89PNG\r\n\x1a\n"), None);
        assert_eq!(svg_size(b"nothing that is markup at all"), None);
    }

    #[test]
    fn a_badge_rasterizes_where_the_pixel_decoder_gives_up() {
        let bytes = Bytes::from(BADGE.to_string());
        assert!(decode(&bytes).is_none());
        let handle = decode_svg(&bytes).expect("rasterized badge");
        let size = handle.image.dimensions();
        assert_eq!((size.width, size.height), (88, 20));
    }
}
