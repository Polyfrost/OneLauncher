mod index;
mod package;

pub use index::Browser;
pub use package::BrowserPackage;

use std::collections::HashMap;

use freya::prelude::*;
use oneclient_content::packages::ProviderId;
use oneclient_core::{BundleFileKind, BundleWithUpdateStatus, LinkedArtifactInfo};

use crate::components::{Icon, IconType};
use crate::hooks::{loaded_image, use_cached_image};
use crate::theme::colors;
use crate::ui::border_all_color;

const BANNER_BG: Color = Color::from_rgb(21, 28, 34);

/// How a browsed package is already present in the cluster.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum InstallSource {
    /// Installed on its own.
    Manual,
    /// Comes from one of the cluster's bundles.
    Bundled,
}

impl InstallSource {
    fn label(self) -> &'static str {
        match self {
            Self::Manual => "Installed",
            Self::Bundled => "Bundled",
        }
    }

    /// Bundled reads blue rather than green: the package is there, but the
    /// cluster's bundle put it there, not the user.
    fn color(self) -> Color {
        match self {
            Self::Manual => colors::success(),
            Self::Bundled => colors::code_info(),
        }
    }
}

/// A browsed package the cluster already has.
#[derive(Clone, PartialEq)]
pub(crate) struct Installed {
    pub source: InstallSource,
    /// Which version is in there, when the provider recorded one. Used to mark
    /// the matching row in a package's version list, never shown as text.
    pub version_id: Option<String>,
}

impl Installed {
    pub fn is_version(&self, version_id: &str) -> bool {
        self.version_id.as_deref() == Some(version_id)
    }
}

/// Indexes what the cluster already has by the project it came from, so a
/// search result can tell at a glance whether it's in there and how it got
/// there. Local files have no project to match a search result against and
/// are left out, as are a bundle's external (non-provider) files.
pub(crate) fn installed_map(
    content: Vec<LinkedArtifactInfo>,
    bundles: &[BundleWithUpdateStatus],
) -> HashMap<(ProviderId, String), Installed> {
    let mut map: HashMap<(ProviderId, String), Installed> = content
        .into_iter()
        .filter_map(|item| {
            Some((
                (item.provider?, item.project_id?),
                Installed {
                    source: InstallSource::Manual,
                    version_id: item.version_id,
                },
            ))
        })
        .collect();

    // Bundle membership wins: a bundle's files land in the cluster as ordinary
    // content, so they'd otherwise read as hand-installed. The version on disk
    // is the truth when there is one; the manifest's pin is the fallback.
    for bundle in bundles {
        for (file, _status) in &bundle.files {
            if let BundleFileKind::Managed {
                provider,
                project_id,
                version_id,
                ..
            } = &file.kind
            {
                map.entry((*provider, project_id.clone()))
                    .and_modify(|installed| installed.source = InstallSource::Bundled)
                    .or_insert_with(|| Installed {
                        source: InstallSource::Bundled,
                        version_id: Some(version_id.clone()),
                    });
            }
        }
    }

    map
}

pub(crate) fn installed_badge(installed: InstallSource, font_size: f32) -> impl IntoElement {
    badge(installed, font_size, installed.color().with_a(38), None)
}

/// The same badge sat on top of a card's artwork. The faint tint the in-card
/// badge uses has nothing to read against there, so this one brings its own
/// backdrop and outlines itself to stay legible over whatever the image is.
pub(crate) fn installed_badge_overlay(installed: InstallSource) -> impl IntoElement {
    badge(
        installed,
        10.,
        BANNER_BG.with_a(225),
        Some(installed.color().with_a(110)),
    )
}

fn badge(
    installed: InstallSource,
    font_size: f32,
    background: Color,
    border: Option<Color>,
) -> impl IntoElement {
    let color = installed.color();

    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.)
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(CornerRadius::new_all(999.))
        .background(background)
        .map(border, |el, border| el.border(border_all_color(1., border)))
        .child(
            Icon::new(IconType::CheckCircle)
                .size(font_size)
                .color(color),
        )
        .child(
            label()
                .text(installed.label())
                .font_size(font_size)
                .max_lines(1)
                .color(color),
        )
}

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
        let loaded = loaded_image(self.icon_url.as_deref(), &query);

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
        let loaded = loaded_image(self.icon_url.as_deref(), &query);

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
                                .image_cover(ImageCover::Center),
                        )
                        .layer(Layer::Relative(1)),
                )
                .child(
                    rect()
                        .position(Position::new_absolute().top(0.).left(0.))
                        .width(Size::fill())
                        .height(Size::fill())
                        .blur(12.)
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
                                .corner_radius(CornerRadius::new_all(10.)),
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
