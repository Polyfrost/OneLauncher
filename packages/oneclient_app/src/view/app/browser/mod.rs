mod index;
mod package;

pub use index::Browser;
pub use package::BrowserPackage;

use freya::prelude::*;
use freya::query::QueryStateData;

use crate::components::{Icon, IconType};
use crate::hooks::use_cached_image;
use crate::theme::colors;

const BANNER_BG: Color = Color::from_rgb(21, 28, 34);

#[derive(PartialEq)]
pub(crate) struct Thumbnail {
    icon_url: Option<String>,
    size: f32,
    radius: f32,
    key: DiffKey,
}

impl Thumbnail {
    pub fn new(icon_url: Option<String>, size: f32) -> Self {
        Self {
            icon_url,
            size,
            radius: 10.,
            key: DiffKey::None,
        }
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }
}

impl KeyExt for Thumbnail {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for Thumbnail {
    fn render(&self) -> impl IntoElement {
        let size = self.size;
        let radius = self.radius;
        let query = use_cached_image(self.icon_url.clone(), 256);
        let reader = query.read();
        let loaded = match (&self.icon_url, &*reader.state()) {
            (Some(url), QueryStateData::Settled { res: Ok(bytes), .. })
            | (
                Some(url),
                QueryStateData::Loading {
                    res: Some(Ok(bytes)),
                },
            ) => Some((url.clone(), bytes.clone())),
            _ => None,
        };

        match loaded {
            Some((url, bytes)) => ImageViewer::new((url, bytes))
                .width(Size::px(size))
                .height(Size::px(size))
                .aspect_ratio(AspectRatio::Min)
                .corner_radius(CornerRadius::new_all(radius))
                .into_element(),
            None => rect()
                .center()
                .width(Size::px(size))
                .height(Size::px(size))
                .corner_radius(CornerRadius::new_all(radius))
                .background(colors::component_bg())
                .child(
                    Icon::new(IconType::DotsGrid)
                        .size(size * 0.4)
                        .color(colors::fg_secondary()),
                )
                .into_element(),
        }
    }
}

#[derive(PartialEq)]
pub(crate) struct PackageBanner {
    icon_url: Option<String>,
    height: f32,
    key: DiffKey,
}

impl PackageBanner {
    pub fn new(icon_url: Option<String>, height: f32) -> Self {
        Self {
            icon_url,
            height,
            key: DiffKey::None,
        }
    }
}

impl KeyExt for PackageBanner {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for PackageBanner {
    fn render(&self) -> impl IntoElement {
        let h = self.height;
        let icon = h * 0.62;
        let query = use_cached_image(self.icon_url.clone(), 512);
        let reader = query.read();
        let loaded = match (&self.icon_url, &*reader.state()) {
            (Some(url), QueryStateData::Settled { res: Ok(bytes), .. })
            | (
                Some(url),
                QueryStateData::Loading {
                    res: Some(Ok(bytes)),
                },
            ) => Some((url.clone(), bytes.clone())),
            _ => None,
        };

        let banner = rect()
            .width(Size::fill())
            .height(Size::px(h))
            .center()
            .overflow(Overflow::Clip)
            .background(BANNER_BG);

        match loaded {
            Some((url, bytes)) => banner
                .child(
                    rect()
                        .position(Position::new_absolute().top(0.).left(0.))
                        .width(Size::fill())
                        .height(Size::fill())
                        .overflow(Overflow::Clip)
                        .child(
                            ImageViewer::new((url.clone(), bytes.clone()))
                                .width(Size::fill())
                                .height(Size::fill())
                                .aspect_ratio(AspectRatio::Max)
                                .blur(12.)
                                .image_cover(ImageCover::Center),
                        )
                        .layer(Layer::Relative(1)),
                )
                .child(
                    rect()
                        .position(Position::new_absolute().top(0.).left(0.))
                        .width(Size::fill())
                        .height(Size::fill())
                        .background(BANNER_BG.with_a(120))
                        .overflow(Overflow::Clip)
                        .layer(Layer::Relative(3)),
                )
                .child(
                    rect()
                        .width(Size::px(icon))
                        .height(Size::px(icon))
                        .child(
                            ImageViewer::new((url, bytes))
                                .width(Size::px(icon))
                                .height(Size::px(icon))
                                .aspect_ratio(AspectRatio::Min)
                                .corner_radius(CornerRadius::new_all(10.))
                        )
                        .layer(Layer::Relative(5)),
                ),
            None => banner.child(
                rect()
                    .center()
                    .width(Size::px(icon))
                    .height(Size::px(icon))
                    .corner_radius(CornerRadius::new_all(10.))
                    .background(colors::component_bg())
                    .child(
                        Icon::new(IconType::DotsGrid)
                            .size(icon * 0.45)
                            .color(colors::fg_secondary()),
                    ),
            ),
        }
    }
}
