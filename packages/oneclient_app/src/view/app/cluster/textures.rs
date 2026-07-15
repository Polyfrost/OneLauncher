use freya::prelude::*;
use oneclient_core::packages::ContentType;

use crate::hooks::{
    bundle_overrides_map, bundles_with_status_items, cluster_content_items, use_bundle_overrides,
    use_bundles_with_status, use_cluster_content,
};
use crate::layout::cluster_content;

use super::package_manager::{
    PackageManager, bundle_categories, bundle_packages, use_content_meta,
};
use super::{cluster_not_found, load_cluster};

#[derive(PartialEq)]
pub struct ClusterTextures {
    pub cluster_id: i64,
}

impl Component for ClusterTextures {
    fn render(&self) -> impl IntoElement {
        let content = use_cluster_content(self.cluster_id, ContentType::ResourcePack);
        let bundles = use_bundles_with_status(self.cluster_id);
        let overrides = use_bundle_overrides(self.cluster_id);
        let bundle_items = bundles_with_status_items(&bundles);
        let content_items = cluster_content_items(&content);
        let meta = use_content_meta(&content_items, &bundle_items, ContentType::ResourcePack);

        let Some(_cluster) = load_cluster(self.cluster_id) else {
            return cluster_not_found();
        };

        let all_categories = bundle_categories(&bundle_items);
        let items = bundle_packages(
            content_items,
            &bundle_items,
            &bundle_overrides_map(&overrides),
            &meta,
            ContentType::ResourcePack,
        );

        cluster_content()
            .child(
                PackageManager::new(
                    "Textures",
                    "resource packs",
                    "texture",
                    ContentType::ResourcePack,
                    self.cluster_id,
                    items,
                    all_categories,
                )
                .into_element(),
            )
            .into_element()
    }
}
