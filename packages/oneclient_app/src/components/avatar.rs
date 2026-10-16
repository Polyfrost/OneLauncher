use bytes::Bytes;
use freya::elements::image::{ImageHandle, image};
use freya::engine::prelude::{Paint, SkData, SkImage, raster_n32_premul};
use freya::prelude::*;

use crate::hooks::use_player_skin;

const AVATAR_SIZE: f32 = 32.;
const FACE: f32 = 8.;
const HEAD_FACE: (f32, f32) = (8., 8.);
const HEAD_OVERLAY: (f32, f32) = (40., 8.);

#[derive(PartialEq, Clone)]
pub struct Avatar {
    uuid: String,
    size: f32,
    radius: f32,
    layout: LayoutData,
}

impl Avatar {
    pub fn new(uuid: impl Into<String>) -> Self {
        Self {
            uuid: uuid.into(),
            size: AVATAR_SIZE,
            radius: 8.,
            layout: LayoutData::default(),
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }
}

impl LayoutExt for Avatar {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl ContainerSizeExt for Avatar {}

impl Component for Avatar {
    fn render(&self) -> impl IntoElement {
        let (skin_bytes, _is_slim) = use_player_skin(self.uuid.clone());

        let scale_factor = *Platform::get().scale_factor.read() as f32;
        let texel = texel_px(self.size, scale_factor);
        let face = face_size(texel, scale_factor);

        let mut cache = use_state(|| None::<(usize, u32, ImageHandle)>);
        let src_ptr = skin_bytes.as_ptr() as usize;
        let cached = cache.read().clone();
        let head = match cached {
            Some((ptr, scale, holder)) if ptr == src_ptr && scale == texel => Some(holder),
            _ => {
                let holder = compose_head(&skin_bytes, texel);
                if let Some(holder) = &holder {
                    cache.set(Some((src_ptr, texel, holder.clone())));
                }
                holder
            }
        };

        let mut nudge = use_state(|| (0f32, 0f32));
        let inset = (self.size - face) / 2.;
        let (dx, dy) = *nudge.read();

        rect()
            .width(Size::px(self.size))
            .height(Size::px(self.size))
            .corner_radius(CornerRadius::from(self.radius))
            .on_sized(move |e: Event<SizedEventData>| {
                let snap = |at: f32| {
                    let physical = (at + inset) * scale_or_one(scale_factor);
                    (physical.round() - physical) / scale_or_one(scale_factor)
                };
                nudge.set_if_modified((snap(e.area.min_x()), snap(e.area.min_y())));
            })
            .maybe_child(head.map(|holder| {
                image(holder)
                    .position(Position::new_absolute().left(inset + dx).top(inset + dy))
                    .width(Size::px(face))
                    .height(Size::px(face))
                    .aspect_ratio(AspectRatio::None)
                    .sampling_mode(SamplingMode::Nearest)
                    .corner_radius(CornerRadius::from(self.radius))
            }))
    }
}

fn scale_or_one(scale_factor: f32) -> f32 {
    if scale_factor > 0. { scale_factor } else { 1. }
}

fn texel_px(size: f32, scale_factor: f32) -> u32 {
    (size * scale_or_one(scale_factor) / FACE).floor().max(1.) as u32
}

fn face_size(texel: u32, scale_factor: f32) -> f32 {
    texel as f32 * FACE / scale_or_one(scale_factor)
}

fn compose_head(skin_bytes: &Bytes, texel: u32) -> Option<ImageHandle> {
    let data = unsafe { SkData::new_bytes(skin_bytes) };
    let skin = SkImage::from_encoded(data)?;
    let skin = skin.make_raster_image(None, None).unwrap_or(skin);

    let side = (FACE as u32 * texel) as i32;
    let mut surface = raster_n32_premul((side, side))?;
    {
        let canvas = surface.canvas();
        let paint = Paint::default();
        canvas.scale((texel as f32, texel as f32));

        canvas.draw_image(&skin, (-HEAD_FACE.0, -HEAD_FACE.1), Some(&paint));
        canvas.draw_image(&skin, (-HEAD_OVERLAY.0, -HEAD_OVERLAY.1), Some(&paint));
    }

    let head = surface.image_snapshot();
    Some(ImageHandle::new(head, skin_bytes.clone()))
}

#[cfg(test)]
mod tests {
    use super::{face_size, texel_px};

    #[test]
    fn faces_snap_down_to_whole_texels() {
        assert_eq!(texel_px(28., 1.), 3);
        assert_eq!(face_size(3, 1.), 24.);
        assert_eq!(texel_px(32., 1.), 4);
        assert_eq!(texel_px(16., 1.), 2);
    }

    #[test]
    fn snapping_follows_the_display_scale() {
        assert_eq!(texel_px(28., 1.25), 4);
        assert_eq!(face_size(4, 1.25), 25.6);
        assert_eq!(texel_px(32., 1.5), 6);
    }
}
