use bytes::Bytes;
use freya::prelude::*;
use freya::query::QueryStateData;
use oneclient_core::clusters::Cluster;
use oneclient_core::images::DEFAULT_IMAGE_EDGE;
use oneclient_core::packages::domain::GameLoader;
use oneclient_core::parse_mc_version;

use crate::AppAssets;
use crate::hooks::{use_cached_image, use_version_metadata};
use crate::layout::HOME_BACKGROUND_ASSET;

#[derive(PartialEq, Clone)]
pub struct DynamicArt {
    major: Option<u32>,
    minor: Option<u32>,
    loader: Option<GameLoader>,
    max_edge: u32,
}

impl DynamicArt {
    pub fn for_version(major: u32, minor: Option<u32>, loader: Option<GameLoader>) -> Self {
        Self {
            major: Some(major),
            minor,
            loader,
            max_edge: DEFAULT_IMAGE_EDGE,
        }
    }

    pub fn for_major(major: u32) -> Self {
        Self {
            major: Some(major),
            minor: None,
            loader: None,
            max_edge: DEFAULT_IMAGE_EDGE,
        }
    }

    pub fn for_cluster(cluster: &Cluster) -> Self {
        let parsed = parse_mc_version(&cluster.mc_version);
        Self {
            major: parsed.as_ref().map(|p| p.major),
            minor: parsed.and_then(|p| p.minor),
            loader: Some(cluster.mc_loader),
            max_edge: DEFAULT_IMAGE_EDGE,
        }
    }

    pub fn fallback() -> Self {
        Self {
            major: None,
            minor: None,
            loader: None,
            max_edge: DEFAULT_IMAGE_EDGE,
        }
    }

    #[must_use]
    pub fn max_edge(mut self, max_edge: u32) -> Self {
        self.max_edge = max_edge;
        self
    }

    pub fn use_bytes(&self) -> (String, Bytes) {
        use_art_bytes(self.major, self.minor, self.loader, self.max_edge)
    }
}

pub fn use_art_bytes(
    major: Option<u32>,
    minor: Option<u32>,
    loader: Option<GameLoader>,
    max_edge: u32,
) -> (String, Bytes) {
    let fallback = use_memo(|| AppAssets::get_bytes(HOME_BACKGROUND_ASSET).unwrap_or_default());

    let art_url = use_version_metadata(major, minor, loader).and_then(|m| m.art_url);

    let image_query = use_cached_image(art_url.clone(), max_edge);

    let reader = image_query.read();
    let state = reader.state();
    match (&art_url, &*state) {
        (Some(url), QueryStateData::Settled { res: Ok(bytes), .. })
        | (
            Some(url),
            QueryStateData::Loading {
                res: Some(Ok(bytes)),
            },
        ) => (format!("{max_edge}|{url}"), bytes.clone()),
        _ => (
            format!("{max_edge}|{HOME_BACKGROUND_ASSET}"),
            fallback.read().clone(),
        ),
    }
}

impl Component for DynamicArt {
    fn render(&self) -> impl IntoElement {
        let (key, bytes) = self.use_bytes();

        ImageViewer::new((key, bytes))
            .width(Size::fill())
            .height(Size::fill())
            .aspect_ratio(AspectRatio::Max)
            .image_cover(ImageCover::Center)
            .into_element()
    }
}
