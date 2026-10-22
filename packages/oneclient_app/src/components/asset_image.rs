use freya::elements::image::{AspectRatio, ImageCover, ImageHandle, image};
use freya::prelude::*;

use super::local_image::decode;
use crate::AppAssets;

#[derive(PartialEq)]
pub struct AssetImage {
    path: &'static str,
    size: f32,
}

impl AssetImage {
    pub fn new(path: &'static str, size: f32) -> Self {
        Self { path, size }
    }
}

impl Component for AssetImage {
    fn render(&self) -> impl IntoElement {
        let path = self.path;
        let size = self.size;

        let handle: Option<ImageHandle> =
            use_hook(move || AppAssets::get_bytes(path).as_ref().and_then(decode));

        rect()
            .key(path)
            .width(Size::px(size))
            .height(Size::px(size))
            .center()
            .maybe_child(handle.map(|handle| {
                image(handle)
                    .width(Size::fill())
                    .height(Size::fill())
                    .aspect_ratio(AspectRatio::Min)
                    .image_cover(ImageCover::Center)
                    .into_element()
            }))
    }
}
