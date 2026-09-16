use std::collections::HashSet;
use std::fs::FileType;
use std::path::{Path, PathBuf};

use oneclient_db::dao::applied_migration as migration_dao;
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster as cluster_dao;

use crate::LauncherResult;
use crate::clusters::Cluster;
use oneclient_cluster::remove_mods_link;
use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_common::paths;
use oneclient_content::packages::store::manifest::{
    self, ManifestEntry, MaterializedManifest,
};
use oneclient_content::packages::store::{
    artifact_absolute_path, link_or_copy, remove_entry, sweep_staging_files,
};
use oneclient_content::packages::PackageStore;
use crate::state::LauncherServices;

const REDIRECTED_DIRS: [&str; 2] = ["logs", "crash-reports"];

const GLOBAL_TYPES: [ContentType; 2] = [ContentType::ResourcePack, ContentType::Shader];

const SWAP_TYPES: [ContentType; 1] = [ContentType::Mod];

fn swap_types(mods_in_cluster: bool) -> &'static [ContentType] {
    if mods_in_cluster { &[] } else { &SWAP_TYPES }
}

const FABRIC_DEP_OVERRIDES: &str = "config/fabric_loader_dependencies.json";

struct Desired {
    content_type: ContentType,
    file_name: String,
    hash: String,
    /// Where it lives in the artifact cache
    src: std::path::PathBuf,
}

impl Desired {
    fn relative_path(&self) -> String {
        manifest::entry_path(self.content_type.folder_name(), &self.file_name)
    }
}

#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id, game_dir = %game_dir.display()), level = "debug")]
pub async fn materialize_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
    mods_in_cluster: bool,
) -> LauncherResult<()> {
    let dedicated = cluster.uses_dedicated_dir();
    let cluster_dir = cluster.dir()?;
    let global_root = paths::shared_minecraft_dir()?;

    polyio::create_dir_all(game_dir).await.ok();
    polyio::create_dir_all(&global_root).await.ok();

    if mods_in_cluster {
        polyio::create_dir_all(paths::cluster_mods_dir(&cluster.folder_name)?)
            .await
            .ok();
        ensure_mods_link(cluster).await;
        prune_mods_links(services).await;
    } else {
        unwind_cluster_mods(cluster, &cluster_dir).await;
    }

    adopt_into_global(&cluster_dir, &global_root).await;
    adopt_into_global(game_dir, &global_root).await;
    ensure_global_links(game_dir, &global_root).await;

    let mods_swapped = !dedicated && !mods_in_cluster;
    drop_stale_notes(&[game_dir, &cluster_dir, &global_root], mods_swapped).await;

    // In the shared directory this often belongs to another cluster so every
    // use of it checks the id
    let previous = manifest::load(game_dir, manifest::MANIFEST_NAME)
        .await
        .map(without_global_entries);
    let previous_mods = manifest::load(&cluster_dir, manifest::MODS_MANIFEST_NAME).await;
    let previous_global = manifest::load(&global_root, manifest::GLOBAL_MANIFEST_NAME).await;
    crate::game::heal::clear_zeroed_files(game_dir).await;
    if mods_in_cluster {
        crate::game::heal::clear_zeroed_mods(&cluster_dir).await;
    }

    let linked = PackageStore::list_linked_artifacts(cluster.id, &services.content())
        .await
        .unwrap_or_default();

    if mods_in_cluster && previous_mods.is_none() && !dedicated {
        let from = game_dir.join(ContentType::Mod.folder_name());
        let into = cluster_dir.join(ContentType::Mod.folder_name());
        let ours = ours_in_folder(ContentType::Mod, &linked, previous.as_ref());

        tracing::info!(cluster_id = cluster.id, "moving mods out of the shared game directory");
        stash_content_files(&from, &into, ContentType::Mod, &ours).await;
    }

    for content_type in GLOBAL_TYPES {
        let dir = global_root.join(content_type.folder_name());
        let disabled = disable_hand_removed(
            services,
            cluster,
            &dir,
            content_type,
            previous_global.as_ref(),
        )
        .await;

        if !disabled.is_empty() {
            let (title, body) = removal_notice(&disabled);
            services.events.notify(title).body(body).send();
        }
    }

    if mods_in_cluster {
        clear_unlinked_mods_once(services, cluster, &cluster_dir.join(ContentType::Mod.folder_name()))
            .await;
    }

    import_manual_content_with(services, cluster, game_dir, mods_in_cluster, true).await;

    if let Err(err) = oneclient_content::packages::reconcile_duplicate_activity(
        cluster.id,
        &services.content(),
    )
    .await
    {
        // Not worth blocking a launch the duplicates were already there
        tracing::warn!(cluster_id = cluster.id, %err, "failed to resolve duplicate package versions");
    }

    match oneclient_content::packages::disable_foreign_game_versions(
        cluster.id,
        &cluster.mc_version,
        cluster.mc_loader as i64,
        &services.content(),
    )
    .await
    {
        Ok(disabled) if !disabled.is_empty() => {
            let names = removal_summary(&disabled);
            let body = if disabled.len() == 1 {
                format!("{names} is built for a different Minecraft version, so it has been switched off in {}.", cluster.name)
            } else {
                format!("{names} are built for a different Minecraft version, so they have been switched off in {}.", cluster.name)
            };
            services.events.notify("Incompatible mods switched off").body(body).send();
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(cluster_id = cluster.id, %err, "failed to switch off mods built for another game version");
        }
    }

    let active_mods_dir = if mods_in_cluster {
        cluster_dir.as_path()
    } else {
        game_dir
    }
    .join(ContentType::Mod.folder_name());
    clear_disabled_mod_files(services, cluster, &active_mods_dir).await;

    let mut unrestored = Vec::new();
    let (mods, rest): (Vec<Desired>, Vec<Desired>) =
        desired_mods(services, cluster, &mut unrestored)
            .await?
            .into_iter()
            .partition(|_| mods_in_cluster);

    // read across every cluster rather than this one so a pack installed anywhere is present here too
    let packs = desired_global(services, &mut unrestored).await?;

    if !unrestored.is_empty() {
        let (title, body) = unrestored_notice(&unrestored);
        services.events.notify(title).body(body).send();
    }

    // Held from the database snapshot through the save so a package removed
    // mid-launch is not resurrected by our own write; the two calls above take
    // it themselves so it cannot be taken any earlier
    let _manifest = manifest::lock().await;

    let mods_root = if mods_in_cluster { &cluster_dir } else { game_dir };
    for content_type in SWAP_TYPES {
        sweep_staging_files(&mods_root.join(content_type.folder_name())).await;
    }
    for content_type in GLOBAL_TYPES {
        sweep_staging_files(&global_root.join(content_type.folder_name())).await;
    }

    // While the game is still closed this is what lands a package removed
    // mid-session and clears another cluster's content from the shared dir
    let mod_paths: HashSet<String> = mods.iter().map(Desired::relative_path).collect();
    let rest_paths: HashSet<String> = rest.iter().map(Desired::relative_path).collect();
    let pack_paths: HashSet<String> = packs.iter().map(Desired::relative_path).collect();

    prune_previous(&cluster_dir, previous_mods.as_ref(), &mod_paths).await;
    prune_previous(game_dir, previous.as_ref(), &rest_paths).await;
    prune_previous(&global_root, previous_global.as_ref(), &pack_paths).await;

    if !dedicated {
        for content_type in swap_types(mods_in_cluster) {
            let dir = game_dir.join(content_type.folder_name());
            let stash = cluster_dir.join(content_type.folder_name());
            polyio::create_dir_all(&dir).await.ok();

            let ours = ours_in_folder(*content_type, &linked, previous.as_ref());
            stash_content_files(&dir, &stash, *content_type, &ours).await;
            ensure_note(&dir, *content_type).await;
            restore_stashed(&stash, &dir, *content_type, &ours).await;
        }
    }

    if mods_in_cluster {
        let mod_entries = link_desired(&cluster_dir, &mods).await;
        drop_unmaterialized(
            &cluster_dir.join(ContentType::Mod.folder_name()),
            ContentType::Mod,
            &mod_entries,
        )
        .await;
        manifest::save(
            &cluster_dir,
            manifest::MODS_MANIFEST_NAME,
            &MaterializedManifest::new(cluster.id, mod_entries),
        )
        .await;
    }

    let pack_entries = link_desired(&global_root, &packs).await;
    manifest::save(
        &global_root,
        manifest::GLOBAL_MANIFEST_NAME,
        &MaterializedManifest::new(cluster.id, pack_entries),
    )
    .await;

    let entries = link_desired(game_dir, &rest).await;
    manifest::save(
        game_dir,
        manifest::MANIFEST_NAME,
        &MaterializedManifest::new(cluster.id, entries),
    )
    .await;

    sync_fabric_dep_overrides(cluster, game_dir).await?;

    Ok(())
}

