use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use oneclient_cluster::naming::validate_tag;
use oneclient_cluster::{Cluster, ClusterUpdate, CreateClusterOptions, ProfileUpdate};
use oneclient_common::Resolution;
use oneclient_common::domain::ContentType;
use oneclient_common::patch::Patch;
use oneclient_content::ContentCtx;
use oneclient_content::bundles::{remove_artifact_from_cluster, set_artifact_enabled_to};
use oneclient_content::modpacks::{BlockedFile, MODPACK_BUNDLE_NAME};
use oneclient_content::packages::store::cache_local_file;
use oneclient_content::packages::{PackageStore, ProviderId, curseforge_fingerprint, pick_version};
use oneclient_db::dao::{cluster as cluster_dao, cluster_bundle as bundle_dao};
use oneclient_events::GroupedProgressSession;

use crate::LauncherResult;
use crate::clusters::modpack::{
    ModpackSource, PreparedModpack, create_modpack_instance, install_modpack_instance,
    instance_name, kind_for, prepare_modpack, unique_name,
};
use crate::state::LauncherState;

use super::{ExternalInstance, ExternalSettings, LinkedPack};

/// Content the package store manages; copied through the store rather than
/// as plain files so it shows up (and updates) like anything else installed
const CONTENT_FOLDERS: &[ContentType] = &[
    ContentType::Mod,
    ContentType::ResourcePack,
    ContentType::Shader,
    ContentType::DataPack,
];

/// Never worth carrying over: the launcher rebuilds or redownloads all of it
const COPY_EXCLUDE_TOP: &[&str] = &[
    "mods",
    "resourcepacks",
    "shaderpacks",
    "datapacks",
    "logs",
    "crash-reports",
    "versions",
    "libraries",
    "assets",
    "natives",
    "bin",
    ".fabric",
    ".cache",
    ".mixin.out",
    "downloads",
];

/// Launcher bookkeeping inside the mods folder, meaningless to OneClient
const MODS_SKIP: &[&str] = &[".index", ".connector"];

/// Held open by a running game on Windows and recreated on every world load
const COPY_SKIP_FILES: &[&str] = &["session.lock"];

const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, Default)]
pub struct ExternalImportReport {
    pub cluster_id: i64,
    pub cluster_name: String,
    pub content_imported: usize,
    pub content_failed: Vec<String>,
    /// Mods found in an instance without a mod loader
    pub mods_skipped: usize,
    pub mods_left_out: usize,
    pub linked_pack: bool,
    pub blocked: Vec<BlockedFile>,
    /// Paths (relative to the source game dir) that could not be read or
    /// copied; the rest of the instance came over regardless
    pub files_skipped: Vec<String>,
    /// Set when the instance was pinned to the newer Java it ran on before,
    /// so its JVM arguments keep working
    pub java_pinned: Option<u32>,
    /// Set when that Java could not be set up and the JVM arguments, written
    /// for it, were left out rather than stop the game from starting
    pub jvm_args_dropped_for: Option<u32>,
}

/// What the import does about Java, decided before any settings are written
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JavaPlan {
    /// OneClient's usual choice works with what the instance recorded
    Default,
    /// The instance ran on a newer Java than OneClient would pick; its JVM
    /// arguments may hold flags the older one refuses to start with
    Pin(u32),
}

