use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster as cluster_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::ClusterPatch;
use oneclient_db::models::{BundleTrackedArtifactRow, ClusterBundleOverrideRow, OverrideType};
use tokio::sync::Mutex as AsyncMutex;
use tracing::instrument;

use crate::bundles::install::{install_package_from_bundle, remove_artifact_from_cluster};
use crate::bundles::manager::BundlesManager;
use crate::bundles::overrides;
use crate::bundles::types::{
    ApplyBundleUpdatesResult, BundleArchive, BundleFileKind, BundlePackageAddition,
    BundlePackageRemoval, BundlePackageUpdate, BundleUpdateCheckResult, BundleWithUpdateStatus,
    FileUpdateStatus, external_bundle_key, managed_bundle_key,
};
use crate::packages::domain::{GameLoader, ProviderId};
use crate::packages::store::PackageStore;
use crate::packages::types::LinkedArtifactInfo;
use crate::state::LauncherServices;
use crate::{LauncherError, LauncherResult};

static CLUSTER_UPDATE_LOCKS: OnceLock<Mutex<HashMap<i64, Arc<AsyncMutex<()>>>>> = OnceLock::new();

fn cluster_lock(cluster_id: i64) -> Arc<AsyncMutex<()>> {
    let locks = CLUSTER_UPDATE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = locks.lock().unwrap();
    map.entry(cluster_id)
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

pub async fn check_bundle_updates(
    cluster_id: i64,
    bundles: &BundlesManager,
    services: &LauncherServices,
) -> LauncherResult<BundleUpdateCheckResult> {
    let overrides = bundle_dao::list_overrides(&services.db, cluster_id).await?;
    check_bundle_updates_inner(cluster_id, bundles, services, &overrides).await
}

#[instrument(skip(bundles, services, overrides))]
async fn check_bundle_updates_inner(
    cluster_id: i64,
    bundles: &BundlesManager,
    services: &LauncherServices,
    overrides: &[ClusterBundleOverrideRow],
) -> LauncherResult<BundleUpdateCheckResult> {
    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).ok_or_else(|| {
        LauncherError::InvalidSettingsProfile {
            reason: format!("unknown loader {}", cluster.mc_loader),
        }
    })?;

    let bundle_packages = bundle_dao::list_bundle_tracked(&services.db, cluster_id).await?;
    let all_linked = PackageStore::list_linked_artifacts(cluster_id, services).await?;
    let all_linked_by_hash: HashMap<String, &LinkedArtifactInfo> =
        all_linked.iter().map(|a| (a.hash.clone(), a)).collect();

    let (all_installed_managed_keys, all_installed_external_hashes) =
        installed_bundle_keys(services, &all_linked).await?;

    let archives = bundles
        .archives_for(services, &cluster.mc_version, loader)
        .await?;

    let tracked_bundle_names: HashSet<String> = bundle_packages
        .iter()
        .filter_map(|bp| bp.bundle_name.clone())
        .collect();
    let loaded_bundle_names: HashSet<String> =
        archives.iter().map(|a| a.manifest.name.clone()).collect();
    let unavailable_tracked: HashSet<String> = tracked_bundle_names
        .into_iter()
        .filter(|name| !loaded_bundle_names.contains(name))
        .collect();

    let mut bundle_versions: HashMap<
        String,
        HashMap<String, (String, crate::bundles::types::BundleFile)>,
    > = HashMap::new();
    let mut hidden_dependency_keys_by_bundle: HashMap<String, HashSet<String>> = HashMap::new();
    let mut hidden_explicit_keys_by_bundle: HashMap<String, HashSet<String>> = HashMap::new();

    for archive in &archives {
        let mut files_map = HashMap::new();
        let mut hidden_dependency_keys = HashSet::new();
        let mut hidden_explicit_keys = HashSet::new();

        for file in &archive.manifest.files {
            if !file.enabled {
                continue;
            }
            match &file.kind {
                BundleFileKind::Managed {
                    provider,
                    project_id,
                    version_id,
                    ..
                } => {
                    let key = managed_bundle_key(*provider, project_id);
                    files_map.insert(key.clone(), (version_id.clone(), file.clone()));
                    if file.hidden {
                        hidden_explicit_keys.insert(key);
                    }
                }
                BundleFileKind::External(ext) => {
                    let key = external_bundle_key(&ext.sha1);
                    files_map.insert(key.clone(), (ext.sha1.clone(), file.clone()));
                    if file.hidden {
                        hidden_explicit_keys.insert(key);
                    }
                }
            }
        }

        for visible_key in files_map.keys() {
            hidden_dependency_keys.remove(visible_key);
        }

        hidden_dependency_keys_by_bundle
            .insert(archive.manifest.name.clone(), hidden_dependency_keys);
        hidden_explicit_keys_by_bundle.insert(archive.manifest.name.clone(), hidden_explicit_keys);
        bundle_versions.insert(archive.manifest.name.clone(), files_map);
    }

    let mut subscribed_bundles: HashSet<String> = bundle_packages
        .iter()
        .filter_map(|bp| bp.bundle_name.clone())
        .collect();

    let mut candidate_keys_by_bundle = HashMap::new();
    for archive in &archives {
        let Some(files_map) = bundle_versions.get(&archive.manifest.name) else {
            continue;
        };
        let hidden_keys = hidden_dependency_keys_by_bundle
            .get(&archive.manifest.name)
            .cloned()
            .unwrap_or_default();
        let hidden_explicit = hidden_explicit_keys_by_bundle
            .get(&archive.manifest.name)
            .cloned()
            .unwrap_or_default();
        candidate_keys_by_bundle.insert(
            archive.manifest.name.clone(),
            collect_inferable_managed_keys(files_map, &hidden_keys, &hidden_explicit),
        );
    }

    for inferred in infer_bundle_names_from_unique_installed_keys(
        &candidate_keys_by_bundle,
        &all_installed_managed_keys,
    ) {
        subscribed_bundles.insert(inferred);
    }

    let subscribed_bundle_names: Vec<&str> = archives
        .iter()
        .filter(|a| subscribed_bundles.contains(&a.manifest.name))
        .map(|a| a.manifest.name.as_str())
        .collect();

    let overrides_map: HashMap<(String, String), OverrideType> = overrides
        .iter()
        .filter_map(|o| {
            OverrideType::parse(&o.override_type)
                .map(|t| ((o.bundle_name.clone(), o.package_id.clone()), t))
        })
        .collect();

    let mut updates_available = Vec::new();
    let mut removals_available = Vec::new();

    for bundle_pkg in &bundle_packages {
        let Some(pkg_id) = &bundle_pkg.package_id else {
            continue;
        };
        let Some(installed_version_id) = &bundle_pkg.bundle_version_id else {
            continue;
        };

        let installed_key = bundle_package_key(bundle_pkg, &all_linked_by_hash, pkg_id);

        let Some(bundle_name) = &bundle_pkg.bundle_name else {
            continue;
        };
        if unavailable_tracked.contains(bundle_name) {
            continue;
        }

        let mut matched_target = bundle_versions.get(bundle_name).and_then(|files_map| {
            files_map
                .get(&installed_key)
                .map(|(new_version_id, new_file)| {
                    (
                        bundle_name.clone(),
                        new_version_id.clone(),
                        new_file.clone(),
                    )
                })
        });

        if matched_target.is_none() {
            for candidate in &subscribed_bundle_names {
                if *candidate == bundle_name.as_str() {
                    continue;
                }
                if let Some((new_version_id, new_file)) = bundle_versions
                    .get(*candidate)
                    .and_then(|m| m.get(&installed_key))
                {
                    matched_target = Some((
                        (*candidate).to_string(),
                        new_version_id.clone(),
                        new_file.clone(),
                    ));
                    break;
                }
            }
        }

        if let Some((resolved_bundle_name, new_version_id, new_file)) = matched_target {
            if installed_version_id != &new_version_id {
                updates_available.push(BundlePackageUpdate {
                    cluster_id,
                    installed_hash: bundle_pkg.hash.clone(),
                    installed_version_id: installed_version_id.clone(),
                    bundle_name: resolved_bundle_name,
                    new_version_id,
                    new_file,
                });
            }
        } else {
            removals_available.push(BundlePackageRemoval {
                cluster_id,
                hash: bundle_pkg.hash.clone(),
                package_id: pkg_id.clone(),
                bundle_name: bundle_name.clone(),
            });
        }
    }

    updates_available.retain(|u| {
        let file_id = u.new_file.kind.package_id();
        !matches!(
            overrides_map.get(&(u.bundle_name.clone(), file_id)),
            Some(OverrideType::Removed)
        )
    });

    let mut additions_available = Vec::new();
    let mut planned_addition_keys = all_installed_managed_keys.clone();
    for hash in all_installed_external_hashes {
        planned_addition_keys.insert(external_bundle_key(&hash));
    }

    for archive in &archives {
        if !subscribed_bundles.contains(&archive.manifest.name) {
            continue;
        }

        for file in &archive.manifest.files {
            if !file.enabled {
                continue;
            }

            let file_key = match &file.kind {
                BundleFileKind::Managed {
                    provider,
                    project_id,
                    ..
                } => managed_bundle_key(*provider, project_id),
                BundleFileKind::External(ext) => external_bundle_key(&ext.sha1),
            };
            if planned_addition_keys.contains(&file_key) {
                continue;
            }

            let file_id = file.kind.package_id();
            if matches!(
                overrides_map.get(&(archive.manifest.name.clone(), file_id.clone())),
                Some(OverrideType::Removed)
            ) {
                continue;
            }

            additions_available.push(BundlePackageAddition {
                cluster_id,
                bundle_name: archive.manifest.name.clone(),
                new_file: file.clone(),
            });
            planned_addition_keys.insert(file_key);
        }
    }

    Ok(BundleUpdateCheckResult {
        cluster_id,
        updates_available,
        removals_available,
        additions_available,
    })
}

