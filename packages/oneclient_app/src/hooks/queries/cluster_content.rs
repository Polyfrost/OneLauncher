use freya::prelude::{spawn, use_hook};
use freya::query::{Query, QueryCapability, UseQuery, use_query};
use notify::{Event, EventKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oneclient_content::packages::release_migration::has_packages_to_migrate;
use oneclient_content::packages::{ContentType, PackageStore};
use oneclient_core::{LauncherError, LinkedArtifactInfo};
use oneclient_db::models::ClusterId;

use crate::launcher::off_ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterContentQuery {
    pub cluster_id: ClusterId,
    pub content_type: ContentType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterContentKeys {
    pub cluster_id: ClusterId,
    pub content_type: ContentType,
}

impl QueryCapability for ClusterContentQuery {
    type Ok = Vec<LinkedArtifactInfo>;
    type Err = LauncherError;
    type Keys = ClusterContentKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let all =
            PackageStore::list_linked_artifacts(self.cluster_id, &state.services.content()).await?;
        Ok(all
            .into_iter()
            .filter(|item| item.content_type == self.content_type)
            .collect())
    }
}

pub fn use_cluster_content(
    cluster_id: ClusterId,
    content_type: ContentType,
) -> UseQuery<ClusterContentQuery> {
    use_query(Query::new(
        ClusterContentKeys {
            cluster_id,
            content_type,
        },
        ClusterContentQuery {
            cluster_id,
            content_type,
        },
    ))
}

pub fn cluster_content_items(query: &UseQuery<ClusterContentQuery>) -> Vec<LinkedArtifactInfo> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MigratableRoutesQuery;

impl QueryCapability for MigratableRoutesQuery {
    type Ok = Arc<HashSet<(ClusterId, ClusterId)>>;
    type Err = LauncherError;
    type Keys = Vec<(ClusterId, ClusterId)>;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let routes = keys.clone();
        off_ui(async move {
            let content = state.services.content();
            let mut migratable = HashSet::new();
            for (target, source) in routes {
                if has_packages_to_migrate(source, target, &state.bundles, &content).await? {
                    migratable.insert((target, source));
                }
            }
            Ok(Arc::new(migratable))
        })
        .await
    }
}

pub fn use_migratable_routes(
    routes: Vec<(ClusterId, ClusterId)>,
) -> Arc<HashSet<(ClusterId, ClusterId)>> {
    let query = use_query(Query::new(routes, MigratableRoutesQuery));
    super::state::settled_or_loading(&query).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShadowedModsQuery;

impl QueryCapability for ShadowedModsQuery {
    type Ok = Arc<HashSet<String>>;
    type Err = LauncherError;
    type Keys = ClusterId;

    async fn run(&self, cluster_id: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let cluster_id = *cluster_id;
        Ok(off_ui(async move {
            Arc::new(oneclient_core::game::shadowed_bundle_mods(&state, cluster_id).await)
        })
        .await)
    }
}

pub fn use_shadowed_mods(cluster_id: ClusterId) -> Arc<HashSet<String>> {
    let query = use_query(Query::new(cluster_id, ShadowedModsQuery));
    super::state::settled_or_loading(&query).unwrap_or_default()
}

pub fn use_mods_folder_sync(cluster_id: ClusterId, folder: Option<PathBuf>) {
    let sync = move || {
        spawn(async move {
            let changed = off_ui(async move {
                let Ok(state) = crate::launcher::state() else {
                    return false;
                };
                oneclient_core::game::sync_cluster_mods(&state, cluster_id).await
            })
            .await;

            if changed {
                super::mutations::invalidate_cluster_queries().await;
            }
        });
    };

    use_hook(sync);
    super::folder_watch::use_folder_watch(folder, false, touches_jar, sync);
}

fn touches_jar(_root: &Path, event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }

    event.paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                let name = name.to_ascii_lowercase();
                name.ends_with(".jar") || name.ends_with(".disabled")
            })
    })
}