// every enabled pack across every cluster
async fn desired_global(
    services: &LauncherServices,
    unrestored: &mut Vec<String>,
) -> LauncherResult<Vec<Desired>> {
    let mut desired = Vec::new();

    for content_type in GLOBAL_TYPES {
        for row in artifact_dao::list_global_artifacts(&services.db, content_type as i64).await? {
            if row.enabled == 0 {
                continue;
            }

            let Some(artifact) = artifact_dao::get_artifact_by_hash(&services.db, &row.hash).await?
            else {
                continue;
            };

            let Some(src) = cached_file(services, &row.hash, &artifact.path).await else {
                unrestored.push(row.file_name);
                continue;
            };

            desired.push(Desired {
                content_type,
                file_name: row.file_name,
                hash: row.hash,
                src,
            });
        }
    }

    Ok(desired)
}

fn without_global_entries(mut manifest: MaterializedManifest) -> MaterializedManifest {
    let prefixes: Vec<String> = GLOBAL_TYPES
        .iter()
        .map(|content_type| format!("{}/", content_type.folder_name()))
        .collect();

    manifest
        .entries
        .retain(|entry| !prefixes.iter().any(|prefix| entry.path.starts_with(prefix)));

    manifest
}

// moves a cluster's own pack folders into the shared one before [`ensure_global_links`] replaces them with links
async fn adopt_into_global(own_root: &Path, global_root: &Path) {
    if own_root == global_root {
        return;
    }

    for content_type in GLOBAL_TYPES {
        let own = own_root.join(content_type.folder_name());
        let shared = global_root.join(content_type.folder_name());

        match polyio::symlink_metadata(&own).await {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            _ => continue,
        }

        polyio::create_dir_all(&shared).await.ok();

        let Ok(mut entries) = polyio::read_dir(&own).await else {
            continue;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let dest = shared.join(&name);

            if polyio::symlink_metadata(&dest).await.is_ok() {
                continue;
            }

            if let Err(err) = move_entry(&entry.path(), &dest).await {
                tracing::warn!(
                    file = %name.to_string_lossy(),
                    error = %err,
                    "failed to move content into the shared folder; leaving it in place"
                );
            }
        }
    }
}

// points a cluster's pack folders at the shared ones
async fn ensure_global_links(game_dir: &Path, global_root: &Path) {
    if game_dir == global_root {
        return;
    }

    for content_type in GLOBAL_TYPES {
        let link = game_dir.join(content_type.folder_name());
        let target = global_root.join(content_type.folder_name());
        polyio::create_dir_all(&target).await.ok();

        match polyio::symlink_metadata(&link).await {
            Ok(meta) if meta.file_type().is_symlink() => {
                let aimed_right = matches!(
                    (polyio::canonicalize(&link), polyio::canonicalize(&target)),
                    (Ok(from), Ok(to)) if from == to
                );
                if aimed_right {
                    continue;
                }

                polyio::remove_symlink_dir(&link).await.ok();
            }

            Ok(meta) if meta.is_dir() => {
                if polyio::remove_dir_all(&link).await.is_err() {
                    tracing::warn!(
                        dir = %link.display(),
                        "cannot clear the cluster's own pack folder; leaving it unlinked"
                    );
                    continue;
                }
            }

            Ok(_) => continue,
            Err(_) => {}
        }

        if let Err(err) = polyio::symlink_dir(&target, &link).await {
            tracing::warn!(
                dir = %link.display(),
                error = %err,
                "failed to link the shared pack folder into the game directory"
            );
        }
    }
}

const MASS_REMOVAL_FLOOR: usize = 3;

// names in a manifest whose file is no longer on disk paired with the hash that identifies the row to disable
async fn hand_removed_content(
    dir: &Path,
    content_type: ContentType,
    previous: Option<&MaterializedManifest>,
) -> Vec<(String, String)> {
    let Some(previous) = previous else {
        return Vec::new();
    };

    if polyio::read_dir(dir).await.is_err() {
        tracing::warn!(
            dir = %dir.display(),
            "cannot read the content folder; leaving activity alone"
        );
        return Vec::new();
    }

    let prefix = format!("{}/", content_type.folder_name());
    let mut considered = 0usize;
    let mut removed = Vec::new();

    for entry in &previous.entries {
        let Some(name) = entry.path.strip_prefix(&prefix) else {
            continue;
        };
        considered += 1;

        if polyio::symlink_metadata(dir.join(name)).await.is_ok() {
            continue;
        }

        removed.push((name.to_owned(), entry.hash.clone()));
    }

    if removed.len() == considered && considered >= MASS_REMOVAL_FLOOR {
        tracing::warn!(
            count = considered,
            dir = %dir.display(),
            "everything materialized here is missing; reading that as a folder problem, not as deletions"
        );
        return Vec::new();
    }

    removed
}

// turns hand-removed content off so the next launch stops putting it back
async fn disable_hand_removed(
    services: &LauncherServices,
    cluster: &Cluster,
    dir: &Path,
    content_type: ContentType,
    previous: Option<&MaterializedManifest>,
) -> Vec<String> {
    let removed = hand_removed_content(dir, content_type, previous).await;
    if removed.is_empty() {
        return Vec::new();
    }

    let ctx = services.content();
    let mut disabled = Vec::new();

    for (name, hash) in removed {
        let outcome = if content_type.is_global() {
            disable_globally(cluster, &hash, &ctx).await
        } else {
            oneclient_content::bundles::set_artifact_enabled_to(cluster.id, &hash, false, &ctx)
                .await
                .map(|_| ())
                .map_err(Into::into)
        };

        match outcome {
            Ok(()) => {
                tracing::info!(
                    cluster_id = cluster.id,
                    file = %name,
                    ?content_type,
                    "removed by hand; disabling it instead of restoring it"
                );
                disabled.push(name);
            }
            Err(err) => tracing::warn!(
                cluster_id = cluster.id,
                file = %name,
                error = %err,
                "failed to disable hand-removed content; it will be restored"
            ),
        }
    }

    disabled
}

// switches a globally installed artifact off for every cluster that has it
async fn disable_globally(
    cluster: &Cluster,
    hash: &str,
    ctx: &oneclient_content::ContentCtx,
) -> LauncherResult<()> {
    artifact_dao::set_enabled_for_hash(&ctx.db, hash, 0).await?;
    oneclient_content::bundles::on_user_disable_artifact(cluster.id, hash, ctx).await?;
    Ok(())
}

// at most three names
fn removal_summary(disabled: &[String]) -> String {
    const SHOWN: usize = 3;

    let names = disabled
        .iter()
        .take(SHOWN)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");

    match disabled.len().saturating_sub(SHOWN) {
        0 => names,
        rest => format!("{names} and {rest} more"),
    }
}

fn removal_notice(disabled: &[String]) -> (&'static str, String) {
    let names = removal_summary(disabled);

    if disabled.len() == 1 {
        return (
            "Content disabled",
            format!(
                "{names} is gone from your shared folder, so it has been switched off on every \
                cluster. Turn it back on in OneClient to restore it."
            ),
        );
    }

    (
        "Content disabled",
        format!(
            "{names} are gone from your shared folder, so they have been switched off on every \
            cluster. Turn them back on in OneClient to restore them."
        ),
    )
}

