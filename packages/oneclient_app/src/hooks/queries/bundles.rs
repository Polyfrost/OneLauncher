use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_common::domain::GameLoader;
use oneclient_core::clusters::{Cluster, ImportedModpack};
use oneclient_core::{
    BundleArchive, BundleFile, BundleUpdateCheckResult, BundleWithUpdateStatus, LauncherError,
    get_bundles_with_update_status, list_cluster_bundle_overrides,
};
use oneclient_db::models::{ClusterId, OverrideType};

use crate::launcher::off_ui;

#[derive(Clone, Debug)]
pub struct ClusterBundles {
    pub cluster: Cluster,
    pub archives: Vec<BundleArchive>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OnboardingBundlesQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OnboardingBundlesKeys;

impl QueryCapability for OnboardingBundlesQuery {
    type Ok = Vec<ClusterBundles>;
    type Err = LauncherError;
    type Keys = OnboardingBundlesKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        off_ui(async move {
            let clusters = state.clusters.list().await?;

            let mut out = Vec::with_capacity(clusters.len());
            for cluster in clusters.into_iter().filter(|cluster| !cluster.user_created) {
                let archives = state
                    .bundles
                    .archives_for(
                        &state.services.content(),
                        &cluster.mc_version,
                        cluster.mc_loader,
                    )
                    .await
                    .unwrap_or_default();
                out.push(ClusterBundles { cluster, archives });
            }
            Ok(out)
        })
        .await
    }
}

pub fn use_onboarding_bundles() -> UseQuery<OnboardingBundlesQuery> {
    use_query(Query::new(OnboardingBundlesKeys, OnboardingBundlesQuery))
}