pub async fn apply_bundle_updates(
    cluster_id: i64,
    bundles: &BundlesManager,
    services: &LauncherServices,
) -> LauncherResult<ApplyBundleUpdatesResult> {
    let lock = cluster_lock(cluster_id);
    let _guard = lock.lock().await;
    let overrides = bundle_dao::list_overrides(&services.db, cluster_id).await?;
    let check = check_bundle_updates_inner(cluster_id, bundles, services, &overrides).await?;

    let mut result = ApplyBundleUpdatesResult::default();

    for removal in check.removals_available {
        match remove_artifact_from_cluster(cluster_id, &removal.hash, false, services).await {
            Ok(()) => result.removals_applied.push(removal),
            Err(err) => result
                .removals_failed
                .push(format!("{}: {err:#}", removal.package_id)),
        }
    }

    let session = (!check.updates_available.is_empty() || !check.additions_available.is_empty())
        .then(|| {
            crate::notification::GroupedProgressSession::start(
                &services.notifier,
                "Updating bundle content",
            )
        });

    for update in check.updates_available {
        let child = session.as_ref().map(|s| {
            let c = s.child(update.new_file.display_name(), update.new_file.size.max(1));
            c.set_phase(crate::notification::TaskPhase::Downloading);
            c
        });
        match apply_single_update(&update, &overrides, child.as_ref(), services).await {
            Ok(_) => result.updates_applied.push(update),
            Err(err) => result.updates_failed.push(err.to_string()),
        }
    }

    for addition in check.additions_available {
        let file_id = addition.new_file.kind.package_id();
        let child = session.as_ref().map(|s| {
            let c = s.child(addition.new_file.display_name(), addition.new_file.size.max(1));
            c.set_phase(crate::notification::TaskPhase::Downloading);
            c
        });
        match apply_single_addition(&addition, &overrides, child.as_ref(), services).await {
            Ok(_) => result.additions_applied.push(addition),
            Err(err) => result.additions_failed.push(format!("{file_id}: {err:#}")),
        }
    }

    if let Some(session) = session {
        session.finish();
    }

    let has_changes = !result.updates_applied.is_empty()
        || !result.removals_applied.is_empty()
        || !result.additions_applied.is_empty();

    if has_changes {
        let mut affected = HashSet::new();
        for u in &result.updates_applied {
            affected.insert(u.bundle_name.clone());
        }
        for r in &result.removals_applied {
            affected.insert(r.bundle_name.clone());
        }
        for a in &result.additions_applied {
            affected.insert(a.bundle_name.clone());
        }

        let cluster = PackageStore::get_cluster(cluster_id, services).await?;
        let loader = GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Fabric);
        if let Ok(archives) = bundles
            .archives_for(services, &cluster.mc_version, loader)
            .await
        {
            for archive in archives {
                if affected.contains(&archive.manifest.name) {
                    let _ = overrides::extract_bundle_overrides_no_overwrite(
                        &archive.bundle.path,
                        &cluster,
                    )
                    .await;
                }
            }
        }
    }

    let failures =
        result.updates_failed.len() + result.removals_failed.len() + result.additions_failed.len();
    if failures == 0 {
        sync_cluster_loader_version_from_bundles(cluster_id, bundles, services).await?;
    }

    Ok(result)
}