fn unrestored_notice(unrestored: &[String]) -> (&'static str, String) {
    let names = removal_summary(unrestored);

    if unrestored.len() == 1 {
        return (
            "Missing from the game",
            format!(
                "{names} could not be downloaded again, so the game starts without it. \
                Reinstall it in OneClient once you are back online."
            ),
        );
    }

    (
        "Missing from the game",
        format!(
            "{names} could not be downloaded again, so the game starts without them. \
            Reinstall them in OneClient once you are back online."
        ),
    )
}

// puts a cluster back on the old layout after its loader stopped supporting `fabric.modsFolder` (a downgrade or a switch away from Fabric 0.15.0)
async fn unwind_cluster_mods(cluster: &Cluster, cluster_dir: &Path) {
    remove_mods_link(&cluster.folder_name).await;

    let Some(previous) = manifest::load(cluster_dir, manifest::MODS_MANIFEST_NAME).await else {
        return;
    };

    tracing::info!(
        cluster_id = cluster.id,
        "loader cannot be redirected; returning mods to the game directory"
    );

    prune_previous(cluster_dir, Some(&previous), &HashSet::new()).await;
    manifest::clear(cluster_dir, manifest::MODS_MANIFEST_NAME).await;
}

#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id), level = "debug")]
pub async fn dematerialize_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
) -> LauncherResult<()> {
    // Runs first so anything dropped in during the session is a tracked artifact
    // by now and gets dropped rather than stashed as a loose file
    // It loads the manifest itself so the lock comes after it
    import_manual_content(services, cluster, game_dir).await;

    let _manifest = manifest::lock().await;
    let cluster_dir = cluster.dir()?;
    let current = manifest::load(game_dir, manifest::MANIFEST_NAME).await;
    let linked = PackageStore::list_linked_artifacts(cluster.id, &services.content())
        .await
        .unwrap_or_default();

    let mods_in_cluster = manifest::mods_live_in_cluster(&cluster_dir).await;

    for content_type in swap_types(mods_in_cluster) {
        let dir = game_dir.join(content_type.folder_name());
        let stash = cluster_dir.join(content_type.folder_name());
        polyio::create_dir_all(&dir).await.ok();

        let ours = ours_in_folder(*content_type, &linked, current.as_ref());
        stash_content_files(&dir, &stash, *content_type, &ours).await;
        sweep_staging_files(&dir).await;
        ensure_note(&dir, *content_type).await;
    }

    manifest::clear(game_dir, manifest::MANIFEST_NAME).await;
    Ok(())
}

async fn desired_mods(
    services: &LauncherServices,
    cluster: &Cluster,
    unrestored: &mut Vec<String>,
) -> LauncherResult<Vec<Desired>> {
    let linked = PackageStore::list_linked_artifacts(cluster.id, &services.content()).await?;
    let mut desired = Vec::with_capacity(linked.len());

    for link in linked {
        if !link.enabled || link.content_type != ContentType::Mod {
            continue;
        }

        let Some(artifact) = artifact_dao::get_artifact_by_hash(&services.db, &link.hash).await?
        else {
            continue;
        };

        let Some(src) = cached_file(services, &link.hash, &artifact.path).await else {
            unrestored.push(link.cluster_file_name);
            continue;
        };

        desired.push(Desired {
            content_type: link.content_type,
            file_name: link.cluster_file_name,
            hash: link.hash,
            src,
        });
    }

    Ok(desired)
}

async fn cached_file(
    services: &LauncherServices,
    hash: &str,
    stored_path: &str,
) -> Option<PathBuf> {
    let src = artifact_absolute_path(stored_path).ok()?;
    if polyio::try_exists(&src).await.unwrap_or(false) {
        return Some(src);
    }

    let release = artifact_dao::get_release_by_hash(&services.db, hash)
        .await
        .ok()
        .flatten()?;
    let provider = ProviderId::from_repr(release.provider as u8)?;

    tracing::info!(hash, "cached package file is gone; fetching it again");

    let restored = PackageStore::resolve_or_download(
        provider,
        &release.project_id,
        &release.version_id,
        &services.content(),
    )
    .await
    .inspect_err(|err| tracing::warn!(hash, error = %err, "could not restore a cached package"))
    .ok()
    .filter(|artifact| artifact.hash == hash)?;

    let path = artifact_absolute_path(&restored.path).ok()?;
    polyio::try_exists(&path).await.unwrap_or(false).then_some(path)
}

async fn link_desired(root: &Path, desired: &[Desired]) -> Vec<ManifestEntry> {
    let mut entries = Vec::with_capacity(desired.len());

    for item in desired {
        let dest = root
            .join(item.content_type.folder_name())
            .join(&item.file_name);

        match link_or_copy(&item.src, &dest).await {
            Ok(()) => entries.push(ManifestEntry {
                path: item.relative_path(),
                hash: item.hash.clone(),
            }),
            Err(err) => tracing::warn!(
                file = %item.file_name,
                error = %err,
                "failed to materialize content into the game directory"
            ),
        }
    }

    entries
}

const UNLINKED_MODS_REPAIR: &str = "clear-unlinked-mods";

#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id), level = "debug")]
async fn clear_unlinked_mods_once(services: &LauncherServices, cluster: &Cluster, mods_dir: &Path) {
    let id = format!("{UNLINKED_MODS_REPAIR}:{}", cluster.id);
    match migration_dao::is_applied(&services.db, &id).await {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            tracing::warn!(error = %err, "cannot read the repair log; leaving the mods folder alone");
            return;
        }
    }

    let linked = match PackageStore::list_linked_artifacts(cluster.id, &services.content()).await {
        Ok(linked) => linked,
        Err(err) => {
            tracing::warn!(error = %err, "cannot list links; leaving the mods folder alone");
            return;
        }
    };

    let ours = names_linked_here(&linked, ContentType::Mod);
    let Ok(mut entries) = polyio::read_dir(mods_dir).await else {
        mark_repair_applied(services, &id).await;
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with('.')
            || is_note(name)
            || !has_content_extension(ContentType::Mod, name)
            || ours.contains(name)
        {
            continue;
        }

        tracing::info!(file = name, "clearing a mod this cluster never linked");
        if let Err(err) = remove_entry(&path).await {
            tracing::warn!(file = name, error = %err, "failed to clear an unlinked mod");
        }
    }

    mark_repair_applied(services, &id).await;
}

async fn mark_repair_applied(services: &LauncherServices, id: &str) {
    if let Err(err) = migration_dao::mark_applied(&services.db, id).await {
        tracing::warn!(repair = id, error = %err, "failed to record a one-time repair");
    }
}

async fn drop_unmaterialized(dir: &Path, content_type: ContentType, entries: &[ManifestEntry]) {
    let kept: HashSet<&str> = entries
        .iter()
        .filter_map(|entry| entry.path.rsplit('/').next())
        .collect();

    let Ok(mut read) = polyio::read_dir(dir).await else {
        return;
    };

    while let Ok(Some(entry)) = read.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with('.') || is_note(name) || !has_content_extension(content_type, name) {
            continue;
        }
        if kept.contains(name) {
            continue;
        }

        tracing::info!(
            file = name,
            dir = %dir.display(),
            "clearing content the launcher no longer lists for this cluster"
        );
        if let Err(err) = remove_entry(&path).await {
            tracing::warn!(file = name, error = %err, "failed to clear stale content");
        }
    }
}

/// Entries are keyed by path so a package whose file name is unchanged is left
/// in place rather than being deleted and relinked on every launch
async fn prune_previous(
    root: &Path,
    previous: Option<&MaterializedManifest>,
    keep: &HashSet<String>,
) {
    let Some(previous) = previous else {
        return;
    };

    for entry in &previous.entries {
        if keep.contains(&entry.path) {
            continue;
        }

        let path = root.join(&entry.path);
        if let Err(err) = remove_entry(&path).await {
            tracing::warn!(
                file = %entry.path,
                error = %err,
                "failed to clear stale materialized content"
            );
        }
    }
}

