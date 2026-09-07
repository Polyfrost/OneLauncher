use std::any::Any;
use std::borrow::Cow;
use std::rc::Rc;

use freya::elements::image::ImageHandle;
use freya::engine::prelude::{ClipOp, Paint, SkRect};
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

            let holder = decode(&bytes)?;
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

        let available =
            (*context.area_size - context.torin_node.margin.into()).max(Size2D::zero());

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
    use freya::prelude::Bytes;
    use freya::engine::prelude::AlphaType;
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
}
