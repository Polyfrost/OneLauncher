use freya::prelude::*;
use oneclient_content::packages::ContentType;

use crate::hooks::{
    bundle_overrides_map, bundles_with_status_items, cluster_content_items, modpack_sources_map,
    stale_hashes, use_bundle_overrides, use_bundles_with_status, use_cluster_content,
    use_modpack_sources, use_package_updates,
};
use crate::layout::cluster_content;

use super::cluster_not_found;
use super::package_manager::{
    PackageManager, bundle_categories, bundle_packages, use_content_meta,
};
use crate::hooks::use_cluster;

#[derive(PartialEq)]
pub struct ClusterMods {
    pub cluster_id: i64,
}

impl Component for ClusterMods {
    fn render(&self) -> impl IntoElement {
        let content = use_cluster_content(self.cluster_id, ContentType::Mod);
        let bundles = use_bundles_with_status(self.cluster_id);
        let overrides = use_bundle_overrides(self.cluster_id);
        let updates = use_package_updates(self.cluster_id);
        let sources = use_modpack_sources(self.cluster_id);
        let bundle_items = bundles_with_status_items(&bundles);
        let content_items = cluster_content_items(&content);
        let meta = use_content_meta(&content_items, &bundle_items, ContentType::Mod);

        let Some(_cluster) = use_cluster(self.cluster_id) else {
            return cluster_not_found();
        };

        let all_categories = bundle_categories(&bundle_items);
        let mut items = bundle_packages(
            content_items,
            &bundle_items,
            &bundle_overrides_map(&overrides),
            &meta,
            &stale_hashes(&updates),
            ContentType::Mod,
        );
        let sources = modpack_sources_map(&sources);
        for item in &mut items {
            item.modpack = item.hash.as_ref().and_then(|hash| sources.get(hash).cloned());
        }

        cluster_content()
            .child(
                PackageManager::new(
                    "Mods",
                    "mods",
                    "mod",
                    ContentType::Mod,
                    self.cluster_id,
                    items,
                    all_categories,
                )
                .into_element(),
            )
            .into_element()
    }
}