#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id), level = "debug")]
async fn clear_disabled_mod_files(services: &LauncherServices, cluster: &Cluster, mods_dir: &Path) {
    let linked = match PackageStore::list_linked_artifacts(cluster.id, &services.content()).await {
        Ok(linked) => linked,
        Err(err) => {
            tracing::warn!(error = %err, "cannot list links; leaving the mods folder alone");
            return;
        }
    };

    for name in disabled_mod_names(&linked) {
        let path = mods_dir.join(&name);
        if polyio::symlink_metadata(&path).await.is_err() {
            continue;
        }

        match remove_entry(&path).await {
            Ok(()) => tracing::info!(file = %name, "cleared a switched-off mod from the folder"),
            Err(err) => {
                tracing::warn!(file = %name, error = %err, "failed to clear a switched-off mod")
            }
        }
    }
}

fn disabled_mod_names(
    linked: &[oneclient_content::packages::LinkedArtifactInfo],
) -> HashSet<String> {
    let mods = || {
        linked
            .iter()
            .filter(|link| link.content_type == ContentType::Mod)
    };

    let live: HashSet<&str> = mods()
        .filter(|link| link.enabled)
        .map(|link| link.cluster_file_name.as_str())
        .collect();

    mods()
        .filter(|link| !link.enabled)
        .map(|link| link.cluster_file_name.as_str())
        .filter(|name| !live.contains(name))
        .map(str::to_owned)
        .collect()
}

/// Names in one folder that are the launcher's not the user's what we
/// materialized last plus what the database tracks (covering files just
/// adopted by [`import_manual_content`])
fn ours_in_folder(
    content_type: ContentType,
    linked: &[oneclient_content::packages::LinkedArtifactInfo],
    manifest: Option<&MaterializedManifest>,
) -> HashSet<String> {
    let mut names: HashSet<String> = linked
        .iter()
        .filter(|link| link.content_type == content_type)
        .map(|link| link.cluster_file_name.clone())
        .collect();

    let folder = content_type.folder_name();
    if let Some(manifest) = manifest {
        names.extend(
            manifest
                .entries
                .iter()
                .filter_map(|entry| entry.path.strip_prefix(&format!("{folder}/")))
                .map(str::to_owned),
        );
    }

    names
}

#[tracing::instrument(skip(services, cluster), fields(cluster_id = cluster.id), level = "debug")]
pub async fn import_manual_content(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
) {
    let mods_in_cluster = match cluster.dir() {
        Ok(dir) => manifest::mods_live_in_cluster(&dir).await,
        Err(_) => false,
    };

    import_manual_content_with(services, cluster, game_dir, mods_in_cluster, false).await;
}

async fn import_manual_content_with(
    services: &LauncherServices,
    cluster: &Cluster,
    game_dir: &Path,
    mods_in_cluster: bool,
    discard_originals: bool,
) {
    let linked = match PackageStore::list_linked_artifacts(cluster.id, &services.content()).await {
        Ok(linked) => linked,
        Err(err) => {
            tracing::warn!(error = %err, "failed to list links; skipping manual-content import");
            return;
        }
    };

    // Not held across the import loop below, which is long and does not need it
    let manifest = {
        let _guard = manifest::lock().await;
        manifest::load(game_dir, manifest::MANIFEST_NAME).await
    };

    // under the old layout mods sit in the game directory and are matched
    // against its manifest exactly like resource packs and shaders
    let cluster_dir = cluster.dir().ok();
    let mods_manifest = match cluster_dir.as_deref() {
        Some(dir) if mods_in_cluster => manifest::load(dir, manifest::MODS_MANIFEST_NAME).await,
        _ => None,
    };

    let (mods_dir, mods_manifest) = match cluster_dir.as_deref() {
        Some(dir) if mods_in_cluster => (
            dir.join(ContentType::Mod.folder_name()),
            mods_manifest.as_ref(),
        ),
        _ => (
            game_dir.join(ContentType::Mod.folder_name()),
            manifest.as_ref(),
        ),
    };

    import_from_dir(
        services,
        cluster,
        &mods_dir,
        ContentType::Mod,
        &names_linked_here(&linked, ContentType::Mod),
        mods_manifest,
        discard_originals,
    )
    .await;

    let Ok(global_root) = paths::shared_minecraft_dir() else {
        return;
    };
    let global_manifest = manifest::load(&global_root, manifest::GLOBAL_MANIFEST_NAME).await;

    for content_type in GLOBAL_TYPES {
        let dir = global_root.join(content_type.folder_name());
        let known = match artifact_dao::list_global_artifacts(&services.db, content_type as i64)
            .await
        {
            Ok(rows) => rows.into_iter().map(|row| row.file_name).collect(),
            Err(err) => {
                tracing::warn!(error = %err, "cannot list global content; skipping its import");
                continue;
            }
        };

        import_from_dir(
            services,
            cluster,
            &dir,
            content_type,
            &known,
            global_manifest.as_ref(),
            discard_originals,
        )
        .await;
    }
}

fn names_linked_here(
    linked: &[oneclient_content::packages::LinkedArtifactInfo],
    content_type: ContentType,
) -> HashSet<String> {
    linked
        .iter()
        .filter(|link| link.content_type == content_type)
        .map(|link| link.cluster_file_name.clone())
        .collect()
}

async fn import_from_dir(
    services: &LauncherServices,
    cluster: &Cluster,
    dir: &Path,
    content_type: ContentType,
    known: &HashSet<String>,
    manifest: Option<&MaterializedManifest>,
    discard_originals: bool,
) {
    let Ok(mut entries) = polyio::read_dir(dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }

        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') || !has_content_extension(content_type, name) {
            continue;
        }
        if known.contains(name) {
            continue;
        }

        let relative = manifest::entry_path(content_type.folder_name(), name);
        if manifest.is_some_and(|m| m.contains(&relative)) {
            continue;
        }

        if is_cached_artifact(services, &path).await {
            tracing::debug!(
                file = name,
                dir = %dir.display(),
                "discarding stale launcher content; the cache still holds it"
            );
            if let Err(err) = polyio::remove_file(&path).await {
                tracing::warn!(file = name, error = %err, "failed to discard stale content");
            }
            continue;
        }

        match PackageStore::import_local_file(&path, content_type, cluster.id, &services.content())
            .await
        {
            Ok(row) => {
                tracing::debug!(file = name, "registered manually-added content");
                if discard_originals {
                    discard_adopted_original(&row, &path, name).await;
                }
            }
            Err(err) => tracing::warn!(
                file = name,
                error = %err,
                "failed to register manually-added content"
            ),
        }
    }
}

async fn discard_adopted_original(
    row: &oneclient_db::models::ArtifactRow,
    path: &Path,
    name: &str,
) {
    match artifact_absolute_path(&row.path) {
        Ok(cached) if polyio::try_exists(&cached).await.unwrap_or(false) => {}
        _ => {
            tracing::warn!(file = name, "adopted content is not in the cache; leaving the original");
            return;
        }
    }

    if let Err(err) = polyio::remove_file(path).await {
        tracing::warn!(file = name, error = %err, "failed to clear the adopted original");
    }
}

async fn is_cached_artifact(services: &LauncherServices, path: &Path) -> bool {
    let Ok(hash) = polyio::sha1_file(path).await else {
        return false;
    };

    artifact_dao::get_artifact_by_hash(&services.db, &polyio::normalize_hash(&hash))
        .await
        .ok()
        .flatten()
        .is_some()
}

fn has_content_extension(content_type: ContentType, name: &str) -> bool {
    let lower = name.to_lowercase();
    match content_type {
        ContentType::Mod => lower.ends_with(".jar"),
        ContentType::ResourcePack | ContentType::Shader => lower.ends_with(".zip"),
        _ => false,
    }
}

