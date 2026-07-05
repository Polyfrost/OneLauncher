use std::path::PathBuf;

use bytes::Bytes;
use freya::animation::{
    AnimNum, Ease, Function, OnChange, OnCreation, OnFinish, use_animation_with_dependencies,
};
use freya::elements::image::{AspectRatio, ImageCover, ImageHandle, image};
use freya::engine::prelude::{SkData, SkImage};
use freya::prelude::*;
use freya::query::QueryStateData;

use crate::hooks::use_local_image;
use crate::theme::colors;

#[derive(PartialEq)]
pub struct LocalImage {
    path: PathBuf,
    max_edge: u32,
    cover: bool,
    skeleton: bool,
}

impl LocalImage {
    pub fn new(path: PathBuf, max_edge: u32, cover: bool) -> Self {
        Self {
            path,
            max_edge,
            cover,
            skeleton: false,
        }
    }

    pub fn skeleton(mut self, skeleton: bool) -> Self {
        self.skeleton = skeleton;
        self
    }
}

impl Component for LocalImage {
    fn render(&self) -> impl IntoElement {
        let query = use_local_image(self.path.clone(), self.max_edge);

        let bytes: Option<Bytes> = match &*query.read().state() {
            QueryStateData::Settled { res: Ok(b), .. }
            | QueryStateData::Loading { res: Some(Ok(b)) } => Some(b.clone()),
            _ => None,
        };

        let mut cache = use_state(|| None::<(usize, ImageHandle)>);
        let holder = bytes.and_then(|bytes| {
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

        let aspect = if self.cover {
            AspectRatio::Max
        } else {
            AspectRatio::Min
        };

        let loaded = holder.is_some();
        let show_skeleton = self.skeleton && !loaded;

        let pulse = use_animation_with_dependencies(&show_skeleton, |conf, show| {
            conf.on_change(OnChange::Rerun);

            if *show {
                conf.on_creation(OnCreation::Run);
                conf.on_finish(OnFinish::reverse());
            }

            AnimNum::new(0.4, 0.9)
                .time(800)
                .ease(Ease::InOut)
                .function(Function::Sine)
        });

        let pulse_v = pulse.read().value();

        let fade = use_animation_with_dependencies(&loaded, |conf, loaded| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            let to = if *loaded { 1.0 } else { 0.0 };
            AnimNum::new(0., to)
                .time(260)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let fade_v = fade.read().value();

        let root = rect().width(Size::fill()).height(Size::fill());

        match holder {
            Some(holder) => root.child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .opacity(fade_v)
                    .child(
                        image(holder)
                            .width(Size::fill())
                            .height(Size::fill())
                            .aspect_ratio(aspect)
                            .image_cover(ImageCover::Center),
                    ),
            ),
            None if self.skeleton => root.child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .background(colors::component_bg())
                    .opacity(pulse_v),
            ),
            None => root,
        }
    }
}

fn decode(bytes: &Bytes) -> Option<ImageHandle> {
    let data = unsafe { SkData::new_bytes(bytes) };
    let img = SkImage::from_encoded(data)?;
    Some(ImageHandle::new(img, bytes.clone()))
}
