use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use oneclient_cluster::{Cluster, ClusterUpdate};
use oneclient_common::domain::ProviderId;
use oneclient_common::patch::Patch;
use oneclient_content::modpacks::{self, ModpackRelease};
use oneclient_content::packages::{
    PackageStore, ResolvedAlternative, fetch_explanation, load_bad_mods, resolve_alternatives,
};
use oneclient_content::{ContentCtx, ContentError};
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlaggedPackMod {
    pub package_id: String,
    pub name: String,
    pub explanation: Option<String>,
    pub alternatives: Vec<ResolvedAlternative>,
}

#[derive(Debug, Clone)]
pub struct PreparedImport {
    pub prepared: PreparedModpack,
    pub cluster_id: i64,
    pub cluster_name: String,
    pub bundle_name: String,
    pub release: Option<ModpackRelease>,
    pub previous: Option<ImportedModpack>,
    pub flagged: Vec<FlaggedPackMod>,
}

impl PreparedImport {
    #[must_use]
    pub fn pack_name(&self) -> String {
        pack_name(&self.prepared, self.release.as_ref())
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
    let previous = read_imported(&cluster)
        .await
        .into_iter()
        .find(|pack| pack.bundle_name == bundle_name);
    let flagged = screen(&prepared, cluster_id, &content).await;

    Ok(PreparedImport {
        cluster_id,
        cluster_name: cluster.name,
        bundle_name,
        release,
        previous,
        flagged,
        prepared,
    })
}

#[tracing::instrument(skip(state, import, skipped, progress), fields(pack = %import.prepared.manifest.name))]
pub async fn import_modpack_into_cluster(
    state: &Arc<LauncherState>,
    import: &PreparedImport,
    skipped: &HashSet<String>,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ModpackCluster> {
    let content = state.services.content();
    let cluster_id = import.cluster_id;
    let bundle_name = import.bundle_name.as_str();
    let manifest = &import.prepared.manifest;

    apply_flagged_choices(import, skipped, &content).await?;

    let report = modpacks::install_modpack(
        &import.prepared.archive_path,
        manifest,
        cluster_id,
        bundle_name,
        progress,
        &content,
    )
    .await?;

    let cluster = state.clusters.get(cluster_id).await?;
    if cluster.mc_loader_version.is_none()
        && let Some(version) = &manifest.loader_version
    {
        state
            .clusters
            .update(
                cluster_id,
                ClusterUpdate {
                    mc_loader_version: Patch::Set(version.clone()),
                    ..Default::default()
                },
            )
            .await?;
    }

    let icon_url = match &import.release {
        Some(release) => match fetch_project(state, release).await {
            Ok(project) => project.icon_url.filter(|url| !url.is_empty()),
            Err(err) => {
                tracing::debug!(error = %err, "could not read the modpack's project page");
                None
            }
        },
        None => None,
    };
    let entry = ImportedModpack {
        bundle_name: bundle_name.to_string(),
        name: import.pack_name(),
        version: manifest.version.clone(),
        provider: import.release.as_ref().map(|release| release.provider),
        project_id: import
            .release
            .as_ref()
            .map(|release| release.project_id.clone()),
        icon_url: icon_url.or_else(|| {
            import
                .previous
                .as_ref()
                .and_then(|previous| previous.icon_url.clone())
        }),
    };

    let mut packs = read_imported(&cluster).await;
    packs.retain(|pack| pack.bundle_name != entry.bundle_name);
    packs.push(entry);
    write_imported(&cluster, &packs).await?;

    let cluster = state.clusters.get(cluster_id).await?;
    Ok(ModpackCluster { cluster, report })
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
    let failed = modpacks::remove_modpack_files(cluster_id, bundle_name, &content).await?;

    let cluster = state.clusters.get(cluster_id).await?;
    let mut packs = read_imported(&cluster).await;
    packs.retain(|pack| pack.bundle_name != bundle_name);
    write_imported(&cluster, &packs).await?;

    Ok(failed)
}

async fn apply_flagged_choices(
    import: &PreparedImport,
    skipped: &HashSet<String>,
    content: &ContentCtx,
) -> LauncherResult<()> {
    if import.flagged.is_empty() {
        return Ok(());
    }

    let db = &content.db;
    let bundle_name = import.bundle_name.as_str();
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
    Ok(())
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

async fn write_imported(cluster: &Cluster, packs: &[ImportedModpack]) -> LauncherResult<()> {
    let path = cluster.dir()?.join(IMPORTED_MODPACKS_FILE);
    if packs.is_empty() {
        if polyio::try_exists(&path).await.unwrap_or(false) {
            polyio::remove_file(&path).await?;
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        polyio::create_dir_all(parent).await?;
    }
    polyio::write(&path, serde_json::to_vec_pretty(packs)?).await?;
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
