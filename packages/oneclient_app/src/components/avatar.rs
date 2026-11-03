use bytes::Bytes;
use freya::elements::image::{ImageHandle, image};
use freya::engine::prelude::{Paint, SkData, SkImage, raster_n32_premul};
use freya::prelude::*;

use crate::hooks::use_player_skin;

const AVATAR_SIZE: f32 = 32.;
const FACE: f32 = 8.;
const UPSCALE: f32 = 16.;
const HEAD_FACE: (f32, f32) = (8., 8.);
const HEAD_OVERLAY: (f32, f32) = (40., 8.);

#[derive(PartialEq, Clone)]
pub struct Avatar {
    uuid: String,
    size: f32,
    layout: LayoutData,
}

impl Avatar {
    pub fn new(uuid: impl Into<String>) -> Self {
        Self {
            uuid: uuid.into(),
            size: AVATAR_SIZE,
            layout: LayoutData::default(),
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
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

        let mut cache = use_state(|| None::<(usize, ImageHandle)>);
        let src_ptr = skin_bytes.as_ptr() as usize;
        let cached = cache.read().clone();
        let head = match cached {
            Some((ptr, holder)) if ptr == src_ptr => Some(holder),
            _ => {
                let holder = compose_head(&skin_bytes);
                if let Some(holder) = &holder {
                    cache.set(Some((src_ptr, holder.clone())));
                }
                holder
            }
        };

        let radius = self.size / 4.;

        rect()
            .width(Size::px(self.size))
            .height(Size::px(self.size))
            .corner_radius(CornerRadius::from(radius))
            .center()
            .maybe_child(head.map(|holder| {
                image(holder)
                    .width(Size::fill())
                    .height(Size::fill())
                    .aspect_ratio(AspectRatio::None)
                    .sampling_mode(SamplingMode::Trilinear)
                    .corner_radius(CornerRadius::from(radius))
            }))
    }
}

fn compose_head(skin_bytes: &Bytes) -> Option<ImageHandle> {
    let data = unsafe { SkData::new_bytes(skin_bytes) };
    let skin = SkImage::from_encoded(data)?;
    let skin = skin.make_raster_image(None, None).unwrap_or(skin);

    let edge = (FACE * UPSCALE) as i32;
    let mut surface = raster_n32_premul((edge, edge))?;
    {
        let canvas = surface.canvas();
        canvas.scale((UPSCALE, UPSCALE));
        let paint = Paint::default();

        canvas.draw_image(&skin, (-HEAD_FACE.0, -HEAD_FACE.1), Some(&paint));
        canvas.draw_image(&skin, (-HEAD_OVERLAY.0, -HEAD_OVERLAY.1), Some(&paint));
    }

    let head = surface.image_snapshot();
    Some(ImageHandle::new(head, skin_bytes.clone()))
}