// puts this cluster's mods folder into the shared `mods` directory
#[tracing::instrument(skip(cluster), fields(cluster_id = cluster.id), level = "debug")]
async fn ensure_mods_link(cluster: &Cluster) {
    let (Ok(link), Ok(target)) = (
        paths::shared_mods_link(&cluster.folder_name),
        paths::cluster_mods_dir(&cluster.folder_name),
    ) else {
        return;
    };

    match polyio::symlink_metadata(&link).await {
        Ok(meta) if meta.file_type().is_symlink() => {
            let aimed_right = matches!(
                (polyio::canonicalize(&link), polyio::canonicalize(&target)),
                (Ok(from), Ok(to)) if from == to
            );
            if aimed_right {
                return;
            }

            polyio::remove_symlink_dir(&link).await.ok();
        }

        Ok(_) => {
            tracing::warn!(
                folder = %cluster.folder_name,
                "shared mods folder holds a real entry under this name; not linking"
            );
            return;
        }

        Err(_) => {}
    }

    if let Some(parent) = link.parent() {
        polyio::create_dir_all(parent).await.ok();
        ensure_links_note(parent).await;
    }
    polyio::create_dir_all(&target).await.ok();

    if let Err(err) = polyio::symlink_dir(&target, &link).await {
        tracing::warn!(
            folder = %cluster.folder_name,
            error = %err,
            "failed to link cluster mods into the shared minecraft folder"
        );
    }
}

async fn points_into_clusters_dir(path: &Path) -> bool {
    let (Ok(target), Ok(root)) = (polyio::read_link(path).await, paths::clusters_dir()) else {
        return false;
    };

    target.starts_with(root)
}

// drops links a deleted cluster left behind
#[tracing::instrument(skip(services), level = "debug")]
async fn prune_mods_links(services: &LauncherServices) {
    let Ok(root) = paths::shared_mods_dir() else {
        return;
    };

    let Ok(mut entries) = polyio::read_dir(&root).await else {
        return;
    };

    let known: HashSet<String> = match cluster_dao::list_all(&services.db).await {
        Ok(rows) => rows.into_iter().map(|row| row.folder_name).collect(),
        Err(err) => {
            tracing::warn!(error = %err, "cannot list clusters; leaving shared mods links alone");
            return;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        if !file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if known.contains(&name) {
            continue;
        }

        if !points_into_clusters_dir(&entry.path()).await {
            continue;
        }

        match polyio::remove_symlink_dir(entry.path()).await {
            Ok(()) => tracing::debug!(link = %name, "cleared mods link for a deleted cluster"),
            Err(err) => {
                tracing::warn!(link = %name, error = %err, "failed to clear stale cluster mods link")
            }
        }
    }
}

const ALLOWED_SYMLINKS_NAME: &str = "allowed_symlinks.txt";

#[tracing::instrument(level = "debug")]
pub async fn write_allowed_symlinks(game_dir: &Path) -> LauncherResult<()> {
    let root = oneclient_common::paths::data_dir()?;

    // Distributions that keep the home directory behind a symlink resolve to a
    // different prefix than the one the launcher hands the game (Fedora Atomic
    // and its derivatives put /home behind /var/home), and a prefix the game
    // does not recognise makes it refuse every linked pack, so allow both
    let mut roots = vec![root.to_path_buf()];
    if let Ok(canonical) = polyio::canonicalize(root)
        && canonical != root
    {
        roots.push(canonical);
    }

    polyio::write(
        game_dir.join(ALLOWED_SYMLINKS_NAME),
        allowed_symlinks_body(&roots),
    )
    .await?;
    Ok(())
}

fn allowed_symlinks_body(roots: &[PathBuf]) -> String {
    let sep = std::path::MAIN_SEPARATOR;

    roots
        .iter()
        .map(|root| format!("[prefix]{}{}", root.to_string_lossy(), sep))
        .collect::<Vec<_>>()
        .join("\n")
}

const EMPTY_NOTE_NAME: &str = "WHY_NOTHING_HERE.txt";
const LINKS_NOTE_NAME: &str = "EACH_FOLDER_IS_A_CLUSTER.txt";

fn is_note(name: &str) -> bool {
    name == EMPTY_NOTE_NAME || name == LINKS_NOTE_NAME
}

async fn ensure_links_note(dir: &Path) {
    let note = dir.join(LINKS_NOTE_NAME);

    if polyio::try_exists(&note).await.unwrap_or(false) {
        return;
    }

    let noun = ContentType::Mod.folder_name();

    polyio::write(
        &note,
        format!(
            "PLEASE READ CAREFULLY!!!!\n\
            \n\
            OneClient SPLITS your {noun} folder per version/cluster, so that you can have \
            separate {noun} in each.\n\
            \n\
            Add / remove {noun} IN THESE FOLDERS!!!\n\
            \n\
            Also... you can drag your {noun} and stuff straight into the launcher as well...\n"
        ),
    )
    .await
    .ok();
}

async fn stash_content_files(
    dir: &Path,
    stash: &Path,
    content_type: ContentType,
    ours: &HashSet<String>,
) {
    let Ok(mut entries) = polyio::read_dir(dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if is_note(&name) || name.starts_with('.') {
            continue;
        }

        let Ok(file_type) = entry.file_type().await else {
            continue;
        };

        if file_type.is_symlink() {
            if points_into_clusters_dir(&path).await {
                continue;
            }

            remove_dir_or_file(&path, file_type).await;
            continue;
        }

        if ours.contains(&name) {
            remove_dir_or_file(&path, file_type).await;
            continue;
        }

        if file_type.is_file() && has_content_extension(content_type, &name) {
            tracing::debug!(
                file = %name,
                dir = %dir.display(),
                "leaving unrecognised content where it is rather than stashing it"
            );
            continue;
        }

        let dest = stash.join(&name);
        if let Err(err) = move_entry(&path, &dest).await {
            tracing::warn!(
                file = %name,
                error = %err,
                "failed to stash content into cluster; leaving it in place"
            );
        }
    }
}

async fn restore_stashed(
    stash: &Path,
    dir: &Path,
    content_type: ContentType,
    ours: &HashSet<String>,
) {
    let Ok(mut entries) = polyio::read_dir(stash).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if is_note(&name) || name.starts_with('.') || ours.contains(&name) {
            continue;
        }

        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        // A link into the artifact cache managed content is materialized from
        // the database not from here
        if file_type.is_symlink() {
            continue;
        }

        // A bare jar in the stash is a leftover from the old scheme often with
        // its package already removed restoring it would put a deleted mod back
        // every launch
        // Left on disk for the Storage settings page to clear
        if file_type.is_file() && has_content_extension(content_type, &name) {
            tracing::debug!(
                file = %name,
                "ignoring stray content in cluster folder; use the Storage settings page to clear it"
            );
            continue;
        }

        let dest = dir.join(&name);
        let result = if file_type.is_dir() {
            polyio::symlink_dir(&path, &dest).await.map_err(Into::into)
        } else {
            link_or_copy(&path, &dest).await
        };

        if let Err(err) = result {
            tracing::warn!(
                file = %name,
                error = %err,
                "failed to restore stashed content into game dir"
            );
        }
    }
}

async fn remove_dir_or_file(path: &Path, file_type: FileType) {
    if file_type.is_dir() {
        polyio::remove_dir_all(path).await.ok();
    } else if polyio::remove_file(path).await.is_err() {
        // A Windows junction has to go through `remove_dir`
        polyio::remove_symlink_dir(path).await.ok();
    }
}

async fn move_entry(src: &Path, dest: &Path) -> LauncherResult<()> {
    if let Some(parent) = dest.parent() {
        polyio::create_dir_all(parent).await.ok();
    }

    // A stash from an earlier session is older than what the game just wrote
    // so it loses
    if let Ok(meta) = polyio::symlink_metadata(dest).await {
        remove_dir_or_file(dest, meta.file_type()).await;
    }

    if polyio::rename(src, dest).await.is_ok() {
        return Ok(());
    }

    // `rename` cannot cross devices fall back to a copy for plain files
    polyio::copy(src, dest).await?;
    polyio::remove_file(src).await.ok();
    Ok(())
}

