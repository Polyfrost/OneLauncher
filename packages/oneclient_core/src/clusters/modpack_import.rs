use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use oneclient_cluster::{Cluster, ClusterUpdate};
use oneclient_common::domain::ProviderId;
use oneclient_common::patch::Patch;
use oneclient_common::paths;
use oneclient_content::modpacks::{self, ModpackInstallReport, ModpackRelease, WantedFile};
use oneclient_content::packages::store::artifact_absolute_path;
use oneclient_content::packages::{
    PackageStore, ResolvedAlternative, fetch_explanation, load_bad_mods, resolve_alternatives,
};
use oneclient_content::{ContentCtx, ContentError};
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::OverrideType;
use oneclient_events::GroupedProgressSession;

use super::modpack::{
    ModpackCluster, ModpackSource, PreparedModpack, fetch_project, prepare_modpack,
};
use crate::LauncherResult;
use crate::state::LauncherState;

const IMPORTED_MODPACKS_FILE: &str = ".oneclient/modpacks.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedModpack {
    pub bundle_name: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub provider: Option<ProviderId>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub disabled_hashes: Vec<String>,
    #[serde(default)]
    pub wanted: Vec<WantedFile>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlaggedPackMod {
    pub package_id: String,
    pub name: String,
    pub explanation: Option<String>,
    pub alternatives: Vec<ResolvedAlternative>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExistingPack {
    #[default]
    Replace,
    KeepBoth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderPlan {
    Keep,
    Raise { from: Option<String>, to: String },
    Behind { current: String, needed: String },
}

#[derive(Debug, Clone)]
pub struct PreparedImport {
    pub prepared: PreparedModpack,
    pub cluster_id: i64,
    pub cluster_name: String,
    pub bundle_name: String,
    pub release: Option<ModpackRelease>,
    pub project: Option<(ProviderId, String)>,
    pub previous: Option<ImportedModpack>,
    pub same_name: Option<ImportedModpack>,
    pub separate_bundle_name: String,
    pub loader: LoaderPlan,
    pub shared_game_dir: bool,
    pub flagged: Vec<FlaggedPackMod>,
}

impl PreparedImport {
    #[must_use]
    pub fn pack_name(&self) -> String {
        pack_name(&self.prepared, self.release.as_ref())
    }

    #[must_use]
    pub fn existing(&self) -> Option<&ImportedModpack> {
        match &self.previous {
            Some(previous) if modpacks::is_file_bundle(&self.bundle_name) => Some(previous),
            Some(_) => None,
            None => self.same_name.as_ref(),
        }
    }

    #[must_use]
    pub fn target_bundle_name(&self, existing: ExistingPack) -> &str {
        if existing == ExistingPack::KeepBoth && self.previous.is_some() && self.existing().is_some()
        {
            &self.separate_bundle_name
        } else {
            &self.bundle_name
        }
    }

    fn replaced_pack(&self, existing: ExistingPack) -> Option<&ImportedModpack> {
        match existing {
            ExistingPack::Replace if self.previous.is_none() => self.same_name.as_ref(),
            _ => None,
        }
    }
}

#[tracing::instrument(skip(state))]
pub async fn prepare_modpack_import(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    source: &ModpackSource,
) -> LauncherResult<PreparedImport> {
    let content = state.services.content();
    let cluster = state.clusters.get(cluster_id).await?;
    let prepared = prepare_modpack(state, source).await?;
    let manifest = &prepared.manifest;

    let release = match modpacks::identify_modpack(&prepared.artifact_hash, &content).await {
        Ok(release) => release,
        Err(err) => {
            tracing::debug!(error = %err, "could not identify the modpack on any provider");
            None
        }
    };

    if manifest.mc_version != cluster.mc_version || manifest.loader != cluster.mc_loader {
        return Err(invalid(format!(
            "{} needs Minecraft {} with {}, but {} uses Minecraft {} with {}.",
            pack_name(&prepared, release.as_ref()),
            manifest.mc_version,
            manifest.loader,
            cluster.name,
            cluster.mc_version,
            cluster.mc_loader
        )));
    }

    let project = release
        .as_ref()
        .map(|release| (release.provider, release.project_id.as_str()))
        .or(match source {
            ModpackSource::Provider {
                provider,
                project_id,
                ..
            } => Some((*provider, project_id.as_str())),
            ModpackSource::File(_) => None,
        });

    if is_primary_pack(&cluster, &prepared.artifact_hash, project, &content).await {
        return Err(invalid(format!(
            "{} was made from this modpack. Use Modpack Updates in its settings instead.",
            cluster.name
        )));
    }

    let bundle_name = modpacks::imported_bundle_name(project, &manifest.name);
    let project = project.map(|(provider, id)| (provider, id.to_string()));
    let packs = read_imported_strict(&cluster).await?;
    let previous = packs
        .iter()
        .find(|pack| pack.bundle_name == bundle_name)
        .cloned();
    let name = pack_name(&prepared, release.as_ref());
    let same_name = if previous.is_none() {
        packs
            .iter()
            .find(|pack| same_text(&pack.name, &name))
            .cloned()
    } else {
        None
    };
    let taken: HashSet<&str> = packs.iter().map(|pack| pack.bundle_name.as_str()).collect();
    let separate_bundle_name = modpacks::separate_bundle_name(&bundle_name, &taken);
    let loader = loader_plan(&cluster, manifest.loader_version.as_deref());
    let shared_game_dir = uses_shared_game_dir(&cluster);
    let flagged = screen(&prepared, cluster_id, &content).await;

    Ok(PreparedImport {
        cluster_id,
        cluster_name: cluster.name,
        bundle_name,
        release,
        project,
        previous,
        same_name,
        separate_bundle_name,
        loader,
        shared_game_dir,
        flagged,
        prepared,
    })
}

#[tracing::instrument(skip(state, import, skipped, progress), fields(pack = %import.prepared.manifest.name))]
pub async fn import_modpack_into_cluster(
    state: &Arc<LauncherState>,
    import: &PreparedImport,
    skipped: &HashSet<String>,
    existing: ExistingPack,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ModpackCluster> {
    let content = state.services.content();
    let cluster_id = import.cluster_id;
    let bundle_name = import.target_bundle_name(existing);
    let manifest = &import.prepared.manifest;
    let cluster = state.clusters.get(cluster_id).await?;
    refuse_while_shared_game_runs(state, &cluster).await?;

    let replaced = import.replaced_pack(existing);
    let previous = if bundle_name == import.bundle_name {
        import.previous.as_ref()
    } else {
        None
    };
    let mut entry = ImportedModpack {
        bundle_name: bundle_name.to_string(),
        name: import.pack_name(),
        version: previous.map_or_else(|| manifest.version.clone(), |pack| pack.version.clone()),
        provider: import.project.as_ref().map(|(provider, _)| *provider),
        project_id: import.project.as_ref().map(|(_, id)| id.clone()),
        icon_url: previous.or(replaced).and_then(|pack| pack.icon_url.clone()),
        disabled_hashes: previous
            .map(|pack| pack.disabled_hashes.clone())
            .unwrap_or_default(),
        wanted: previous.map(|pack| pack.wanted.clone()).unwrap_or_default(),
    };
    save_imported(&cluster, &entry).await?;

    apply_flagged_choices(import, bundle_name, skipped, &content).await?;

    let install = || {
        modpacks::install_modpack(
            &import.prepared.archive_path,
            manifest,
            cluster_id,
            bundle_name,
            progress,
            &content,
        )
    };
    let mut report = install().await?;
    record_install(&mut entry, &report);
    save_imported(&cluster, &entry).await?;

    if let Some(old) = replaced {
        let failed = remove_pack(state, &cluster, &old.bundle_name, &content).await?;
        if !failed.is_empty() {
            return Err(invalid(format!(
                "{} was added, but {} of the files from {} could not be removed. Close anything using them, then remove {} under Added Modpacks.",
                entry.name,
                failed.len(),
                old.name,
                old.name
            )));
        }
        let again = install().await?;
        record_install(&mut entry, &again);
        report.absorb(again);
    }

    if let LoaderPlan::Raise { to, .. } = &import.loader {
        let current = state.clusters.get(cluster_id).await?;
        let raises = current
            .mc_loader_version
            .as_deref()
            .is_none_or(|current| modpacks::is_newer_version(to, current));
        if raises {
            state
                .clusters
                .update(
                    cluster_id,
                    ClusterUpdate {
                        mc_loader_version: Patch::Set(to.clone()),
                        ..Default::default()
                    },
                )
                .await?;
        }
    }

    entry.version = manifest.version.clone();
    if let Some(release) = &import.release {
        match fetch_project(state, release).await {
            Ok(project) => {
                if let Some(url) = project.icon_url.filter(|url| !url.is_empty()) {
                    entry.icon_url = Some(url);
                }
            }
            Err(err) => {
                tracing::debug!(error = %err, "could not read the modpack's project page");
            }
        }
    }
    save_imported(&cluster, &entry).await?;

    let cluster = state.clusters.get(cluster_id).await?;
    Ok(ModpackCluster { cluster, report })
}

fn record_install(entry: &mut ImportedModpack, report: &ModpackInstallReport) {
    for hash in &report.disabled_hashes {
        if !entry.disabled_hashes.contains(hash) {
            entry.disabled_hashes.push(hash.clone());
        }
    }
    entry.wanted = report.wanted_present.clone();
}

#[tracing::instrument(skip(state))]
pub async fn list_imported_modpacks(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<Vec<ImportedModpack>> {
    let cluster = state.clusters.get(cluster_id).await?;
    Ok(read_imported(&cluster).await)
}

#[tracing::instrument(skip(state))]
pub async fn modpack_file_sources(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<HashMap<String, String>> {
    let content = state.services.content();
    let cluster = state.clusters.get(cluster_id).await?;
    let tracked = bundle_dao::list_bundle_tracked(&content.db, cluster_id).await?;

    let mut names: HashMap<String, String> = read_imported(&cluster)
        .await
        .into_iter()
        .map(|pack| (pack.bundle_name, pack.name))
        .collect();
    let has_primary = tracked
        .iter()
        .any(|row| row.bundle_name.as_deref() == Some(modpacks::MODPACK_BUNDLE_NAME));
    if has_primary {
        let release_name = match cluster.linked_modpack_hash.as_deref() {
            Some(hash) => artifact_dao::get_release_by_hash(&content.db, hash)
                .await
                .ok()
                .flatten()
                .map(|release| release.display_name)
                .filter(|name| !name.trim().is_empty()),
            None => None,
        };
        names.insert(
            modpacks::MODPACK_BUNDLE_NAME.to_string(),
            release_name.unwrap_or_else(|| cluster.name.clone()),
        );
    }

    Ok(tracked
        .into_iter()
        .filter_map(|row| {
            let name = names.get(row.bundle_name.as_deref()?)?;
            Some((row.hash, name.clone()))
        })
        .collect())
}

#[tracing::instrument(skip(state))]
pub async fn remove_imported_modpack(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    bundle_name: &str,
) -> LauncherResult<Vec<String>> {
    if !modpacks::is_imported_bundle(bundle_name) {
        return Err(invalid(
            "Only modpacks added to an existing instance can be removed.".to_string(),
        ));
    }

    let content = state.services.content();
    let cluster = state.clusters.get(cluster_id).await?;
    refuse_while_shared_game_runs(state, &cluster).await?;
    remove_pack(state, &cluster, bundle_name, &content).await
}

async fn remove_pack(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    bundle_name: &str,
    content: &ContentCtx,
) -> LauncherResult<Vec<String>> {
    let packs = read_imported_strict(cluster).await?;
    let removing = packs
        .iter()
        .find(|pack| pack.bundle_name == bundle_name)
        .cloned();
    hand_over_files(cluster, bundle_name, packs, content).await?;

    let hand_to = shared_elsewhere(state, cluster, bundle_name).await?;
    let failed = modpacks::remove_modpack_files(cluster.id, bundle_name, hand_to, content).await?;
    if !failed.is_empty() {
        return Ok(failed);
    }

    let mut packs = read_imported_strict(cluster).await?;
    packs.retain(|pack| pack.bundle_name != bundle_name);
    if let Some(removing) = removing {
        let still_off: HashSet<&String> = packs
            .iter()
            .flat_map(|pack| &pack.disabled_hashes)
            .collect();
        let restore: Vec<String> = removing
            .disabled_hashes
            .iter()
            .filter(|hash| !still_off.contains(hash))
            .cloned()
            .collect();
        modpacks::restore_disabled(cluster.id, &restore, content).await;
    }
    write_imported(cluster, &packs).await?;
    Ok(failed)
}

async fn hand_over_files(
    cluster: &Cluster,
    bundle_name: &str,
    mut packs: Vec<ImportedModpack>,
    content: &ContentCtx,
) -> LauncherResult<()> {
    let rows = bundle_dao::list_bundle_tracked(&content.db, cluster.id).await?;
    let primary_tracked: HashSet<String> = rows
        .iter()
        .filter(|row| row.bundle_name.as_deref() == Some(modpacks::MODPACK_BUNDLE_NAME))
        .filter_map(|row| row.package_id.clone())
        .collect();
    let leaving: Vec<_> = rows
        .into_iter()
        .filter(|row| row.bundle_name.as_deref() == Some(bundle_name))
        .collect();
    if leaving.is_empty() {
        return Ok(());
    }

    let primary: Vec<WantedFile> = primary_wanted(cluster, content)
        .await
        .into_iter()
        .filter(|wanted| !primary_tracked.contains(&wanted.package_id))
        .collect();
    let mut changed = false;

    for row in leaving {
        let matches = |wanted: &WantedFile| {
            wanted.sha1 == row.hash || row.package_id.as_deref() == Some(wanted.package_id.as_str())
        };
        let mut target = None;
        for pack in packs
            .iter_mut()
            .filter(|pack| pack.bundle_name != bundle_name)
        {
            if let Some(index) = pack.wanted.iter().position(|wanted| matches(wanted)) {
                target = Some((pack.bundle_name.clone(), pack.wanted.remove(index)));
                changed = true;
                break;
            }
        }
        if target.is_none() {
            target = primary
                .iter()
                .find(|wanted| matches(wanted))
                .map(|wanted| (modpacks::MODPACK_BUNDLE_NAME.to_string(), wanted.clone()));
        }
        if let Some((owner, wanted)) = target {
            modpacks::retrack_file(cluster.id, &row.hash, &owner, &wanted, content).await?;
            tracing::info!(hash = %row.hash, from = bundle_name, to = %owner, "handed a shared file to another pack");
        }
    }

    if changed {
        write_imported(cluster, &packs).await?;
    }
    Ok(())
}

async fn primary_wanted(cluster: &Cluster, content: &ContentCtx) -> Vec<WantedFile> {
    let Some(hash) = cluster.linked_modpack_hash.as_deref() else {
        return Vec::new();
    };
    let Ok(Some(artifact)) = artifact_dao::get_artifact_by_hash(&content.db, hash).await else {
        return Vec::new();
    };
    let Ok(path) = artifact_absolute_path(&artifact.path) else {
        return Vec::new();
    };
    match modpacks::read_modpack(&path, content).await {
        Ok(manifest) => modpacks::manifest_wanted(&manifest),
        Err(err) => {
            tracing::debug!(error = %err, "could not read the instance's own modpack");
            Vec::new()
        }
    }
}

async fn refuse_while_shared_game_runs(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<()> {
    if !uses_shared_game_dir(cluster) {
        return Ok(());
    }
    for other in state.clusters.list().await? {
        if other.id != cluster.id && uses_shared_game_dir(&other) && state.games.is_active(other.id)
        {
            return Err(invalid(format!(
                "Close Minecraft in {} first. It uses the same game folder as {}.",
                other.name, cluster.name
            )));
        }
    }
    Ok(())
}

async fn apply_flagged_choices(
    import: &PreparedImport,
    bundle_name: &str,
    skipped: &HashSet<String>,
    content: &ContentCtx,
) -> LauncherResult<()> {
    if import.flagged.is_empty() {
        return Ok(());
    }

    let db = &content.db;
    let removed: HashSet<String> = bundle_dao::list_overrides(db, import.cluster_id)
        .await?
        .into_iter()
        .filter(|row| row.bundle_name == bundle_name)
        .filter(|row| OverrideType::parse(&row.override_type) == Some(OverrideType::Removed))
        .map(|row| row.package_id)
        .collect();

    for flagged in &import.flagged {
        let package_id = flagged.package_id.as_str();
        if skipped.contains(package_id) {
            bundle_dao::save_override(
                db,
                import.cluster_id,
                bundle_name,
                package_id,
                OverrideType::Removed,
            )
            .await?;
        } else if removed.contains(package_id) {
            bundle_dao::remove_override(db, import.cluster_id, bundle_name, package_id).await?;
        }
    }

    modpacks::remove_tracked_packages(import.cluster_id, bundle_name, skipped, content).await?;
    Ok(())
}

fn loader_plan(cluster: &Cluster, needed: Option<&str>) -> LoaderPlan {
    let Some(needed) = needed.map(str::trim).filter(|version| !version.is_empty()) else {
        return LoaderPlan::Keep;
    };
    match cluster.mc_loader_version.as_deref() {
        None => LoaderPlan::Raise {
            from: None,
            to: needed.to_string(),
        },
        Some(current) if modpacks::is_newer_version(needed, current) => {
            if cluster.uses_bundles() {
                LoaderPlan::Behind {
                    current: current.to_string(),
                    needed: needed.to_string(),
                }
            } else {
                LoaderPlan::Raise {
                    from: Some(current.to_string()),
                    to: needed.to_string(),
                }
            }
        }
        Some(_) => LoaderPlan::Keep,
    }
}

fn uses_shared_game_dir(cluster: &Cluster) -> bool {
    !cluster.is_isolated() && !paths::cluster_uses_dedicated_dir(&cluster.folder_name)
}

async fn shared_elsewhere(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    bundle_name: &str,
) -> LauncherResult<Option<i64>> {
    if !uses_shared_game_dir(cluster) {
        return Ok(None);
    }
    for other in state.clusters.list().await? {
        if other.id == cluster.id || !uses_shared_game_dir(&other) {
            continue;
        }
        if read_imported(&other)
            .await
            .iter()
            .any(|pack| pack.bundle_name == bundle_name)
        {
            return Ok(Some(other.id));
        }
    }
    Ok(None)
}

pub(super) async fn has_imported_packs(cluster: &Cluster) -> bool {
    !read_imported(cluster).await.is_empty()
}

fn same_text(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

async fn screen(
    prepared: &PreparedModpack,
    cluster_id: i64,
    content: &ContentCtx,
) -> Vec<FlaggedPackMod> {
    let list = load_bad_mods(content).await;
    if list.bad_mods.is_empty() {
        return Vec::new();
    }

    let bundled = match modpacks::bundled_mods(&prepared.archive_path, &prepared.manifest).await {
        Ok(bundled) => bundled,
        Err(err) => {
            tracing::warn!(error = %err, "could not read the jars bundled in the modpack");
            Vec::new()
        }
    };
    let flagged = modpacks::screen_modpack(&list, &prepared.manifest, &bundled, content).await;
    if flagged.is_empty() {
        return Vec::new();
    }

    let cluster = match PackageStore::get_cluster(cluster_id, content).await {
        Ok(cluster) => Some(cluster),
        Err(err) => {
            tracing::warn!(cluster_id, error = %err, "cannot resolve alternatives without the cluster");
            None
        }
    };
    let cluster = cluster.as_ref();

    futures_util::future::join_all(flagged.into_iter().map(|file| async move {
        let alternatives = async {
            match cluster {
                Some(cluster) => resolve_alternatives(file.entry, cluster, content).await,
                None => Vec::new(),
            }
        };
        let (alternatives, explanation) =
            tokio::join!(alternatives, fetch_explanation(file.entry, content));
        FlaggedPackMod {
            package_id: file.package_id,
            name: file.name,
            explanation,
            alternatives,
        }
    }))
    .await
}

async fn is_primary_pack(
    cluster: &Cluster,
    artifact_hash: &str,
    project: Option<(ProviderId, &str)>,
    content: &ContentCtx,
) -> bool {
    let Some(linked) = cluster.linked_modpack_hash.as_deref() else {
        return false;
    };
    if linked == artifact_hash {
        return true;
    }
    let Some((provider, project_id)) = project else {
        return false;
    };
    match modpacks::cluster_modpack(cluster.id, content).await {
        Ok(Some(release)) => release.provider == provider && release.project_id == project_id,
        _ => false,
    }
}

fn pack_name(prepared: &PreparedModpack, release: Option<&ModpackRelease>) -> String {
    let name = prepared.manifest.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    release
        .map(|release| release.name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| prepared.instance_name.clone())
}

fn invalid(reason: String) -> crate::LauncherError {
    ContentError::InvalidData { reason }.into()
}

fn imported_file(cluster: &Cluster) -> Option<PathBuf> {
    Some(cluster.dir().ok()?.join(IMPORTED_MODPACKS_FILE))
}

async fn read_imported(cluster: &Cluster) -> Vec<ImportedModpack> {
    let Some(path) = imported_file(cluster) else {
        return Vec::new();
    };
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|err| {
        tracing::warn!(path = %path.display(), error = %err, "unreadable imported modpacks list");
        Vec::new()
    })
}

async fn read_imported_strict(cluster: &Cluster) -> LauncherResult<Vec<ImportedModpack>> {
    let path = cluster.dir()?.join(IMPORTED_MODPACKS_FILE);
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(invalid(format!(
                "Could not read the list of added modpacks ({err}). Try again in a moment."
            )));
        }
    };
    serde_json::from_slice(&bytes).map_err(|err| {
        invalid(format!(
            "The list of added modpacks at {} is damaged ({err}). Fix or delete it, then try again.",
            path.display()
        ))
    })
}

async fn save_imported(cluster: &Cluster, entry: &ImportedModpack) -> LauncherResult<()> {
    let mut packs = read_imported_strict(cluster).await?;
    match packs
        .iter_mut()
        .find(|pack| pack.bundle_name == entry.bundle_name)
    {
        Some(pack) => *pack = entry.clone(),
        None => packs.push(entry.clone()),
    }
    write_imported(cluster, &packs).await
}

async fn write_imported(cluster: &Cluster, packs: &[ImportedModpack]) -> LauncherResult<()> {
    let path = cluster.dir()?.join(IMPORTED_MODPACKS_FILE);
    if packs.is_empty() {
        if polyio::try_exists(&path).await.unwrap_or(false) {
            polyio::remove_file(&path).await?;
        }
        return Ok(());
    }
    polyio::write_atomic(&path, serde_json::to_vec_pretty(packs)?).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_imported_list_tolerates_missing_optional_fields() {
        let packs: Vec<ImportedModpack> = serde_json::from_str(
            r#"[{"bundle_name":"modpack:file:pack","name":"Pack","version":"1.0"}]"#,
        )
        .unwrap();
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].provider, None);
        assert_eq!(packs[0].icon_url, None);
    }
}
