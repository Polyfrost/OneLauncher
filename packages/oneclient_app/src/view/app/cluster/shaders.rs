use freya::prelude::*;
use oneclient_core::packages::{ContentType, ProviderId};

use crate::hooks::{
    bundle_overrides_map, bundles_with_status_items, cluster_content_items, package_meta_batch,
    use_bundle_overrides, use_bundles_with_status, use_cluster_content, use_package_meta_batch,
};
use crate::layout::cluster_content;

use super::package_manager::{
    PackageManager, bundle_categories, bundle_packages, managed_project_ids,
};
use super::{cluster_not_found, load_cluster};

#[derive(PartialEq)]
pub struct ClusterShaders {
    pub cluster_id: i64,
}

impl Component for ClusterShaders {
    fn render(&self) -> impl IntoElement {
        let content = use_cluster_content(self.cluster_id, ContentType::Shader);
        let bundles = use_bundles_with_status(self.cluster_id);
        let overrides = use_bundle_overrides(self.cluster_id);
        let bundle_items = bundles_with_status_items(&bundles);
        let project_ids = managed_project_ids(&bundle_items, ContentType::Shader);
        let meta = use_package_meta_batch(ProviderId::Modrinth, project_ids);

        let Some(_cluster) = load_cluster(self.cluster_id) else {
            return cluster_not_found();
        };

        let all_categories = bundle_categories(&bundle_items);
        let items = bundle_packages(
            cluster_content_items(&content),
            &bundle_items,
            &bundle_overrides_map(&overrides),
            &package_meta_batch(&meta),
            ContentType::Shader,
        );

        cluster_content()
            .child(
                PackageManager::new(
                    "Shaders",
                    "shaders",
                    "shader",
                    ContentType::Shader,
                    self.cluster_id,
                    items,
                    all_categories,
                )
                .into_element(),
            )
            .into_element()
    }
}
