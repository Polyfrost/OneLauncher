use bytes::Bytes;
use freya::elements::image::{ImageHandle, image};
use freya::engine::prelude::{Paint, SkData, SkImage, raster_n32_premul};
use freya::prelude::*;
use freya::query::QueryStateData;

use crate::AppAssets;
use crate::hooks::{use_cached_image, use_player_profile};

const AVATAR_SIZE: f32 = 32.;
const FACE: f32 = 8.;
const HEAD_FACE: (f32, f32) = (8., 8.);
const HEAD_OVERLAY: (f32, f32) = (40., 8.);

#[derive(PartialEq, Clone)]
pub struct Avatar {
    uuid: String,
    layout: LayoutData,
}

impl Avatar {
    pub fn new(uuid: impl Into<String>) -> Self {
        Self {
            uuid: uuid.into(),
            layout: LayoutData::default(),
        }
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
        let profile = use_player_profile(self.uuid.clone(), None::<String>);

        let skin_url = match &*profile.read().state() {
            QueryStateData::Settled {
                res: Ok(profile), ..
            } => profile.skin_url.clone(),
            _ => None,
        };

        let skin_query = use_cached_image(skin_url.clone(), 256);

        let steve = use_memo(|| AppAssets::get_bytes("steve.png").unwrap_or_default());

        let skin_bytes = {
            let reader = skin_query.read();
            match (&skin_url, &*reader.state()) {
                (Some(_), QueryStateData::Settled { res: Ok(bytes), .. })
                | (
                    Some(_),
                    QueryStateData::Loading {
                        res: Some(Ok(bytes)),
                    },
                ) => bytes.clone(),
                _ => steve.read().clone(),
            }
        };

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

        rect()
            .width(Size::px(AVATAR_SIZE))
            .height(Size::px(AVATAR_SIZE))
            .corner_radius(CornerRadius::from(8.))
            .center()
            .maybe_child(head.map(|holder| {
                image(holder)
                    .width(Size::fill())
                    .height(Size::fill())
                    .aspect_ratio(AspectRatio::None)
                    .sampling_mode(SamplingMode::Nearest)
                    .corner_radius(CornerRadius::from(8.))
            }))
    }
}

fn compose_head(skin_bytes: &Bytes) -> Option<ImageHandle> {
    let data = unsafe { SkData::new_bytes(skin_bytes) };
    let skin = SkImage::from_encoded(data)?;
    let skin = skin.make_raster_image(None, None).unwrap_or(skin);

    let mut surface = raster_n32_premul((FACE as i32, FACE as i32))?;
    {
        let canvas = surface.canvas();
        let paint = Paint::default();

        canvas.draw_image(&skin, (-HEAD_FACE.0, -HEAD_FACE.1), Some(&paint));
        canvas.draw_image(&skin, (-HEAD_OVERLAY.0, -HEAD_OVERLAY.1), Some(&paint));
    }

    let head = surface.image_snapshot();
    Some(ImageHandle::new(head, skin_bytes.clone()))
}