async fn apply_single_update(
    update: &BundlePackageUpdate,
    overrides: &[ClusterBundleOverrideRow],
    child: Option<&crate::notification::GroupedProgressChild>,
    services: &LauncherServices,
) -> LauncherResult<()> {
    let hash = install_package_from_bundle(
        &update.new_file,
        update.cluster_id,
        &update.bundle_name,
        true,
        child,
        services,
    )
    .await?;
    if let Some(child) = child {
        child.set_phase(crate::notification::TaskPhase::Installing);
        child.finish();
    }

    let file_id = update.new_file.kind.package_id();
    if should_be_disabled(&update.bundle_name, &file_id, overrides) {
        PackageStore::toggle_artifact_enabled(update.cluster_id, &hash, services).await?;
    }

    if hash == update.installed_hash {
        return Ok(());
    }

    remove_artifact_from_cluster(update.cluster_id, &update.installed_hash, false, services).await
}

async fn apply_single_addition(
    addition: &BundlePackageAddition,
    overrides: &[ClusterBundleOverrideRow],
    child: Option<&crate::notification::GroupedProgressChild>,
    services: &LauncherServices,
) -> LauncherResult<()> {
    let hash = install_package_from_bundle(
        &addition.new_file,
        addition.cluster_id,
        &addition.bundle_name,
        true,
        child,
        services,
    )
    .await?;
    if let Some(child) = child {
        child.set_phase(crate::notification::TaskPhase::Installing);
        child.finish();
    }

    let file_id = addition.new_file.kind.package_id();
    if should_be_disabled(&addition.bundle_name, &file_id, overrides) {
        PackageStore::toggle_artifact_enabled(addition.cluster_id, &hash, services).await?;
    }

    Ok(())
}

