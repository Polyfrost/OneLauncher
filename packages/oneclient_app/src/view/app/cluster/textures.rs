use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use freya::prelude::*;
use oneclient_content::packages::ContentType;
use oneclient_core::clusters::Cluster;

use crate::hooks::{
    bundle_overrides_map, bundles_with_status_items, cluster_content_items, stale_hashes,
    use_bundle_overrides, use_bundles_with_status, use_cluster_content, use_folder_packs,
    use_package_updates, use_packs_folder_watch,
};
use crate::i18n::t;
use crate::layout::cluster_content;

use super::cluster_not_found;
use super::package_manager::{
    PackageManager, bundle_categories, bundle_packages, folder_pack_rows, use_content_meta,
};
use crate::hooks::use_cluster;

#[derive(PartialEq)]
pub struct ClusterTextures {
    pub cluster_id: i64,
}

impl Component for ClusterTextures {
    fn render(&self) -> impl IntoElement {
        let content = use_cluster_content(self.cluster_id, ContentType::ResourcePack);
        let bundles = use_bundles_with_status(self.cluster_id);
        let overrides = use_bundle_overrides(self.cluster_id);
        let updates = use_package_updates(self.cluster_id);
        let folder_packs = use_folder_packs(self.cluster_id, ContentType::ResourcePack);
        let bundle_items = bundles_with_status_items(&bundles);
        let content_items = cluster_content_items(&content);
        let meta = use_content_meta(&content_items, &bundle_items, ContentType::ResourcePack);

        let Some(cluster) = use_cluster(self.cluster_id) else {
            return cluster_not_found();
        };

        let all_categories = bundle_categories(&bundle_items);
        let mut items = bundle_packages(
            content_items,
            &bundle_items,
            &bundle_overrides_map(&overrides),
            &meta,
            &stale_hashes(&updates),
            ContentType::ResourcePack,
        );
        items.extend(folder_pack_rows(folder_packs));

        cluster_content()
            .child(PacksFolderWatch::new(&cluster, ContentType::ResourcePack))
            .child(
                PackageManager::new(
                    t("oneclient.cluster.textures.title"),
                    t("oneclient.cluster.package_manager.noun.resource_pack.other"),
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

#[derive(PartialEq)]
pub(super) struct PacksFolderWatch {
    game_folder: Option<PathBuf>,
    cluster_folder: Option<PathBuf>,
}

impl PacksFolderWatch {
    pub(super) fn new(cluster: &Cluster, content_type: ContentType) -> Self {
        let folder = content_type.folder_name();
        let cluster_folder = cluster
            .shares_content(content_type)
            .then(|| cluster.dir().ok())
            .flatten()
            .map(|dir| dir.join(folder));
        Self {
            game_folder: cluster
                .missing_shared_group()
                .is_none()
                .then(|| cluster.game_dir().ok())
                .flatten()
                .map(|dir| dir.join(folder)),
            cluster_folder,
        }
    }
}

impl Component for PacksFolderWatch {
    fn render(&self) -> impl IntoElement {
        use_packs_folder_watch(self.game_folder.clone(), self.cluster_folder.clone());
        rect()
    }

    fn render_key(&self) -> DiffKey {
        let mut hasher = DefaultHasher::new();
        (&self.game_folder, &self.cluster_folder).hash(&mut hasher);
        DiffKey::U64(hasher.finish())
    }
}