async fn drop_stale_notes(roots: &[&Path], mods_swapped: bool) {
    let mut swept: Vec<&Path> = Vec::new();

    for root in roots {
        // The shared game directory *is* the global root for a cluster without
        // a dedicated directory, and both are the cluster folder for one with
        // it visiting a root twice would only walk the same folders again
        if swept.contains(root) {
            continue;
        }
        swept.push(*root);

        for content_type in GLOBAL_TYPES {
            drop_note(&root.join(content_type.folder_name())).await;
        }

        if !mods_swapped {
            drop_note(&root.join(ContentType::Mod.folder_name())).await;
        }
    }
}

async fn drop_note(dir: &Path) {
    let note = dir.join(EMPTY_NOTE_NAME);
    if polyio::symlink_metadata(&note).await.is_err() {
        return;
    }

    match polyio::remove_file(&note).await {
        Ok(()) => tracing::debug!(dir = %dir.display(), "removed a stale empty-folder note"),
        // Nothing downstream reads it the next launch tries again
        Err(err) => tracing::debug!(
            dir = %dir.display(),
            error = %err,
            "could not remove the stale empty-folder note"
        ),
    }
}

async fn ensure_note(dir: &Path, content_type: ContentType) {
    let note = dir.join(EMPTY_NOTE_NAME);

    if polyio::try_exists(&note).await.unwrap_or(false) {
        return;
    }

    polyio::write(&note, {
        let noun = content_type.folder_name();

        format!(
            "It's empty here, but nothing is broken!\n\
        \n\
        OneClient keeps your {noun} safe somewhere else (specifically in the global launcher cache)\
		and only puts them here while you play. When you close the game, it tidies them away again.\n\
        \n\
        Want to add {noun}? The best way is to do it right inside OneClient. Or you can drop \
        files in this folder, and OneClient will pick them up the next time you play.\n"
        )
    })
    .await
    .ok();
}

#[tracing::instrument(skip(cluster), fields(cluster_id = cluster.id), level = "debug")]
pub async fn link_cluster_logs(cluster: &Cluster, game_dir: &Path) {
    let cluster_dir = match cluster.dir() {
        Ok(dir) => dir,
        Err(err) => {
            tracing::warn!(error = %err, "cannot resolve cluster dir; skipping log redirect");
            return;
        }
    };

    for name in REDIRECTED_DIRS {
        let target = cluster_dir.join(name);
        let shared = game_dir.join(name);

        if let Err(err) = redirect_dir(&shared, &target).await {
            tracing::warn!(
                dir = name,
                error = %err,
                "failed to redirect shared game dir into cluster; logs may pool in shared dir"
            );
        }
    }
}

async fn redirect_dir(shared: &Path, target: &Path) -> LauncherResult<()> {
    polyio::create_dir_all(target).await.ok();

    match polyio::symlink_metadata(shared).await {
        Ok(meta) if meta.file_type().is_symlink() => {
            polyio::remove_symlink_dir(shared).await?;
        }

        // A real directory likely the user's so salvage its contents into the
        // cluster folder instead of deleting it
        Ok(meta) if meta.is_dir() => {
            move_dir_contents(shared, target).await;
            polyio::remove_dir_all(shared).await.ok();
        }

        Ok(_) => {
            polyio::remove_file(shared).await.ok();
        }

        Err(_) => {}
    }

    if let Some(parent) = shared.parent() {
        polyio::create_dir_all(parent).await.ok();
    }
    polyio::symlink_dir(target, shared).await?;
    Ok(())
}

#[tracing::instrument(level = "debug")]
pub async fn unlink_cluster_logs(game_dir: &Path) {
    for name in REDIRECTED_DIRS {
        let shared = game_dir.join(name);

        match polyio::symlink_metadata(&shared).await {
            Ok(meta) if meta.file_type().is_symlink() => {
                if let Err(err) = polyio::remove_symlink_dir(&shared).await {
                    tracing::warn!(dir = name, error = %err, "failed to unlink shared log dir");
                }
            }
            Ok(_) => {
                tracing::warn!(
                    dir = name,
                    "shared log dir is not our link; leaving as-is (next launch will salvage it)"
                );
            }
            Err(_) => {}
        }
    }
}

async fn move_dir_contents(from: &Path, to: &Path) {
    let Ok(mut entries) = polyio::read_dir(from).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let src = entry.path();
        let Some(name) = src.file_name() else {
            continue;
        };

        let dest = to.join(name);

        if polyio::rename(&src, &dest).await.is_err() {
            tracing::warn!(file = %src.display(), "failed to salvage leaked log file");
        }
    }
}