fn should_be_disabled(
    bundle_name: &str,
    package_id: &str,
    overrides: &[ClusterBundleOverrideRow],
) -> bool {
    overrides.iter().any(|o| {
        o.bundle_name == bundle_name
            && o.package_id == package_id
            && OverrideType::parse(&o.override_type) == Some(OverrideType::Disabled)
    })
}

async fn sync_cluster_loader_version_from_bundles(
    cluster_id: i64,
    bundles: &BundlesManager,
    services: &LauncherServices,
) -> LauncherResult<()> {
    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Fabric);
    let bundle_packages = bundle_dao::list_bundle_tracked(&services.db, cluster_id).await?;
    let all_linked = PackageStore::list_linked_artifacts(cluster_id, services).await?;
    let archives = bundles
        .archives_for(services, &cluster.mc_version, loader)
        .await?;

    let (managed_keys, _) = installed_bundle_keys(services, &all_linked).await?;
    let subscribed: HashSet<String> = bundle_packages
        .iter()
        .filter_map(|bp| bp.bundle_name.clone())
        .chain(infer_subscribed_from_archives(&archives, &managed_keys))
        .collect();

    let target = select_highest_loader_version(
        archives
            .iter()
            .filter(|a| subscribed.contains(&a.manifest.name))
            .map(|a| a.manifest.loader_version.as_str()),
    );

    let Some(target) = target else {
        return Ok(());
    };

    if cluster.mc_loader_version.as_deref() == Some(target.as_str()) {
        return Ok(());
    }

    cluster_dao::update(
        &services.db,
        cluster_id,
        &ClusterPatch {
            mc_loader_version: Some(Some(target)),
            ..Default::default()
        },
    )
    .await?;

    Ok(())
}

