use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use freya::prelude::*;
use oneclient_content::packages::ContentType;

use crate::hooks::{
    bundle_overrides_map, bundles_with_status_items, cluster_content_items, stale_hashes,
    use_bisect, use_bundle_overrides, use_bundles_with_status, use_cluster_content,
    use_mods_folder_sync, use_package_updates, use_shadowed_mods,
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
        let shadowed = use_shadowed_mods(self.cluster_id);
        let bisect = use_bisect(self.cluster_id);
        let bundle_items = bundles_with_status_items(&bundles);
        let content_items = cluster_content_items(&content);
        let meta = use_content_meta(&content_items, &bundle_items, ContentType::Mod);

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
            ContentType::Mod,
        );
        for item in &mut items {
            item.locked = bisect.is_some();
            let Some(hash) = &item.hash else {
                continue;
            };
            item.shadowed = shadowed.shadowed.contains(hash);
            item.outranked = shadowed.outranked.contains(hash);
            item.bisect_role = bisect.as_ref().and_then(|status| status.role_of(hash));
        }

        cluster_content()
            .child(ModsFolderSync {
                cluster_id: self.cluster_id,
                folder: oneclient_common::paths::cluster_mods_dir(&cluster.folder_name).ok(),
            })
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
                .bisect(bisect.is_some())
                .into_element(),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct ModsFolderSync {
    cluster_id: i64,
    folder: Option<PathBuf>,
}

impl Component for ModsFolderSync {
    fn render(&self) -> impl IntoElement {
        use_mods_folder_sync(self.cluster_id, self.folder.clone());
        rect()
    }

    fn render_key(&self) -> DiffKey {
        let mut hasher = DefaultHasher::new();
        (self.cluster_id, &self.folder).hash(&mut hasher);
        DiffKey::U64(hasher.finish())
    }
}