fn java_plan(source: Option<u32>, required: Option<u32>) -> JavaPlan {
    match (source, required) {
        // An older Java than the version requires cannot have run it, so
        // whatever was recorded is not worth reproducing
        (Some(source), Some(required)) if source > required => JavaPlan::Pin(source),
        _ => JavaPlan::Default,
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImportChoices {
    pub skip: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(super) struct LocalContent {
    pub(super) path: PathBuf,
    /// What the store sees: `path` itself, or for a disabled file a staged
    /// copy under its real name. The store names an artifact after the file
    /// it was given and every later link reuses that name, so a cached
    /// `x.jar.disabled` would be handed to other instances as enabled but
    /// never loaded
    import_path: PathBuf,
    content_type: ContentType,
    pub(super) enabled: bool,
    pub(super) hash: String,
    pub(super) cf_fingerprint: Option<u32>,
}

#[derive(Debug, Default)]
pub(super) struct ScannedContent {
    pub(super) files: Vec<LocalContent>,
    /// Folder packs, shader option files and the like, copied verbatim
    /// to the same relative path
    loose: Vec<PathBuf>,
    /// Content that could not be read, relative to the game dir
    unreadable: Vec<String>,
}

/// Creates a new isolated cluster from a third-party instance
///
/// Nothing in the source folder is modified, so the other launcher keeps
/// working. A failure after the cluster exists removes it again rather than
/// leaving a half-imported instance behind
#[tracing::instrument(skip(state, instance, progress), fields(name = %instance.name, launcher = instance.launcher.id()))]
pub async fn import_instance(
    state: &Arc<LauncherState>,
    instance: &ExternalInstance,
    choices: &ImportChoices,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ExternalImportReport> {
    let content = state.services.content();
    let staging = polyio::tempdir().await?;
    let mut scanned = scan_content(&instance.game_dir).await;
    let found = scanned.files.len();
    scanned
        .files
        .retain(|file| !choices.skip.contains(&file.hash));
    let left_out = found - scanned.files.len();
    stage_disabled(&mut scanned, staging.dir_path()).await;

    let taken: Vec<String> = state
        .clusters
        .list()
        .await?
        .into_iter()
        .map(|cluster| cluster.name)
        .collect();
    let name = unique_name(&instance_name(&instance.name), &taken);

    let prepared = match &instance.linked_pack {
        Some(pack) => prepare_linked_pack(state, instance, pack, &name).await,
        None => None,
    };

    // Warm the store first so the modpack install below finds these files
    // by hash instead of downloading them again
    if prepared.is_some() {
        for file in &scanned.files {
            if let Err(err) = cache_local_file(&file.import_path, file.content_type, &content).await
            {
                tracing::debug!(file = %file.path.display(), error = %err, "could not pre-cache file");
            }
        }
    }

    let cluster = match &prepared {
        Some(prepared) => create_modpack_instance(state, prepared).await?,
        None => create_plain(state, instance, &name).await?,
    };

    let result = populate(
        state,
        instance,
        &cluster,
        prepared.as_ref(),
        &scanned,
        choices,
        progress,
    )
    .await;
    match result {
        Ok(mut report) => {
            report.cluster_id = cluster.id;
            report.mods_left_out = left_out;
            report.cluster_name = cluster.name.clone();
            tracing::info!(
                cluster_id = cluster.id,
                imported = report.content_imported,
                failed = report.content_failed.len(),
                linked = report.linked_pack,
                "imported third-party instance"
            );
            Ok(report)
        }
        Err(err) => {
            tracing::warn!(cluster_id = cluster.id, error = %err, "import failed, removing the new cluster");
            if let Err(cleanup) = state.clusters.delete(cluster.id, true).await {
                tracing::warn!(cluster_id = cluster.id, error = %cleanup, "could not remove the half-imported cluster");
            }
            Err(err)
        }
    }
}

/// `None` means "import it as a plain instance": the pack being gone, the
/// provider being unreachable or the instance having drifted to another
/// version or loader are all reasons to keep the files rather than fail
async fn prepare_linked_pack(
    state: &Arc<LauncherState>,
    instance: &ExternalInstance,
    pack: &LinkedPack,
    name: &str,
) -> Option<PreparedModpack> {
    let source = ModpackSource::Provider {
        provider: pack.provider,
        project_id: pack.project_id.clone(),
        version_id: pack.version_id.clone(),
    };

    match prepare_modpack(state, &source).await {
        Ok(mut prepared)
            if prepared.manifest.mc_version == instance.mc_version
                && prepared.manifest.loader == instance.loader =>
        {
            prepared.instance_name = name.to_string();
            Some(prepared)
        }
        Ok(prepared) => {
            tracing::info!(
                pack_version = %prepared.manifest.mc_version,
                instance_version = %instance.mc_version,
                "linked modpack no longer matches the instance, importing unlinked"
            );
            None
        }
        Err(err) => {
            tracing::info!(error = %err, "could not fetch the linked modpack, importing unlinked");
            None
        }
    }
}

async fn create_plain(
    state: &Arc<LauncherState>,
    instance: &ExternalInstance,
    name: &str,
) -> LauncherResult<Cluster> {
    let global = state.settings.read().global_game_settings.clone();

    // `modpack` lifts the short-name limit imported names routinely exceed
    let mut options = CreateClusterOptions::new(name, &instance.mc_version, instance.loader)
        .kind(kind_for(instance.loader))
        .user_created(true)
        .modpack(true)
        .tags(valid_tags(&instance.groups));
    options.mc_loader_version = instance.loader_version.clone();
    options.description = instance.notes.clone().filter(|n| !n.trim().is_empty());

    Ok(state.clusters.create(&global, options).await?)
}

async fn populate(
    state: &Arc<LauncherState>,
    instance: &ExternalInstance,
    cluster: &Cluster,
    prepared: Option<&PreparedModpack>,
    scanned: &ScannedContent,
    choices: &ImportChoices,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ExternalImportReport> {
    let content = state.services.content();
    let mut report = ExternalImportReport::default();

    if let Some(prepared) = prepared {
        let installed = install_modpack_instance(state, cluster.id, prepared, progress).await?;
        report.linked_pack = true;
        report.blocked = installed.report.blocked;
        report.content_failed.extend(installed.report.failed);
        apply_linked_metadata(state, instance, cluster).await;
        drop_pack_files_removed_locally(cluster.id, scanned, &content).await?;
        drop_skipped_pack_files(cluster.id, &choices.skip, &content).await?;
    }

    report
        .files_skipped
        .extend(scanned.unreadable.iter().cloned());

    let game_dir = cluster.game_dir()?;
    polyio::create_dir_all(&game_dir).await?;
    if instance.game_dir.is_dir() {
        let source = instance.game_dir.as_path();
        copy_tree(
            source,
            &game_dir,
            source,
            COPY_EXCLUDE_TOP,
            &mut report.files_skipped,
        )
        .await;
        for relative in &scanned.loose {
            copy_tree(
                &source.join(relative),
                &game_dir.join(relative),
                source,
                &[],
                &mut report.files_skipped,
            )
            .await;
        }
    }

    import_content(cluster, scanned, &mut report, &content).await?;

    let mut settings = instance.settings.clone();
    let java_path = settle_java(state, cluster.id, &mut settings, &mut report, progress).await;
    apply_settings(state, cluster.id, &settings, java_path).await;

    if prepared.is_none()
        && let Some(icon) = &instance.icon
        && let Err(err) = state.clusters.set_cover_from_file(cluster.id, icon).await
    {
        tracing::debug!(error = %err, "could not use the instance icon as a cover");
    }

    if instance.played_secs > 0
        && let Err(err) = state
            .clusters
            .add_playtime(cluster.id, Duration::from_secs(instance.played_secs))
            .await
    {
        tracing::debug!(error = %err, "could not carry over play time");
    }

    Ok(report)
}

/// The modpack path names the cluster after the pack and ignores the source
/// instance's groups and loader pin; the user's own choices win
async fn apply_linked_metadata(
    state: &Arc<LauncherState>,
    instance: &ExternalInstance,
    cluster: &Cluster,
) {
    let tags = valid_tags(&instance.groups);
    let mut update = ClusterUpdate::default();
    if !tags.is_empty() {
        update.tags = Some(tags);
    }
    if let Some(version) = &instance.loader_version
        && cluster.mc_loader_version.as_ref() != Some(version)
    {
        update.mc_loader_version = Patch::Set(version.clone());
    }
    if update.tags.is_none() && update.mc_loader_version.is_unchanged() {
        return;
    }
    if let Err(err) = state.clusters.update(cluster.id, update).await {
        tracing::debug!(error = %err, "could not apply the instance's groups to the modpack cluster");
    }
}

/// The pack install puts back every file the pack lists, including ones the
/// user had deleted or swapped for another version in the other launcher
///
/// Those are taken out again with a `Removed` override, the same record the
/// UI leaves, so the next pack update respects the choice too. A swapped
/// version then arrives as the user's own file through the normal import
async fn drop_pack_files_removed_locally(
    cluster_id: i64,
    scanned: &ScannedContent,
    content: &ContentCtx,
) -> LauncherResult<()> {
    // An instance with no content at all was never installed, and a file
    // that could not be read might be one of these; neither says "removed"
    if scanned.files.is_empty() || !scanned.unreadable.is_empty() {
        return Ok(());
    }

    let local: HashSet<&str> = scanned.files.iter().map(|f| f.hash.as_str()).collect();
    let content_types: HashMap<String, ContentType> =
        PackageStore::list_linked_artifacts(cluster_id, content)
            .await?
            .into_iter()
            .map(|artifact| (artifact.hash, artifact.content_type))
            .collect();

    let tracked = bundle_dao::list_bundle_tracked(&content.db, cluster_id).await?;
    for row in tracked {
        if row.bundle_name.as_deref() != Some(MODPACK_BUNDLE_NAME)
            || local.contains(row.hash.as_str())
            || !content_types
                .get(&row.hash)
                .is_some_and(|ty| CONTENT_FOLDERS.contains(ty))
        {
            continue;
        }

        tracing::debug!(file = %row.cluster_file_name, "pack file was removed in the source instance");
        if let Err(err) = remove_artifact_from_cluster(cluster_id, &row.hash, true, content).await {
            tracing::debug!(file = %row.cluster_file_name, error = %err, "could not drop a removed pack file");
        }
    }

    Ok(())
}

async fn drop_skipped_pack_files(
    cluster_id: i64,
    skip: &HashSet<String>,
    content: &ContentCtx,
) -> LauncherResult<()> {
    if skip.is_empty() {
        return Ok(());
    }

    for artifact in PackageStore::list_linked_artifacts(cluster_id, content).await? {
        if !skip.contains(&polyio::normalize_hash(&artifact.hash)) {
            continue;
        }
        if let Err(err) =
            remove_artifact_from_cluster(cluster_id, &artifact.hash, true, content).await
        {
            tracing::debug!(file = %artifact.cluster_file_name, error = %err, "could not leave out a pack file");
        }
    }

    Ok(())
}

pub async fn alternative_version_id(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    provider: ProviderId,
    project_id: &str,
) -> LauncherResult<Option<String>> {
    let Some(row) = cluster_dao::get_by_id(&state.services.db, cluster_id).await? else {
        return Ok(None);
    };
    let content = state.services.content();
    let provider = content.providers.get(provider)?;
    let picked = pick_version(provider, project_id, &row, &content).await?;
    Ok(picked.map(|pick| pick.version_id))
}

async fn import_content(
    cluster: &Cluster,
    scanned: &ScannedContent,
    report: &mut ExternalImportReport,
    content: &ContentCtx,
) -> LauncherResult<()> {
    let linked: HashSet<String> = PackageStore::list_linked_artifacts(cluster.id, content)
        .await?
        .into_iter()
        .map(|artifact| artifact.hash)
        .collect();

    // Disabled files arrive staged under their real name (see
    // `LocalContent::import_path`) and are switched off after linking
    let mut to_import = Vec::new();
    let mut seen = HashSet::new();
    for file in &scanned.files {
        if file.content_type == ContentType::Mod && cluster.lacks_mod_loader() {
            report.mods_skipped += 1;
            continue;
        }
        if linked.contains(&file.hash) || !seen.insert(file.hash.clone()) {
            continue;
        }

        to_import.push((file.import_path.clone(), file.content_type));
    }

    if !to_import.is_empty() {
        let imported = PackageStore::import_local_files(&to_import, cluster.id, content).await?;
        report.content_imported += imported.imported.len();
        report
            .content_failed
            .extend(imported.failed.into_iter().map(|(path, _)| {
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            }));
    }

    for file in scanned.files.iter().filter(|f| !f.enabled) {
        if file.content_type == ContentType::Mod && cluster.lacks_mod_loader() {
            continue;
        }
        // The UI's own path: for a pack file it also records the choice so
        // the next pack update does not switch it back on
        if let Err(err) = set_artifact_enabled_to(cluster.id, &file.hash, false, content).await {
            tracing::debug!(file = %file.path.display(), error = %err, "could not keep the file disabled");
        }
    }

    Ok(())
}

/// Returns the runtime to pin, if any; when the Java the arguments were
/// written for cannot be had, the arguments go instead
async fn settle_java(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    settings: &mut ExternalSettings,
    report: &mut ExternalImportReport,
    progress: Option<&GroupedProgressSession>,
) -> Option<String> {
    let required = if settings.java_major.is_some() {
        crate::clusters::required_java_major(state, cluster_id)
            .await
            .inspect_err(
                |err| tracing::debug!(error = %err, "could not tell which Java the version needs"),
            )
            .ok()
            .flatten()
    } else {
        None
    };

    let JavaPlan::Pin(major) = java_plan(settings.java_major, required) else {
        return None;
    };

    // Same flags a launch uses: look on the system first, ask before downloading
    match state.java.prepare(major, true, false, progress).await {
        Ok(runtime) => {
            tracing::info!(
                cluster_id,
                major,
                "pinned the Java the instance ran on before"
            );
            report.java_pinned = Some(major);
            Some(runtime.absolute_path)
        }
        Err(err) => {
            tracing::info!(cluster_id, major, error = %err, "could not set up the instance's Java");
            if settings.jvm_args.take().is_some() {
                report.jvm_args_dropped_for = Some(major);
            }
            None
        }
    }
}

async fn apply_settings(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    settings: &ExternalSettings,
    java_path: Option<String>,
) {
    if settings.is_empty() && java_path.is_none() {
        return;
    }
    let update = profile_update(settings, java_path);
    if let Err(err) = state.clusters.update_profile(cluster_id, update).await {
        tracing::warn!(cluster_id, error = %err, "could not carry over instance settings");
    }
}

fn profile_update(settings: &ExternalSettings, java_path: Option<String>) -> ProfileUpdate {
    let set = |value: &Option<String>| match value {
        Some(value) => Patch::Set(value.clone()),
        None => Patch::Unchanged,
    };

    // The launcher splits env on whitespace, so a value containing a space
    // cannot be expressed and is dropped rather than mangled
    let env: Vec<String> = settings
        .env
        .iter()
        .filter(|(key, value)| !key.contains(['=', ' ']) && !value.contains(char::is_whitespace))
        .map(|(key, value)| format!("{key}={value}"))
        .collect();

    ProfileUpdate {
        mem_max: settings.mem_max.map_or(Patch::Unchanged, Patch::Set),
        launch_args: set(&settings.jvm_args),
        launch_env: if env.is_empty() {
            Patch::Unchanged
        } else {
            Patch::Set(env.join(" "))
        },
        resolution: settings
            .resolution
            .map_or(Patch::Unchanged, |(width, height)| {
                Patch::Set(Resolution { width, height })
            }),
        force_fullscreen: settings.fullscreen.map_or(Patch::Unchanged, Patch::Set),
        hook_pre: set(&settings.hook_pre),
        hook_wrapper: set(&settings.hook_wrapper),
        hook_post: set(&settings.hook_post),
        java_path: java_path.map_or(Patch::Unchanged, Patch::Set),
        ..ProfileUpdate::default()
    }
}

fn valid_tags(groups: &[String]) -> Vec<String> {
    groups
        .iter()
        .map(|g| g.trim().to_string())
        .filter(|g| validate_tag(g).is_ok())
        .collect()
}

fn is_archive(content_type: ContentType, name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let name = name.strip_suffix(DISABLED_SUFFIX).unwrap_or(&name);
    match content_type {
        ContentType::Mod => name.ends_with(".jar") || name.ends_with(".zip"),
        _ => name.ends_with(".zip"),
    }
}

/// Never fails: a file that cannot be read is listed in `unreadable` so one
/// bad jar does not stop the rest of the instance from coming over
async fn scan_content(game_dir: &Path) -> ScannedContent {
    scan_folders(game_dir, CONTENT_FOLDERS, false).await
}

pub(super) async fn scan_folders(
    game_dir: &Path,
    folders: &[ContentType],
    fingerprint: bool,
) -> ScannedContent {
    let mut scanned = ScannedContent::default();

    for content_type in folders.iter().copied() {
        let folder = game_dir.join(content_type.folder_name());
        if !folder.is_dir() {
            continue;
        }

        let mut entries = match polyio::read_dir(&folder).await {
            Ok(entries) => entries,
            Err(err) => {
                tracing::debug!(folder = %folder.display(), error = %err, "could not list content folder");
                scanned
                    .unreadable
                    .push(content_type.folder_name().to_string());
                continue;
            }
        };
        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(err) => {
                    tracing::debug!(folder = %folder.display(), error = %err, "could not list content folder");
                    scanned
                        .unreadable
                        .push(content_type.folder_name().to_string());
                    break;
                }
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let relative = format!("{}/{name}", content_type.folder_name());

            if content_type == ContentType::Mod
                && MODS_SKIP
                    .iter()
                    .any(|skip| skip.eq_ignore_ascii_case(&name))
            {
                continue;
            }

            // Follows links: a linked jar or pack folder is the user's real content
            let metadata = match tokio::fs::metadata(&path).await {
                Ok(metadata) => metadata,
                Err(err) => {
                    tracing::debug!(path = %path.display(), error = %err, "could not read content entry");
                    scanned.unreadable.push(relative);
                    continue;
                }
            };

            if metadata.is_file() && is_archive(content_type, &name) {
                match hash_file(&path, fingerprint).await {
                    Ok((hash, cf_fingerprint)) => scanned.files.push(LocalContent {
                        enabled: !name.to_ascii_lowercase().ends_with(DISABLED_SUFFIX),
                        hash: polyio::normalize_hash(&hash),
                        cf_fingerprint,
                        import_path: path.clone(),
                        path,
                        content_type,
                    }),
                    Err(err) => {
                        tracing::debug!(path = %path.display(), error = %err, "could not hash content file");
                        scanned.unreadable.push(relative);
                    }
                }
            } else if metadata.is_file() || metadata.is_dir() {
                scanned
                    .loose
                    .push(PathBuf::from(content_type.folder_name()).join(&name));
            }
        }
    }

    scanned
}

/// Gives every disabled file a copy under its real name for the store; one
/// that cannot be copied is moved to `unreadable` rather than imported under
/// the wrong name
async fn hash_file(
    path: &Path,
    fingerprint: bool,
) -> Result<(String, Option<u32>), polyio::IOError> {
    if !fingerprint {
        return Ok((polyio::sha1_file(path).await?, None));
    }
    let bytes = tokio::fs::read(path).await?;
    Ok((
        polyio::sha1_bytes(&bytes),
        Some(curseforge_fingerprint(&bytes)),
    ))
}

async fn stage_disabled(scanned: &mut ScannedContent, staging: &Path) {
    let mut kept = Vec::with_capacity(scanned.files.len());
    for mut file in std::mem::take(&mut scanned.files) {
        if file.enabled {
            kept.push(file);
            continue;
        }

        let file_name = file
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let real_name = strip_disabled(&file_name);
        // Per-hash folders keep two disabled copies of one name apart
        let staged = staging.join(&file.hash).join(real_name);
        match copy_file(&file.path, &staged).await {
            Ok(()) => {
                file.import_path = staged;
                kept.push(file);
            }
            Err(err) => {
                tracing::debug!(file = %file.path.display(), error = %err, "could not stage a disabled file");
                scanned
                    .unreadable
                    .push(format!("{}/{file_name}", file.content_type.folder_name()));
            }
        }
    }
    scanned.files = kept;
}

/// Case-insensitive like the scan that decided the file was disabled
fn strip_disabled(name: &str) -> &str {
    let cut = name.len().saturating_sub(DISABLED_SUFFIX.len());
    match name.get(cut..) {
        Some(tail) if tail.eq_ignore_ascii_case(DISABLED_SUFFIX) && cut > 0 => &name[..cut],
        _ => name,
    }
}

/// Copies `src` into `dst`, carrying on past anything it cannot read or
/// write and listing it (relative to `display_root`) in `skipped`
///
/// Links are followed, since a linked `saves` or `screenshots` folder holds
/// the user's real files; a folder already visited is not entered twice, so
/// a link pointing back up the tree cannot loop
async fn copy_tree(
    src: &Path,
    dst: &Path,
    display_root: &Path,
    exclude_top: &[&str],
    skipped: &mut Vec<String>,
) {
    let shown = |path: &Path| {
        path.strip_prefix(display_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    };

    match tokio::fs::metadata(src).await {
        Ok(metadata) if metadata.is_file() => {
            if let Err(err) = copy_file(src, dst).await {
                tracing::debug!(path = %src.display(), error = %err, "could not copy file");
                skipped.push(shown(src));
            }
            return;
        }
        Ok(_) => {}
        Err(err) => {
            tracing::debug!(path = %src.display(), error = %err, "could not read path to copy");
            skipped.push(shown(src));
            return;
        }
    }

    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut stack = vec![(src.to_path_buf(), dst.to_path_buf(), true)];
    while let Some((cur_src, cur_dst, top)) = stack.pop() {
        if let Ok(real) = tokio::fs::canonicalize(&cur_src).await
            && !visited.insert(real)
        {
            continue;
        }

        let listed = async {
            polyio::create_dir_all(&cur_dst).await?;
            polyio::read_dir(&cur_src).await
        };
        let mut entries = match listed.await {
            Ok(entries) => entries,
            Err(err) => {
                tracing::debug!(path = %cur_src.display(), error = %err, "could not copy folder");
                skipped.push(shown(&cur_src));
                continue;
            }
        };

        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(err) => {
                    tracing::debug!(path = %cur_src.display(), error = %err, "could not list folder");
                    skipped.push(shown(&cur_src));
                    break;
                }
            };
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if top
                && exclude_top
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(&name_str))
            {
                continue;
            }
            if COPY_SKIP_FILES
                .iter()
                .any(|skip| skip.eq_ignore_ascii_case(&name_str))
            {
                continue;
            }

            let child_src = entry.path();
            let child_dst = cur_dst.join(&name);
            match tokio::fs::metadata(&child_src).await {
                Ok(metadata) if metadata.is_dir() => stack.push((child_src, child_dst, false)),
                Ok(metadata) if metadata.is_file() => {
                    if let Err(err) = copy_file(&child_src, &child_dst).await {
                        tracing::debug!(path = %child_src.display(), error = %err, "could not copy file");
                        skipped.push(shown(&child_src));
                    }
                }
                // Sockets, pipes and the like have nothing to carry over
                Ok(_) => {}
                Err(err) => {
                    tracing::debug!(path = %child_src.display(), error = %err, "could not read path to copy");
                    skipped.push(shown(&child_src));
                }
            }
        }
    }
}

async fn copy_file(src: &Path, dst: &Path) -> Result<(), polyio::IOError> {
    if let Some(parent) = dst.parent() {
        polyio::create_dir_all(parent).await?;
    }
    polyio::copy(src, dst).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn touch(path: PathBuf) {
        polyio::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        polyio::write(&path, path.to_string_lossy().as_bytes())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn scan_splits_store_content_from_loose_files() {
        let tmp = polyio::tempdir().await.unwrap();
        let root = tmp.dir_path();
        for file in [
            "mods/sodium.jar",
            "mods/old.jar.disabled",
            "mods/.index/sodium.pw.toml",
            "mods/notes.txt",
            "resourcepacks/faithful.zip",
            "resourcepacks/Folder Pack/pack.mcmeta",
            "shaderpacks/bsl.zip",
            "shaderpacks/bsl.zip.txt",
            "options.txt",
        ] {
            touch(root.join(file)).await;
        }

        let scanned = scan_content(root).await;

        let mut stored: Vec<(String, bool)> = scanned
            .files
            .iter()
            .map(|f| {
                (
                    f.path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    f.enabled,
                )
            })
            .collect();
        stored.sort();
        assert_eq!(
            stored,
            vec![
                ("mods/old.jar.disabled".to_string(), false),
                ("mods/sodium.jar".to_string(), true),
                ("resourcepacks/faithful.zip".to_string(), true),
                ("shaderpacks/bsl.zip".to_string(), true),
            ]
        );

        let mut loose: Vec<String> = scanned
            .loose
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        loose.sort();
        assert_eq!(
            loose,
            vec![
                "mods/notes.txt".to_string(),
                "resourcepacks/Folder Pack".to_string(),
                "shaderpacks/bsl.zip.txt".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn copy_tree_skips_excluded_and_lock_files() {
        let src = polyio::tempdir().await.unwrap();
        let dst = polyio::tempdir().await.unwrap();
        let (src, dst) = (src.dir_path(), dst.dir_path());
        for file in [
            "options.txt",
            "saves/World/level.dat",
            "saves/World/session.lock",
            "logs/latest.log",
        ] {
            touch(src.join(file)).await;
        }

        let mut skipped = Vec::new();
        copy_tree(src, dst, src, &["logs"], &mut skipped).await;

        assert!(skipped.is_empty(), "{skipped:?}");
        assert!(dst.join("options.txt").is_file());
        assert!(dst.join("saves/World/level.dat").is_file());
        assert!(!dst.join("saves/World/session.lock").exists());
        assert!(!dst.join("logs").exists());
    }

    #[tokio::test]
    async fn copy_tree_reports_a_missing_source() {
        let src = polyio::tempdir().await.unwrap();
        let dst = polyio::tempdir().await.unwrap();
        let mut skipped = Vec::new();
        copy_tree(
            &src.dir_path().join("resourcepacks/Gone"),
            &dst.dir_path().join("resourcepacks/Gone"),
            src.dir_path(),
            &[],
            &mut skipped,
        )
        .await;
        assert_eq!(skipped, vec!["resourcepacks/Gone".to_string()]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn copy_tree_follows_links_without_looping() {
        let shared = polyio::tempdir().await.unwrap();
        let src = polyio::tempdir().await.unwrap();
        let dst = polyio::tempdir().await.unwrap();
        touch(shared.dir_path().join("World/level.dat")).await;
        std::os::unix::fs::symlink(shared.dir_path(), src.dir_path().join("saves")).unwrap();
        // Points back at the instance itself
        std::os::unix::fs::symlink(src.dir_path(), src.dir_path().join("loop")).unwrap();

        let mut skipped = Vec::new();
        copy_tree(
            src.dir_path(),
            dst.dir_path(),
            src.dir_path(),
            &[],
            &mut skipped,
        )
        .await;

        assert!(skipped.is_empty(), "{skipped:?}");
        assert!(dst.dir_path().join("saves/World/level.dat").is_file());
    }

    #[tokio::test]
    async fn disabled_files_reach_the_store_under_their_real_name() {
        let src = polyio::tempdir().await.unwrap();
        let staging = polyio::tempdir().await.unwrap();
        touch(src.dir_path().join("mods/old.jar.DISABLED")).await;
        touch(src.dir_path().join("mods/live.jar")).await;

        let mut scanned = scan_content(src.dir_path()).await;
        stage_disabled(&mut scanned, staging.dir_path()).await;

        assert!(scanned.unreadable.is_empty());
        for file in &scanned.files {
            let name = file.import_path.file_name().unwrap().to_string_lossy();
            assert!(!name.to_ascii_lowercase().ends_with(".disabled"), "{name}");
            assert!(file.import_path.is_file());
        }
        let disabled = scanned.files.iter().find(|f| !f.enabled).unwrap();
        assert!(disabled.import_path.ends_with("old.jar"));
    }

    #[test]
    fn strip_disabled_only_strips_the_suffix() {
        assert_eq!(strip_disabled("a.jar.disabled"), "a.jar");
        assert_eq!(strip_disabled("a.jar.Disabled"), "a.jar");
        assert_eq!(strip_disabled("a.jar"), "a.jar");
        assert_eq!(strip_disabled(".disabled"), ".disabled");
    }

    #[test]
    fn only_a_newer_recorded_java_is_pinned() {
        assert_eq!(java_plan(Some(21), Some(17)), JavaPlan::Pin(21));
        assert_eq!(java_plan(Some(17), Some(17)), JavaPlan::Default);
        assert_eq!(java_plan(Some(8), Some(17)), JavaPlan::Default);
        assert_eq!(java_plan(None, Some(17)), JavaPlan::Default);
        assert_eq!(java_plan(Some(21), None), JavaPlan::Default);
    }

    #[test]
    fn a_pinned_runtime_lands_in_the_profile() {
        let update = profile_update(&ExternalSettings::default(), Some("/jdk21/bin/java".into()));
        assert_eq!(update.java_path, Patch::Set("/jdk21/bin/java".to_string()));
        assert!(
            profile_update(&ExternalSettings::default(), None)
                .java_path
                .is_unchanged()
        );
    }

    #[test]
    fn env_values_with_spaces_are_dropped() {
        let settings = ExternalSettings {
            env: vec![
                ("GOOD".into(), "1".into()),
                ("BAD".into(), "two words".into()),
            ],
            mem_max: Some(4096),
            ..Default::default()
        };
        let update = profile_update(&settings, None);
        assert_eq!(update.launch_env, Patch::Set("GOOD=1".to_string()));
        assert_eq!(update.mem_max, Patch::Set(4096));
        assert!(update.launch_args.is_unchanged());
    }

    #[test]
    fn overlong_groups_are_not_tags() {
        let tags = valid_tags(&[
            "Modded".into(),
            "  ".into(),
            "A group name far too long to be a tag".into(),
        ]);
        assert_eq!(tags, vec!["Modded".to_string()]);
    }
}
