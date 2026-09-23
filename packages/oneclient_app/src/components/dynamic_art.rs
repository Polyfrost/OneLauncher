use std::path::PathBuf;

use bytes::Bytes;
use freya::animation::{
    AnimNum, Ease, Function, OnChange, OnCreation, OnFinish, use_animation_with_dependencies,
};
use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_common::{VersionKey, parse_mc_version};
use oneclient_core::clusters::Cluster;
use oneclient_core::images::{DEFAULT_IMAGE_EDGE, PREVIEW_IMAGE_EDGE};

use crate::AppAssets;
use crate::hooks::{
    loaded_image, resolve_art_url, settled_or_loading, use_cached_image, use_local_image,
    use_picked_image, use_version_art, use_version_metadata,
};
use crate::layout::HOME_BACKGROUND_ASSET;
use crate::theme::colors;
use crate::ui::ImageFallbackExt;

pub const ART_PREVIEW_EDGE: u32 = PREVIEW_IMAGE_EDGE;

#[derive(PartialEq, Clone)]
pub struct DynamicArt {
    major: Option<u32>,
    key: Option<VersionKey>,
    loader: Option<GameLoader>,
    cover: Option<PathBuf>,
    cover_picked: bool,
    max_edge: u32,
    preview_edge: Option<u32>,
    url: Option<String>,
    skeleton: bool,
}

impl DynamicArt {
    pub fn for_version(major: u32, key: Option<VersionKey>, loader: Option<GameLoader>) -> Self {
        Self {
            major: Some(major),
            key,
            loader,
            cover: None,
            cover_picked: false,
            max_edge: DEFAULT_IMAGE_EDGE,
            preview_edge: None,
            url: None,
            skeleton: false,
        }
    }

    pub fn for_cluster(cluster: &Cluster) -> Self {
        let parsed = parse_mc_version(&cluster.mc_version);
        Self {
            major: parsed.as_ref().map(|p| p.major),
            key: parsed.and_then(|p| p.key()),
            loader: Some(cluster.mc_loader),
            cover: cluster.cover_file(),
            cover_picked: false,
            max_edge: DEFAULT_IMAGE_EDGE,
            preview_edge: None,
            url: None,
            skeleton: false,
        }
    }

    pub fn for_url(url: String) -> Self {
        Self {
            major: None,
            key: None,
            loader: None,
            cover: None,
            cover_picked: false,
            max_edge: DEFAULT_IMAGE_EDGE,
            preview_edge: None,
            url: Some(url),
            skeleton: false,
        }
    }

    pub fn fallback() -> Self {
        Self {
            major: None,
            key: None,
            loader: None,
            cover: None,
            cover_picked: false,
            max_edge: DEFAULT_IMAGE_EDGE,
            preview_edge: None,
            url: None,
            skeleton: false,
        }
    }

    #[must_use]
    pub fn cover(mut self, cover: Option<PathBuf>) -> Self {
        self.cover = cover;
        self
    }

    #[must_use]
    pub fn picked_cover(mut self, cover: Option<PathBuf>) -> Self {
        self.cover = cover;
        self.cover_picked = true;
        self
    }

    #[must_use]
    pub fn skeleton(mut self, skeleton: bool) -> Self {
        self.skeleton = skeleton;
        self
    }

    #[must_use]
    pub fn max_edge(mut self, max_edge: u32) -> Self {
        self.max_edge = max_edge;
        self
    }

    /// Stand in with a smaller cached variant until the full-size art downloads
    #[must_use]
    pub fn preview_edge(mut self, preview_edge: u32) -> Self {
        self.preview_edge = Some(preview_edge);
        self
    }

    pub fn use_bytes(&self) -> (String, Bytes) {
        use_art_bytes(self)
    }
}

pub fn use_art_bytes(art: &DynamicArt) -> (String, Bytes) {
    let max_edge = art.max_edge;
    let fallback = use_bundled_art(false);
    use_resolved_art(art).unwrap_or_else(|| {
        (
            format!("{max_edge}|{HOME_BACKGROUND_ASSET}"),
            fallback.read().clone(),
        )
    })
}

fn use_bundled_art(skip: bool) -> Memo<Bytes> {
    use_memo(move || {
        if skip {
            Bytes::new()
        } else {
            AppAssets::get_bytes(HOME_BACKGROUND_ASSET).unwrap_or_default()
        }
    })
}

fn use_resolved_art(art: &DynamicArt) -> Option<(String, Bytes)> {
    let DynamicArt {
        major,
        key,
        loader,
        cover,
        cover_picked,
        max_edge,
        preview_edge,
        url,
        skeleton: _,
    } = art.clone();

    let stored = cover.clone().filter(|_| !cover_picked).unwrap_or_default();
    let chosen = cover.clone().filter(|_| cover_picked).unwrap_or_default();
    let stored_query = use_local_image(stored, max_edge);
    let picked_query = use_picked_image(chosen, max_edge);

    let cover_bytes = cover.as_ref().and_then(|path| {
        if cover_picked {
            settled_or_loading(&picked_query)
        } else {
            settled_or_loading(&stored_query)
        }
        .filter(|bytes: &Bytes| !bytes.is_empty())
        .map(|bytes| (format!("{max_edge}|{}", path.display()), bytes))
    });

    let curated = use_version_metadata(major, key, loader);
    let candidates = use_version_art(major, key);
    let art_url = url.or_else(|| resolve_art_url(curated.as_ref(), &candidates));

    let image_query = use_cached_image(art_url.clone(), max_edge);

    let preview_edge = preview_edge.filter(|edge| *edge != max_edge);
    let preview_query = match preview_edge {
        Some(edge) => use_cached_image(art_url.clone(), edge),
        None => use_cached_image(None, 0),
    };

    let full = loaded_image(art_url.as_deref(), &image_query)
        .map(|(url, bytes)| (format!("{max_edge}|{url}"), bytes));
    let preview = || {
        let edge = preview_edge?;
        loaded_image(art_url.as_deref(), &preview_query)
            .map(|(url, bytes)| (format!("{edge}|{url}"), bytes))
    };

    cover_bytes.or(full).or_else(preview)
}

impl Component for DynamicArt {
    fn render(&self) -> impl IntoElement {
        let fallback = use_bundled_art(self.skeleton);
        let resolved = use_resolved_art(self);
        let show_skeleton = self.skeleton && resolved.is_none();

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

        if show_skeleton {
            return rect()
                .width(Size::fill())
                .height(Size::fill())
                .background(colors::component_bg())
                .opacity(pulse_v)
                .into_element();
        }

        let max_edge = self.max_edge;
        let (key, bytes) = resolved.unwrap_or_else(|| {
            (
                format!("{max_edge}|{HOME_BACKGROUND_ASSET}"),
                fallback.read().clone(),
            )
        });

        ImageViewer::new((key, bytes))
            .width(Size::fill())
            .height(Size::fill())
            .aspect_ratio(AspectRatio::Max)
            .image_cover(ImageCover::Center)
            .fallback(rect().width(Size::fill()).height(Size::fill()))
            .into_element()
    }
}
