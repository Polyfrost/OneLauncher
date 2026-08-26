//! An image element that scales the way a browser's `max-width: 100%` does.
//!
//! None of freya's four [`AspectRatio`] modes can express it, which is why this
//! element exists instead of a call into `freya::elements::image`:
//!
//! - `Fit` keeps the natural size, so a 1920px banner runs past the panel. This
//!   is what `freya-markdown` uses, and it is the bug we are here to fix.
//! - `Min` scales to fit the bounds, which blows an 88x31 badge up to the full
//!   panel width, and lets the *available height* pick the scale — measured in a
//!   300x60 panel a 600x200 banner comes out 180x60 instead of 300x100. In a
//!   column you scroll, height is not a real constraint and must not decide.
//! - `Max` crops, `None` stretches.
//!
//! Clamping the ratio at `1.` on the width axis alone is the whole fix.

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

/// Descriptions are read at panel width, so anything past this is memory spent
/// on pixels nobody sees. The launcher's image service downscales on fetch.
const MAX_EDGE: u32 = 1600;

/// An image from a markdown document, fetched through the launcher's own cache.
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

        // Decoding is not free, and a render happens for every unrelated state
        // change in the panel keep the handle until the bytes actually change
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
            // No spinner: descriptions are read top to bottom, and a loader that
            // resizes into an image shoves the text around while it is read
            None if self.alt.is_empty() => rect().into_element(),
            None => label()
                .text(self.alt.clone())
                .color(colors::fg_secondary())
                .into_element(),
        }
    }
}

/// Builder for [`ScaledImageElement`].
#[derive(Clone)]
pub struct ScaledImage {
    key: DiffKey,
    element: ScaledImageElement,
}

/// Scales a decoded image down to the width it is given, never past its natural
/// size, with the height following along.
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

            // A different image of the same size keeps the layout it had
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

        // `max-width: 100%`: shrink to the width on offer, never grow past the
        // natural size, and leave the height out of it entirely
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

    /// A solid opaque image of the given size, decoded already — no PNG fixture
    /// and no image loading in the way of a layout assertion.
    fn handle(width: u32, height: u32) -> ImageHandle {
        let pixels = Bytes::from(vec![200u8; (width * height * 4) as usize]);
        ImageHandle::from_rgba(width, height, pixels, AlphaType::Opaque).expect("rgba handle")
    }

    /// The measured size of the single image in a panel of the given size.
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

    /// Markdown images can sit inside a paragraph, where they are laid out as
    /// inline placeholders rather than ordinary children. The clamp has to hold
    /// there too, or a banner in a paragraph escapes the panel again.
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

    /// `AspectRatio::Min` gives 180x60 here, because the available height gets a
    /// vote. In a column you scroll, it must not.
    #[test]
    fn short_panel_does_not_shrink_the_width() {
        assert_eq!(measured((300., 60.), (600, 200)), Size2D::new(300., 100.));
    }
}