pub fn onboarding_bundles_items(
    query: &UseQuery<OnboardingBundlesQuery>,
) -> Option<Vec<ClusterBundles>> {
    super::state::settled_or_loading(query)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundlesWithStatusQuery {
    pub cluster_id: ClusterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundlesWithStatusKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for BundlesWithStatusQuery {
    type Ok = Vec<BundleWithUpdateStatus>;
    type Err = LauncherError;
    type Keys = BundlesWithStatusKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let _ = keys;
        let state = crate::launcher::state()?;
        let cluster_id = self.cluster_id;
        Ok(off_ui(async move {
            get_bundles_with_update_status(
                cluster_id,
                state.bundles.as_ref(),
                &state.services.content(),
            )
            .await
        })
        .await?)
    }
}

pub fn bundles_with_status_items(
    query: &UseQuery<BundlesWithStatusQuery>,
) -> Vec<BundleWithUpdateStatus> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

pub fn use_bundles_with_status(cluster_id: ClusterId) -> UseQuery<BundlesWithStatusQuery> {
    use_query(Query::new(
        BundlesWithStatusKeys { cluster_id },
        BundlesWithStatusQuery { cluster_id },
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundleOverridesQuery {
    pub cluster_id: ClusterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundleOverridesKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for BundleOverridesQuery {
    type Ok = HashMap<(String, String), String>;
    type Err = LauncherError;
    type Keys = BundleOverridesKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let _ = keys;
        let state = crate::launcher::state()?;
        let rows =
            list_cluster_bundle_overrides(self.cluster_id, &state.services.content()).await?;
        Ok(rows
            .into_iter()
            .map(|(bundle, pkg, ty)| ((bundle, pkg), ty))
            .collect())
    }
}

pub fn use_bundle_overrides(cluster_id: ClusterId) -> UseQuery<BundleOverridesQuery> {
    use_query(Query::new(
        BundleOverridesKeys { cluster_id },
        BundleOverridesQuery { cluster_id },
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModpackSourcesQuery {
    pub cluster_id: ClusterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModpackSourcesKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for ModpackSourcesQuery {
    type Ok = HashMap<String, String>;
    type Err = LauncherError;
    type Keys = ModpackSourcesKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let _ = keys;
        let state = crate::launcher::state()?;
        let cluster_id = self.cluster_id;
        off_ui(async move {
            oneclient_core::clusters::modpack_file_sources(&state, cluster_id).await
        })
        .await
    }
}

pub fn use_modpack_sources(cluster_id: ClusterId) -> UseQuery<ModpackSourcesQuery> {
    use_query(Query::new(
        ModpackSourcesKeys { cluster_id },
        ModpackSourcesQuery { cluster_id },
    ))
}

pub fn modpack_sources_map(query: &UseQuery<ModpackSourcesQuery>) -> HashMap<String, String> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImportedModpacksQuery {
    pub cluster_id: ClusterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImportedModpacksKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for ImportedModpacksQuery {
    type Ok = Vec<ImportedModpack>;
    type Err = LauncherError;
    type Keys = ImportedModpacksKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let _ = keys;
        let state = crate::launcher::state()?;
        let cluster_id = self.cluster_id;
        off_ui(async move {
            oneclient_core::clusters::list_imported_modpacks(&state, cluster_id).await
        })
        .await
    }
}

pub fn use_imported_modpacks(cluster_id: ClusterId) -> UseQuery<ImportedModpacksQuery> {
    use_query(Query::new(
        ImportedModpacksKeys { cluster_id },
        ImportedModpacksQuery { cluster_id },
    ))
}

pub fn imported_modpacks_list(query: &UseQuery<ImportedModpacksQuery>) -> Vec<ImportedModpack> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

pub fn bundle_overrides_map(
    query: &UseQuery<BundleOverridesQuery>,
) -> HashMap<(String, String), String> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundleUpdatesQuery {
    pub cluster_id: ClusterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BundleUpdatesKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for BundleUpdatesQuery {
    type Ok = BundleUpdateCheckResult;
    type Err = LauncherError;
    type Keys = BundleUpdatesKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let _ = keys;
        let state = crate::launcher::state()?;
        let cluster_id = self.cluster_id;
        Ok(off_ui(async move {
            oneclient_core::check_bundle_updates(
                cluster_id,
                state.bundles.as_ref(),
                &state.services.content(),
            )
            .await
        })
        .await?)
    }
}

pub fn use_bundle_updates(cluster_id: ClusterId) -> UseQuery<BundleUpdatesQuery> {
    use_query(Query::new(
        BundleUpdatesKeys { cluster_id },
        BundleUpdatesQuery { cluster_id },
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AvailableBundlesQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AvailableBundlesKeys {
    pub mc_version: String,
    pub loader: GameLoader,
}

impl QueryCapability for AvailableBundlesQuery {
    type Ok = Arc<[BundleArchive]>;
    type Err = LauncherError;
    type Keys = AvailableBundlesKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        if keys.mc_version.is_empty() {
            return Ok(Arc::from([]));
        }
        let state = crate::launcher::state()?;
        Ok(state
            .bundles
            .archives_for(&state.services.content(), &keys.mc_version, keys.loader)
            .await
            .unwrap_or_default()
            .into())
    }
}

pub fn use_available_bundles(
    mc_version: String,
    loader: GameLoader,
) -> UseQuery<AvailableBundlesQuery> {
    use_query(Query::new(
        AvailableBundlesKeys { mc_version, loader },
        AvailableBundlesQuery,
    ))
}

pub fn available_bundles(query: &UseQuery<AvailableBundlesQuery>) -> Option<Arc<[BundleArchive]>> {
    super::state::settled_or_loading(query)
}

pub const UNCREATED_CLUSTER: i64 = 0;

pub fn pkg_key(cluster_id: i64, bundle_name: &str, package_id: &str) -> String {
    format!("{cluster_id}|{bundle_name}|{package_id}")
}

pub fn shared_mod_keys(
    cluster_id: i64,
    archives: &[BundleArchive],
    edited: &str,
    package_id: &str,
    selected: &HashSet<String>,
) -> Vec<String> {
    archives
        .iter()
        .filter(|archive| {
            archive.manifest.name == edited || bundle_taken(cluster_id, archive, selected)
        })
        .filter(|archive| {
            archive
                .manifest
                .files
                .iter()
                .any(|file| !file.hidden && file.kind.package_id() == package_id)
        })
        .map(|archive| pkg_key(cluster_id, &archive.manifest.name, package_id))
        .collect()
}

pub fn toggle_keys(selected: &mut HashSet<String>, keys: &[String]) {
    if keys.iter().any(|key| selected.contains(key)) {
        for key in keys {
            selected.remove(key);
        }
    } else {
        selected.extend(keys.iter().cloned());
    }
}

pub fn bundle_plan(
    cluster_id: i64,
    archives: &[BundleArchive],
    selected: &HashSet<String>,
) -> (Vec<(String, String, OverrideType)>, Vec<(String, bool)>) {
    let kept = kept_package_ids(cluster_id, archives, selected);
    let in_taken: HashSet<String> = archives
        .iter()
        .filter(|archive| bundle_taken(cluster_id, archive, selected))
        .flat_map(|archive| archive.manifest.files.iter().map(|file| file.kind.package_id()))
        .collect();
    let overrides = archives
        .iter()
        .flat_map(|archive| archive_overrides(cluster_id, archive, selected, &kept, &in_taken))
        .collect();
    let choices = archives
        .iter()
        .filter(|archive| !archive.manifest.enabled)
        .map(|archive| {
            let taken = bundle_taken(cluster_id, archive, selected);
            (archive.manifest.name.clone(), taken)
        })
        .collect();
    (overrides, choices)
}

/// True once anything visible in the bundle was taken including one opted-in extra that would otherwise lose its dependencies
pub fn bundle_taken(cluster_id: i64, archive: &BundleArchive, selected: &HashSet<String>) -> bool {
    archive.manifest.files.iter().any(|file| {
        !file.hidden
            && selected.contains(&pkg_key(
                cluster_id,
                &archive.manifest.name,
                &file.kind.package_id(),
            ))
    })
}

/// Bundles overlap deciding each archive alone recorded a removal for a package an accepted bundle was about to install
fn kept_package_ids(
    cluster_id: i64,
    archives: &[BundleArchive],
    selected: &HashSet<String>,
) -> HashSet<String> {
    let mut kept = HashSet::new();

    for archive in archives {
        let takes_hidden = bundle_taken(cluster_id, archive, selected);
        for file in &archive.manifest.files {
            let package_id = file.kind.package_id();
            let wanted = if file.hidden {
                takes_hidden
            } else {
                selected.contains(&pkg_key(cluster_id, &archive.manifest.name, &package_id))
            };
            if wanted {
                kept.insert(package_id);
            }
        }
    }

    kept
}

fn archive_overrides(
    cluster_id: i64,
    archive: &BundleArchive,
    selected: &HashSet<String>,
    kept_elsewhere: &HashSet<String>,
    in_taken: &HashSet<String>,
) -> Vec<(String, String, OverrideType)> {
    let bundle_name = &archive.manifest.name;
    let wants = |file: &BundleFile| {
        selected.contains(&pkg_key(cluster_id, bundle_name, &file.kind.package_id()))
    };

    // Hidden files follow the bundle kept if anything from it was taken dropped if not
    let takes_hidden = bundle_taken(cluster_id, archive, selected);

    let mut overrides = Vec::new();
    for file in &archive.manifest.files {
        let wanted = if file.hidden {
            takes_hidden
        } else {
            wants(file)
        };

        let override_type = match (wanted, file.enabled) {
            // Matches the manifest default nothing to record
            (true, true) | (false, false) => continue,
            // Opting into a mod the bundle ships turned off
            (true, false) => OverrideType::Enabled,
            // Only really declined if no bundle the user took ships the same package
            (false, true) => {
                if kept_elsewhere.contains(&file.kind.package_id()) {
                    continue;
                }
                if takes_hidden {
                    OverrideType::Disabled
                } else if in_taken.contains(&file.kind.package_id()) {
                    continue;
                } else {
                    OverrideType::Removed
                }
            }
        };

        overrides.push((bundle_name.clone(), file.kind.package_id(), override_type));
    }
    overrides
}
