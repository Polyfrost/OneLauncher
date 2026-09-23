use std::path::PathBuf;

use bytes::Bytes;
use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_common::{VersionKey, parse_mc_version};
use oneclient_core::clusters::Cluster;
use oneclient_core::images::DEFAULT_IMAGE_EDGE;

use crate::AppAssets;
use crate::hooks::{
    loaded_image, settled_or_loading, use_cached_image, use_local_image, use_picked_image,
    use_version_metadata,
};
use crate::layout::HOME_BACKGROUND_ASSET;
use crate::ui::ImageFallbackExt;

/// The size cluster cards request so it is usually already cached when anything else wants it
pub const ART_PREVIEW_EDGE: u32 = 512;

#[derive(PartialEq, Clone)]
pub struct DynamicArt {
    major: Option<u32>,
    key: Option<VersionKey>,
    loader: Option<GameLoader>,
    cover: Option<PathBuf>,
    cover_picked: bool,
    max_edge: u32,
    preview_edge: Option<u32>,
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
        use_art_bytes(
            self.major,
            self.key,
            self.loader,
            self.cover.clone(),
            self.cover_picked,
            self.max_edge,
            self.preview_edge,
        )
    }
}

pub fn use_art_bytes(
    major: Option<u32>,
    key: Option<VersionKey>,
    loader: Option<GameLoader>,
    cover: Option<PathBuf>,
    cover_picked: bool,
    max_edge: u32,
    preview_edge: Option<u32>,
) -> (String, Bytes) {
    let fallback = use_memo(|| AppAssets::get_bytes(HOME_BACKGROUND_ASSET).unwrap_or_default());

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

    let art_url = use_version_metadata(major, key, loader).and_then(|m| m.art_url);

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

    cover_bytes.or(full).or_else(preview).unwrap_or_else(|| {
        (
            format!("{max_edge}|{HOME_BACKGROUND_ASSET}"),
            fallback.read().clone(),
        )
    })
}

impl Component for DynamicArt {
    fn render(&self) -> impl IntoElement {
        let (key, bytes) = self.use_bytes();

        ImageViewer::new((key, bytes))
            .width(Size::fill())
            .height(Size::fill())
            .aspect_ratio(AspectRatio::Max)
            .image_cover(ImageCover::Center)
            .fallback(rect().width(Size::fill()).height(Size::fill()))
            .into_element()
    }
}