pub async fn get_bundles_with_update_status(
    cluster_id: i64,
    bundles: &BundlesManager,
    services: &LauncherServices,
) -> LauncherResult<Vec<BundleWithUpdateStatus>> {
    let cluster = PackageStore::get_cluster(cluster_id, services).await?;
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Fabric);
    let bundle_packages = bundle_dao::list_bundle_tracked(&services.db, cluster_id).await?;
    let all_linked = PackageStore::list_linked_artifacts(cluster_id, services).await?;
    let all_linked_by_hash: HashMap<String, &LinkedArtifactInfo> =
        all_linked.iter().map(|a| (a.hash.clone(), a)).collect();

    let mut installed_map: HashMap<String, &BundleTrackedArtifactRow> = HashMap::new();
    for bundle_pkg in &bundle_packages {
        let Some(pkg_id) = &bundle_pkg.package_id else {
            continue;
        };
        let key = bundle_package_key(bundle_pkg, &all_linked_by_hash, pkg_id);
        installed_map.insert(key, bundle_pkg);
    }

    let overrides = bundle_dao::list_overrides(&services.db, cluster_id).await?;
    let overrides_map: HashMap<(String, String), OverrideType> = overrides
        .iter()
        .filter_map(|o| {
            OverrideType::parse(&o.override_type)
                .map(|t| ((o.bundle_name.clone(), o.package_id.clone()), t))
        })
        .collect();

    let archives = bundles
        .archives_for(services, &cluster.mc_version, loader)
        .await?;
    let mut results = Vec::new();

    for archive in archives {
        let mut files = Vec::new();
        let mut has_updates = false;

        for file in &archive.manifest.files {
            let status = match &file.kind {
                BundleFileKind::Managed {
                    provider,
                    project_id,
                    version_id,
                    ..
                } => {
                    let key = managed_bundle_key(*provider, project_id);
                    if let Some(installed) = installed_map.get(&key) {
                        let installed_version =
                            installed.bundle_version_id.as_deref().unwrap_or("");
                        if installed_version == version_id.as_str() {
                            FileUpdateStatus::UpToDate
                        } else {
                            has_updates = true;
                            FileUpdateStatus::UpdateAvailable {
                                installed_version_id: installed_version.to_string(),
                                new_version_id: version_id.clone(),
                            }
                        }
                    } else if matches!(
                        overrides_map.get(&(archive.manifest.name.clone(), project_id.clone())),
                        Some(OverrideType::Removed)
                    ) {
                        FileUpdateStatus::RemovedByUser
                    } else {
                        FileUpdateStatus::NotInstalled
                    }
                }
                BundleFileKind::External(ext) => {
                    let key = external_bundle_key(&ext.sha1);
                    if installed_map.contains_key(&key) {
                        FileUpdateStatus::UpToDate
                    } else if matches!(
                        overrides_map.get(&(archive.manifest.name.clone(), ext.sha1.clone())),
                        Some(OverrideType::Removed)
                    ) {
                        FileUpdateStatus::RemovedByUser
                    } else {
                        FileUpdateStatus::NotInstalled
                    }
                }
            };
            files.push((file.clone(), status));
        }

        results.push(BundleWithUpdateStatus {
            archive,
            files,
            has_updates,
        });
    }

    Ok(results)
}

pub async fn apply_bundle_updates_for_all_clusters(
    bundles: &BundlesManager,
    services: &LauncherServices,
) -> LauncherResult<Vec<(i64, ApplyBundleUpdatesResult)>> {
    let mut changed = Vec::new();
    for cluster in cluster_dao::list_all(&services.db).await? {
        match apply_bundle_updates(cluster.id, bundles, services).await {
            Ok(result) => {
                if !result.updates_applied.is_empty()
                    || !result.additions_applied.is_empty()
                    || !result.removals_applied.is_empty()
                {
                    changed.push((cluster.id, result));
                }
            }
            Err(err) => tracing::warn!(
                cluster_id = cluster.id,
                error = %err,
                "bundle update apply failed for cluster"
            ),
        }
    }
    Ok(changed)
}