async fn sync_fabric_dep_overrides(cluster: &Cluster, game_dir: &Path) -> LauncherResult<()> {
    let src = cluster.dir()?.join(FABRIC_DEP_OVERRIDES);
    let dest = game_dir.join(FABRIC_DEP_OVERRIDES);

    if src == dest {
        return Ok(());
    }

    if polyio::try_exists(&src).await.unwrap_or(false) {
        if let Some(parent) = dest.parent() {
            polyio::create_dir_all(parent).await.ok();
        }
        polyio::copy(&src, &dest).await?;
    } else if polyio::try_exists(&dest).await.unwrap_or(false) {
        polyio::remove_file(&dest).await.ok();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_symlinked_home_gets_both_prefixes() {
        let body = allowed_symlinks_body(&[
            PathBuf::from("/home/alex/.local/share/OneClient"),
            PathBuf::from("/var/home/alex/.local/share/OneClient"),
        ]);

        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("[prefix]/home/alex/"));
        assert!(lines[1].starts_with("[prefix]/var/home/alex/"));
        assert!(lines.iter().all(|line| line.ends_with(std::path::MAIN_SEPARATOR)));
    }

    #[test]
    fn an_ordinary_home_gets_one_prefix() {
        let body = allowed_symlinks_body(&[PathBuf::from("/home/alex/.local/share/OneClient")]);

        assert_eq!(body.lines().count(), 1);
    }

    use super::*;

    fn link(file_name: &str, enabled: bool) -> oneclient_content::packages::LinkedArtifactInfo {
        oneclient_content::packages::LinkedArtifactInfo {
            hash: format!("{file_name}-hash"),
            cluster_file_name: file_name.into(),
            enabled,
            content_type: ContentType::Mod,
            file_name: file_name.into(),
            project_id: None,
            version_id: None,
            display_name: None,
            display_version: None,
            provider: None,
            published_at: None,
            seen_status: oneclient_db::models::SeenStatus::Seen,
        }
    }

    #[test]
    fn a_switched_off_jar_goes_unless_a_live_copy_shares_its_name() {
        let names = disabled_mod_names(&[
            link("NBTac-FABRIC-26.2-2.0.1.jar", true),
            link("NBTac-FABRIC-26.1-1.3.15.jar", false),
            link("shared.jar", true),
            link("shared.jar", false),
        ]);

        assert!(
            names.contains("NBTac-FABRIC-26.1-1.3.15.jar"),
            "the jar the old stash stranded has to go"
        );
        assert!(
            !names.contains("NBTac-FABRIC-26.2-2.0.1.jar"),
            "a mod that is on is never cleared"
        );
        assert!(
            !names.contains("shared.jar"),
            "a name another copy still has switched on must survive"
        );
    }

    #[test]
    fn other_content_types_are_left_to_their_own_folders() {
        let mut pack = link("pack.zip", false);
        pack.content_type = ContentType::ResourcePack;

        assert!(disabled_mod_names(&[pack]).is_empty());
    }

    fn names(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| (*n).to_string()).collect()
    }

    fn manifest_of(cluster_id: i64, paths: &[&str]) -> MaterializedManifest {
        MaterializedManifest::new(
            cluster_id,
            paths
                .iter()
                .map(|p| ManifestEntry {
                    path: (*p).to_string(),
                    hash: "hash".into(),
                })
                .collect(),
        )
    }

    /// A shaderpack's settings sidecar has to survive a launch/exit cycle and
    /// end up in the cluster rather than the shared dir
    #[tokio::test]
    async fn shader_settings_survive_a_session() {
        let root = polyio::testing::ScratchDir::new("shader_settings");
        let shared = root.join("shared").join("shaderpacks");
        let stash = root.join("cluster").join("shaderpacks");
        polyio::create_dir_all(&shared).await.unwrap();

        let ours = names(&["bsl.zip"]);

        polyio::write(shared.join("bsl.zip"), b"pack".as_slice())
            .await
            .unwrap();
        polyio::write(shared.join("bsl.zip.txt"), b"BLOOM=off".as_slice())
            .await
            .unwrap();

        stash_content_files(&shared, &stash, ContentType::Shader, &ours).await;
        assert!(!shared.join("bsl.zip").exists(), "managed pack left behind");
        assert!(!shared.join("bsl.zip.txt").exists(), "sidecar left behind");
        assert_eq!(
            polyio::read_to_string(stash.join("bsl.zip.txt")).await.unwrap(),
            "BLOOM=off"
        );

        restore_stashed(&stash, &shared, ContentType::Shader, &ours).await;
        assert_eq!(
            polyio::read_to_string(shared.join("bsl.zip.txt")).await.unwrap(),
            "BLOOM=off"
        );

        polyio::write(stash.join("bsl.zip.txt"), b"BLOOM=on".as_slice())
            .await
            .unwrap();
        assert_eq!(
            polyio::read_to_string(shared.join("bsl.zip.txt")).await.unwrap(),
            "BLOOM=on"
        );

        std::fs::remove_dir_all(root.path()).ok();
    }

    #[tokio::test]
    async fn unpacked_dirs_move_into_the_cluster() {
        let root = polyio::testing::ScratchDir::new("unpacked");
        let shared = root.join("shared").join("shaderpacks");
        let stash = root.join("cluster").join("shaderpacks");
        polyio::create_dir_all(shared.join("Loose/shaders")).await.unwrap();
        polyio::write(shared.join("Loose/shaders/final.fsh"), b"void main".as_slice())
            .await
            .unwrap();

        stash_content_files(&shared, &stash, ContentType::Shader, &HashSet::new()).await;
        assert!(!shared.join("Loose").exists());
        assert!(stash.join("Loose/shaders/final.fsh").exists());

        restore_stashed(&stash, &shared, ContentType::Shader, &HashSet::new()).await;
        assert!(shared.join("Loose/shaders/final.fsh").exists());

        // Only the link goes never the stashed original
        stash_content_files(&shared, &stash, ContentType::Shader, &HashSet::new()).await;
        assert!(!shared.join("Loose").exists());
        assert!(stash.join("Loose/shaders/final.fsh").exists());

        std::fs::remove_dir_all(root.path()).ok();
    }

    #[tokio::test]
    async fn note_is_never_stashed() {
        let root = polyio::testing::ScratchDir::new("note");
        let shared = root.join("shared").join("mods");
        let stash = root.join("cluster").join("mods");
        polyio::create_dir_all(&shared).await.unwrap();
        polyio::write(shared.join(EMPTY_NOTE_NAME), b"hi".as_slice())
            .await
            .unwrap();

        stash_content_files(&shared, &stash, ContentType::Mod, &HashSet::new()).await;
        assert!(shared.join(EMPTY_NOTE_NAME).exists());
        assert!(!stash.join(EMPTY_NOTE_NAME).exists());

        std::fs::remove_dir_all(root.path()).ok();
    }

    /// The reported bug a package removed while the game held it open stays in
    /// the folder so the next launch must clear it
    /// An ordinary file is used
    /// because that is what a managed file looks like on Windows
    #[tokio::test]
    async fn removed_package_is_pruned_at_the_next_launch() {
        let root = polyio::testing::ScratchDir::new("prune_removed");
        let game_dir = root.path();
        polyio::create_dir_all(game_dir.join("mods")).await.unwrap();

        let jar = game_dir.join("mods").join("removed.jar");
        polyio::write(&jar, b"jar".as_slice()).await.unwrap();

        let previous = manifest_of(1, &["mods/removed.jar", "mods/kept.jar"]);
        let keep: HashSet<String> = ["mods/kept.jar".to_string()].into_iter().collect();

        prune_previous(game_dir, Some(&previous), &keep).await;

        assert!(
            polyio::symlink_metadata(&jar).await.is_err(),
            "a removed package must not survive into the next session"
        );

        std::fs::remove_dir_all(root.path()).ok();
    }

    /// Whatever a previous cluster materialized in the shared dir is ours to
    /// clear not the user's to keep
    #[tokio::test]
    async fn another_clusters_content_is_cleared_not_stashed() {
        let root = polyio::testing::ScratchDir::new("cross_cluster");
        let game_dir = root.path();
        polyio::create_dir_all(game_dir.join("mods")).await.unwrap();

        let jar = game_dir.join("mods").join("theirs.jar");
        polyio::write(&jar, b"jar".as_slice()).await.unwrap();

        prune_previous(game_dir, Some(&manifest_of(1, &["mods/theirs.jar"])), &HashSet::new()).await;

        assert!(polyio::symlink_metadata(&jar).await.is_err());

        std::fs::remove_dir_all(root.path()).ok();
    }

    /// A hand-dropped file is in no manifest so it is stashed into the cluster
    /// rather than deleted
    #[tokio::test]
    async fn user_files_are_never_pruned() {
        let root = polyio::testing::ScratchDir::new("user_file");
        let game_dir = root.path();
        polyio::create_dir_all(game_dir.join("mods")).await.unwrap();

        let mine = game_dir.join("mods").join("handmade.jar");
        polyio::write(&mine, b"jar".as_slice()).await.unwrap();

        prune_previous(game_dir, Some(&manifest_of(1, &["mods/ours.jar"])), &HashSet::new()).await;

        assert!(mine.exists(), "a file we never materialized is not ours to delete");

        std::fs::remove_dir_all(root.path()).ok();
    }

    /// A jar left in a cluster folder by an older launcher whose package has
    /// since been removed must not be linked back into the game
    #[tokio::test]
    async fn stray_content_in_the_stash_is_not_restored() {
        let root = polyio::testing::ScratchDir::new("stray_content");
        let shared = root.join("shared").join("mods");
        let stash = root.join("cluster").join("mods");
        polyio::create_dir_all(&shared).await.unwrap();
        polyio::create_dir_all(&stash).await.unwrap();

        // A leftover from the old scheme an ordinary file (which is what a
        // Windows hard link looks like) with no database row to explain it
        polyio::write(stash.join("removed.jar"), b"jar".as_slice())
            .await
            .unwrap();
        polyio::write(stash.join("options.txt"), b"k=v".as_slice())
            .await
            .unwrap();

        restore_stashed(&stash, &shared, ContentType::Mod, &HashSet::new()).await;

        assert!(
            !shared.join("removed.jar").exists(),
            "a removed package must not be restored into the game"
        );
        assert!(
            stash.join("removed.jar").exists(),
            "and it must not be silently deleted either"
        );
        assert!(shared.join("options.txt").exists(), "sidecars still restore");

        std::fs::remove_dir_all(root.path()).ok();
    }
    /// Pinned because the halves are easy to swap `stash` reads the game dir
    /// and writes the cluster `restore` the reverse and both take two
    /// same-typed `&Path`s a swap would quietly delete a user's files
    #[tokio::test]
    async fn a_session_never_removes_anything_from_the_cluster_folder() {
        let root = polyio::testing::ScratchDir::new("stash_is_sacred");
        let shared = root.join("shared").join("mods");
        let stash = root.join("cluster").join("mods");
        polyio::create_dir_all(&shared).await.unwrap();
        polyio::create_dir_all(&stash).await.unwrap();

        polyio::write(stash.join("options.txt"), b"k=v".as_slice())
            .await
            .unwrap();
        polyio::write(stash.join("leftover.jar"), b"jar".as_slice())
            .await
            .unwrap();
        polyio::create_dir_all(stash.join("unpacked")).await.unwrap();
        polyio::write(stash.join("unpacked").join("inner.txt"), b"x".as_slice())
            .await
            .unwrap();

        polyio::write(shared.join("managed.jar"), b"jar".as_slice())
            .await
            .unwrap();

        let ours = names(&["managed.jar"]);

        let before = dir_entries(&stash).await;

        stash_content_files(&shared, &stash, ContentType::Mod, &ours).await;
        restore_stashed(&stash, &shared, ContentType::Mod, &ours).await;
        stash_content_files(&shared, &stash, ContentType::Mod, &ours).await;
        restore_stashed(&stash, &shared, ContentType::Mod, &ours).await;

        let after = dir_entries(&stash).await;

        assert_eq!(
            before, after,
            "a session must not add to or remove from the cluster folder"
        );
        assert!(
            stash.join("unpacked").join("inner.txt").exists(),
            "the contents of a stashed directory survive too"
        );

        std::fs::remove_dir_all(root.path()).ok();
    }

    async fn dir_entries(dir: &Path) -> Vec<String> {
        let mut names = Vec::new();
        let Ok(mut entries) = polyio::read_dir(dir).await else {
            return names;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        names
    }

    #[test]
    fn mods_are_swapped_only_while_they_still_live_in_the_game_dir() {
        assert!(
            !swap_types(true).contains(&ContentType::Mod),
            "redirected mods are not the game dir's to swap"
        );
        assert!(
            swap_types(false).contains(&ContentType::Mod),
            "un-redirected mods still have to leave the shared dir on exit"
        );
    }

    #[test]
    fn global_content_is_never_swapped() {
        for types in [swap_types(true), swap_types(false)] {
            for content_type in GLOBAL_TYPES {
                assert!(
                    !types.contains(&content_type),
                    "{content_type:?} is shared and must survive a session"
                );
            }
        }

        for content_type in GLOBAL_TYPES {
            assert!(content_type.is_global());
        }
        assert!(!ContentType::Mod.is_global());
    }

    #[test]
    fn the_game_dir_manifest_stops_claiming_packs() {
        let manifest = manifest_of(
            1,
            &[
                "mods/sodium.jar",
                "resourcepacks/faithful.zip",
                "shaderpacks/bsl.zip",
            ],
        );

        let stripped = without_global_entries(manifest);
        let paths = stripped.paths();

        assert!(paths.contains("mods/sodium.jar"));
        assert!(!paths.contains("resourcepacks/faithful.zip"));
        assert!(!paths.contains("shaderpacks/bsl.zip"));
    }

    #[test]
    fn the_global_notice_owns_up_to_its_reach() {
        let (_, body) = removal_notice(&["bsl.zip".into()]);

        assert!(body.contains("every cluster"), "{body}");
        assert!(!body.contains("'s folder"), "{body}");
    }

    async fn mods_scratch(name: &str, present: &[&str]) -> polyio::testing::ScratchDir {
        let root = polyio::testing::ScratchDir::new(name);
        polyio::create_dir_all(root.path()).await.unwrap();

        for file in present {
            polyio::write(root.join(file), b"jar".as_slice()).await.unwrap();
        }

        root
    }

    fn mods_manifest(files: &[&str]) -> MaterializedManifest {
        MaterializedManifest::new(
            1,
            files
                .iter()
                .map(|name| ManifestEntry {
                    path: manifest::entry_path(ContentType::Mod.folder_name(), name),
                    hash: format!("hash-{name}"),
                })
                .collect(),
        )
    }

    #[tokio::test]
    async fn a_jar_the_user_deleted_is_reported_with_its_hash() {
        let dir = mods_scratch("hand_removed", &["kept.jar", "also_kept.jar"]).await;
        let manifest = mods_manifest(&["kept.jar", "gone.jar", "also_kept.jar"]);

        let removed = hand_removed_content(dir.path(), ContentType::Mod, Some(&manifest)).await;

        assert_eq!(removed, vec![("gone.jar".into(), "hash-gone.jar".into())]);

        std::fs::remove_dir_all(dir.path()).ok();
    }

    #[tokio::test]
    async fn renaming_a_jar_out_of_the_way_counts_as_removing_it() {
        let dir = mods_scratch("renamed_away", &["sodium.jar.disabled", "other.jar"]).await;
        let manifest = mods_manifest(&["sodium.jar", "other.jar"]);

        let removed = hand_removed_content(dir.path(), ContentType::Mod, Some(&manifest)).await;

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].0, "sodium.jar");

        std::fs::remove_dir_all(dir.path()).ok();
    }

    #[tokio::test]
    async fn an_unreadable_folder_disables_nothing() {
        let manifest = mods_manifest(&["a.jar", "b.jar"]);
        let missing = Path::new("definitely-not-a-directory-ю");

        assert!(hand_removed_content(missing, ContentType::Mod, Some(&manifest)).await.is_empty());
    }

    #[tokio::test]
    async fn a_wholesale_disappearance_reads_as_a_folder_problem() {
        let dir = mods_scratch("all_gone", &[]).await;
        let manifest = mods_manifest(&["a.jar", "b.jar", "c.jar", "d.jar"]);

        assert!(
            hand_removed_content(dir.path(), ContentType::Mod, Some(&manifest)).await.is_empty(),
            "an empty folder where everything was is not four deliberate deletions"
        );

        std::fs::remove_dir_all(dir.path()).ok();
    }

    #[tokio::test]
    async fn clearing_a_short_list_is_still_taken_at_face_value() {
        let dir = mods_scratch("small_clear", &[]).await;
        let manifest = mods_manifest(&["a.jar", "b.jar"]);

        assert_eq!(hand_removed_content(dir.path(), ContentType::Mod, Some(&manifest)).await.len(), 2);

        std::fs::remove_dir_all(dir.path()).ok();
    }

    #[tokio::test]
    async fn a_first_launch_concludes_nothing() {
        let dir = mods_scratch("no_manifest", &[]).await;

        assert!(hand_removed_content(dir.path(), ContentType::Mod, None).await.is_empty());

        std::fs::remove_dir_all(dir.path()).ok();
    }

    #[test]
    fn notes_are_never_user_content() {
        assert!(is_note(EMPTY_NOTE_NAME));
        assert!(is_note(LINKS_NOTE_NAME));
        assert!(!is_note("sodium.jar"));
    }

    #[tokio::test]
    async fn stale_notes_go_but_the_folder_is_left_alone() {
        let root = polyio::testing::ScratchDir::new("stale_notes");
        let dir = root.path();

        for folder in ["mods", "resourcepacks", "shaderpacks"] {
            let sub = dir.join(folder);
            polyio::create_dir_all(&sub).await.unwrap();
            polyio::write(sub.join(EMPTY_NOTE_NAME), b"stale".as_slice())
                .await
                .unwrap();
            polyio::write(sub.join("keep.jar"), b"jar".as_slice())
                .await
                .unwrap();
        }

        drop_stale_notes(&[dir], false).await;

        for folder in ["mods", "resourcepacks", "shaderpacks"] {
            let sub = dir.join(folder);
            assert!(!sub.join(EMPTY_NOTE_NAME).exists(), "{folder}");
            assert!(sub.join("keep.jar").exists(), "{folder}");
        }

        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn a_swapped_mods_folder_keeps_its_note() {
        let root = polyio::testing::ScratchDir::new("swapped_note");
        let dir = root.path();

        let mods = dir.join("mods");
        let packs = dir.join("resourcepacks");
        polyio::create_dir_all(&mods).await.unwrap();
        polyio::create_dir_all(&packs).await.unwrap();
        polyio::write(mods.join(EMPTY_NOTE_NAME), b"stale".as_slice())
            .await
            .unwrap();
        polyio::write(packs.join(EMPTY_NOTE_NAME), b"stale".as_slice())
            .await
            .unwrap();

        drop_stale_notes(&[dir], true).await;

        assert!(mods.join(EMPTY_NOTE_NAME).exists());
        assert!(!packs.join(EMPTY_NOTE_NAME).exists());

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn ownership_spans_the_manifest_and_the_database() {
        let manifest = manifest_of(1, &["mods/from_manifest.jar", "shaderpacks/bsl.zip"]);
        let linked = Vec::new();

        let ours = ours_in_folder(ContentType::Mod, &linked, Some(&manifest));

        assert!(ours.contains("from_manifest.jar"));
        assert!(!ours.contains("bsl.zip"));
    }
}