fn bundle_package_key(
    bundle_pkg: &BundleTrackedArtifactRow,
    linked: &HashMap<String, &LinkedArtifactInfo>,
    package_id: &str,
) -> String {
    if package_id == bundle_pkg.hash {
        external_bundle_key(&bundle_pkg.hash)
    } else if let Some(info) = linked.get(&bundle_pkg.hash) {
        managed_bundle_key(info.provider.unwrap_or(ProviderId::Modrinth), package_id)
    } else {
        managed_bundle_key(ProviderId::Modrinth, package_id)
    }
}

async fn installed_bundle_keys(
    services: &LauncherServices,
    linked: &[LinkedArtifactInfo],
) -> LauncherResult<(HashSet<String>, HashSet<String>)> {
    let mut managed = HashSet::new();
    let mut external = HashSet::new();

    for item in linked {
        if let Some(release) = artifact_dao::get_release_by_hash(&services.db, &item.hash).await? {
            let provider =
                ProviderId::from_repr(release.provider as u8).unwrap_or(ProviderId::Modrinth);
            managed.insert(managed_bundle_key(provider, &release.project_id));
        } else {
            external.insert(item.hash.clone());
        }
    }

    Ok((managed, external))
}

fn collect_inferable_managed_keys(
    bundle_files_map: &HashMap<String, (String, crate::bundles::types::BundleFile)>,
    hidden_dependency_keys: &HashSet<String>,
    hidden_explicit_keys: &HashSet<String>,
) -> HashSet<String> {
    bundle_files_map
        .keys()
        .filter(|key| {
            key.starts_with("m:")
                && !hidden_dependency_keys.contains(*key)
                && !hidden_explicit_keys.contains(*key)
        })
        .cloned()
        .collect()
}

fn infer_bundle_names_from_unique_installed_keys(
    candidate_keys_by_bundle: &HashMap<String, HashSet<String>>,
    all_installed_managed_keys: &HashSet<String>,
) -> HashSet<String> {
    let mut key_occurrence_count: HashMap<String, usize> = HashMap::new();
    for candidate_keys in candidate_keys_by_bundle.values() {
        for key in candidate_keys {
            *key_occurrence_count.entry(key.clone()).or_default() += 1;
        }
    }

    candidate_keys_by_bundle
        .iter()
        .filter_map(|(bundle_name, candidate_keys)| {
            let has_unique = candidate_keys.iter().any(|key| {
                all_installed_managed_keys.contains(key)
                    && key_occurrence_count.get(key).copied().unwrap_or_default() == 1
            });
            if has_unique {
                Some(bundle_name.clone())
            } else {
                None
            }
        })
        .collect()
}

fn infer_subscribed_from_archives(
    archives: &[BundleArchive],
    managed_keys: &HashSet<String>,
) -> HashSet<String> {
    let mut candidate_keys_by_bundle = HashMap::new();
    for archive in archives {
        let mut keys = HashSet::new();
        for file in &archive.manifest.files {
            if !file.enabled {
                continue;
            }
            if let BundleFileKind::Managed {
                provider,
                project_id,
                ..
            } = &file.kind
                && !file.hidden
            {
                keys.insert(managed_bundle_key(*provider, project_id));
            }
        }
        candidate_keys_by_bundle.insert(archive.manifest.name.clone(), keys);
    }
    infer_bundle_names_from_unique_installed_keys(&candidate_keys_by_bundle, managed_keys)
}

fn compare_version_segment(left: &str, right: &str) -> Ordering {
    match (left.parse::<u64>(), right.parse::<u64>()) {
        (Ok(left_num), Ok(right_num)) => left_num.cmp(&right_num),
        (Ok(_), Err(_)) => Ordering::Greater,
        (Err(_), Ok(_)) => Ordering::Less,
        (Err(_), Err(_)) => left.cmp(right),
    }
}

fn compare_version_like(left: &str, right: &str) -> Ordering {
    let left_segments: Vec<String> = left
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let right_segments: Vec<String> = right
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();

    for index in 0..left_segments.len().max(right_segments.len()) {
        match (left_segments.get(index), right_segments.get(index)) {
            (Some(left_segment), Some(right_segment)) => {
                let cmp = compare_version_segment(left_segment, right_segment);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => break,
        }
    }
    Ordering::Equal
}

fn select_highest_loader_version<'a>(
    versions: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    versions
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .max_by(|left, right| compare_version_like(left, right))
        .map(str::to_string)
}
