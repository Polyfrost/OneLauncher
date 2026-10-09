use std::path::PathBuf;
use std::sync::Arc;

use oneclient_cluster::naming::{is_allowed_name_char, validate_modpack_instance_name};
use oneclient_cluster::{Cluster, ClusterKind, ClusterUpdate, CreateClusterOptions};
use oneclient_common::domain::{GameLoader, ProviderId};
use oneclient_common::patch::Patch;
use oneclient_content::ContentError;
use oneclient_content::modpacks::{self, ModpackInstallReport, ModpackManifest, ModpackRelease};
use oneclient_content::packages::store::artifact_absolute_path;
use oneclient_content::packages::{PackageStore, ProjectDetail};
use oneclient_db::dao::artifact::get_artifact_by_hash;
use oneclient_events::GroupedProgressSession;

use crate::LauncherResult;
use crate::images::DEFAULT_IMAGE_EDGE;
use crate::state::LauncherState;

const FALLBACK_NAME: &str = "Modpack";
const MODPACK_ICON_EDGE: u32 = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModpackSource {
    File(PathBuf),
    Provider {
        provider: ProviderId,
        project_id: String,
        version_id: String,
    },
}

#[derive(Debug, Clone)]
pub struct ModpackCluster {
    pub cluster: Cluster,
    pub report: ModpackInstallReport,
}

#[derive(Debug, Clone)]
pub struct PreparedModpack {
    pub source: ModpackSource,
    pub artifact_hash: String,
    pub archive_path: PathBuf,
    pub manifest: ModpackManifest,
    pub instance_name: String,
}

#[tracing::instrument(skip(state))]
pub async fn prepare_modpack(
    state: &Arc<LauncherState>,
    source: &ModpackSource,
) -> LauncherResult<PreparedModpack> {
    let content = state.services.content();

    let artifact = match source {
        ModpackSource::File(path) => modpacks::store_modpack_archive(path, &content).await?,
        ModpackSource::Provider {
            provider,
            project_id,
            version_id,
        } => PackageStore::resolve_or_download(*provider, project_id, version_id, &content).await?,
    };
    let archive_path = artifact_absolute_path(&artifact.path)?;
    let manifest = modpacks::read_modpack(&archive_path, &content).await?;

    let taken: Vec<String> = state
        .clusters
        .list()
        .await?
        .into_iter()
        .map(|cluster| cluster.name)
        .collect();

    Ok(PreparedModpack {
        source: source.clone(),
        artifact_hash: artifact.hash,
        archive_path,
        instance_name: unique_name(&instance_name(&manifest.name), &taken),
        manifest,
    })
}

#[tracing::instrument(skip(state, prepared), fields(pack = %prepared.manifest.name))]
pub async fn create_modpack_instance(
    state: &Arc<LauncherState>,
    prepared: &PreparedModpack,
) -> LauncherResult<Cluster> {
    let manifest = &prepared.manifest;
    let global = state.settings.read().global_game_settings.clone();
    let mut options = CreateClusterOptions::new(
        prepared.instance_name.clone(),
        manifest.mc_version.clone(),
        manifest.loader,
    )
    .kind(kind_for(manifest.loader))
    .user_created(true)
    .modpack(true);
    options.mc_loader_version = manifest.loader_version.clone();
    options.description = manifest.summary.clone();

    let cluster = state.clusters.create(&global, options).await?;
    let linked = state
        .clusters
        .update(
            cluster.id,
            ClusterUpdate {
                linked_modpack_hash: Patch::Set(prepared.artifact_hash.clone()),
                ..Default::default()
            },
        )
        .await;

    match linked {
        Ok(cluster) => Ok(cluster),
        Err(err) => {
            if let Err(cleanup) = state.clusters.delete(cluster.id, true).await {
                tracing::warn!(cluster_id = cluster.id, error = %cleanup, "could not remove the unlinked modpack instance");
            }
            Err(err.into())
        }
    }
}

#[tracing::instrument(skip(state, prepared, progress), fields(pack = %prepared.manifest.name))]
pub async fn install_modpack_instance(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    prepared: &PreparedModpack,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ModpackCluster> {
    let content = state.services.content();
    let report = modpacks::install_modpack(
        &prepared.archive_path,
        &prepared.manifest,
        cluster_id,
        progress,
        &content,
    )
    .await?;

    match modpacks::identify_modpack(&prepared.artifact_hash, &content).await {
        Ok(Some(release)) => {
            if let Err(err) = apply_project_details(state, cluster_id, &release).await {
                tracing::debug!(cluster_id, error = %err, "could not read the modpack's project page");
            }
        }
        Ok(None) => {}
        Err(err) => {
            tracing::debug!(error = %err, "could not identify the modpack on any provider");
        }
    }

    let cluster = state.clusters.get(cluster_id).await?;
    Ok(ModpackCluster { cluster, report })
}

#[tracing::instrument(skip(state, progress))]
pub async fn update_modpack_cluster(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    version_id: &str,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ModpackCluster> {
    let content = state.services.content();
    let cluster = state.clusters.get(cluster_id).await?;
    let current = modpacks::cluster_modpack(cluster_id, &content)
        .await?
        .ok_or_else(|| ContentError::InvalidData {
            reason: "This instance's modpack is not published on Modrinth or CurseForge."
                .to_string(),
        })?;

    let artifact = PackageStore::resolve_or_download(
        current.provider,
        &current.project_id,
        version_id,
        &content,
    )
    .await?;
    let archive_path = artifact_absolute_path(&artifact.path)?;
    let manifest = modpacks::read_modpack(&archive_path, &content).await?;

    if manifest.mc_version != cluster.mc_version || manifest.loader != cluster.mc_loader {
        return Err(ContentError::InvalidData {
            reason: format!(
                "This version needs Minecraft {} with {}. Install it as a new instance instead.",
                manifest.mc_version, manifest.loader
            ),
        }
        .into());
    }

    let report =
        modpacks::install_modpack(&archive_path, &manifest, cluster_id, progress, &content).await?;

    let loader_version = match &manifest.loader_version {
        Some(version) if cluster.mc_loader_version.as_ref() != Some(version) => {
            Patch::Set(version.clone())
        }
        _ => Patch::Unchanged,
    };
    let cluster = state
        .clusters
        .update(
            cluster_id,
            ClusterUpdate {
                linked_modpack_hash: Patch::Set(artifact.hash.clone()),
                mc_loader_version: loader_version,
                ..Default::default()
            },
        )
        .await?;
    restore_missing_icon(state, &cluster).await;

    Ok(ModpackCluster { cluster, report })
}

#[tracing::instrument(skip(state, progress))]
pub async fn repair_modpack_cluster(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ModpackCluster> {
    let content = state.services.content();
    let cluster = state.clusters.get(cluster_id).await?;
    let (archive_path, manifest) = linked_modpack_archive(state, &cluster).await?;

    let report =
        modpacks::install_modpack(&archive_path, &manifest, cluster_id, progress, &content).await?;
    restore_missing_icon(state, &cluster).await;

    Ok(ModpackCluster { cluster, report })
}

pub(crate) async fn linked_modpack_archive(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<(PathBuf, ModpackManifest)> {
    let hash = cluster
        .linked_modpack_hash
        .clone()
        .ok_or_else(|| ContentError::InvalidData {
            reason: "This instance was not installed from a modpack.".to_string(),
        })?;

    let artifact = get_artifact_by_hash(&state.services.db, &hash)
        .await?
        .ok_or_else(|| ContentError::InvalidData {
            reason:
                "The modpack file for this instance is gone. Update the pack to fetch it again."
                    .to_string(),
        })?;
    let archive_path = artifact_absolute_path(&artifact.path)?;
    let manifest = modpacks::read_modpack(&archive_path, &state.services.content()).await?;
    Ok((archive_path, manifest))
}

async fn fetch_project(
    state: &Arc<LauncherState>,
    release: &ModpackRelease,
) -> LauncherResult<ProjectDetail> {
    let content = state.services.content();
    Ok(content
        .providers
        .get(release.provider)?
        .get_project(&release.project_id, &content)
        .await?)
}

async fn apply_project_details(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    release: &ModpackRelease,
) -> LauncherResult<()> {
    let project = fetch_project(state, release).await?;
    let cluster = state.clusters.get(cluster_id).await?;

    if let Err(err) = apply_project_description(state, &cluster, &project).await {
        tracing::debug!(cluster_id, error = %err, "could not set the modpack description");
    }
    if let Err(err) = save_project_icon(state, &cluster, &project).await {
        tracing::debug!(cluster_id, error = %err, "no icon for the modpack");
    }
    if let Err(err) = apply_project_cover(state, &cluster, &project).await {
        tracing::debug!(cluster_id, error = %err, "no cover art for the modpack");
    }
    Ok(())
}

async fn save_project_icon(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    project: &ProjectDetail,
) -> LauncherResult<()> {
    let (Some(url), Some(dest)) = (
        project.icon_url.as_deref().filter(|url| !url.is_empty()),
        cluster.modpack_icon_file(),
    ) else {
        return Ok(());
    };

    let cached = state
        .images
        .ensure_on_disk(&state.services.requester, url, MODPACK_ICON_EDGE)
        .await?;
    if let Some(parent) = dest.parent() {
        polyio::create_dir_all(parent).await?;
    }
    polyio::copy(&cached, &dest).await?;
    Ok(())
}

async fn restore_missing_icon(state: &Arc<LauncherState>, cluster: &Cluster) {
    if cluster
        .modpack_icon_file()
        .is_none_or(|icon| icon.is_file())
    {
        return;
    }

    let content = state.services.content();
    let restored = async {
        let Some(release) = modpacks::cluster_modpack(cluster.id, &content).await? else {
            return Ok(());
        };
        let project = fetch_project(state, &release).await?;
        save_project_icon(state, cluster, &project).await
    };
    let result: LauncherResult<()> = restored.await;
    if let Err(err) = result {
        tracing::debug!(cluster_id = cluster.id, error = %err, "could not restore the modpack icon");
    }
}

async fn apply_project_description(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    project: &ProjectDetail,
) -> LauncherResult<()> {
    let Some(description) = project_description(cluster.description.as_deref(), project) else {
        return Ok(());
    };

    state
        .clusters
        .update(
            cluster.id,
            ClusterUpdate {
                description: Patch::Set(description),
                ..Default::default()
            },
        )
        .await?;
    Ok(())
}

fn project_description(current: Option<&str>, project: &ProjectDetail) -> Option<String> {
    if current.is_some_and(|current| !current.trim().is_empty()) {
        return None;
    }
    let summary = project.summary.trim();
    (!summary.is_empty()).then(|| summary.to_string())
}

async fn apply_project_cover(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    project: &ProjectDetail,
) -> LauncherResult<()> {
    let Some(url) = cover_url(project) else {
        return Ok(());
    };

    let cached = state
        .images
        .ensure_on_disk(&state.services.requester, &url, DEFAULT_IMAGE_EDGE)
        .await?;
    state
        .clusters
        .set_cover_from_file(cluster.id, &cached)
        .await?;
    Ok(())
}

fn cover_url(project: &ProjectDetail) -> Option<String> {
    project
        .gallery
        .iter()
        .find(|image| image.featured)
        .or_else(|| project.gallery.first())
        .map(|image| image.url.clone())
        .or_else(|| project.icon_url.clone())
        .filter(|url| !url.is_empty())
}

fn kind_for(loader: GameLoader) -> ClusterKind {
    if loader.is_modded() {
        ClusterKind::Modded
    } else {
        ClusterKind::Vanilla
    }
}

fn unique_name(base: &str, taken: &[String]) -> String {
    let is_taken = |candidate: &str| {
        taken
            .iter()
            .any(|name| name.trim().eq_ignore_ascii_case(candidate))
    };
    if !is_taken(base) {
        return base.to_string();
    }

    (2..)
        .map(|n| format!("{base} ({n})"))
        .find(|candidate| !is_taken(candidate))
        .unwrap_or_else(|| base.to_string())
}

fn instance_name(pack_name: &str) -> String {
    let name = pack_name
        .chars()
        .filter(|c| is_allowed_name_char(*c))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if validate_modpack_instance_name(&name).is_ok() {
        name
    } else {
        FALLBACK_NAME.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(summary: &str) -> ProjectDetail {
        ProjectDetail {
            id: "pack".into(),
            slug: "pack".into(),
            provider: ProviderId::Modrinth,
            content_type: oneclient_content::packages::ContentType::Modpack,
            name: "Pack".into(),
            summary: summary.into(),
            author: String::new(),
            members: Vec::new(),
            gallery: Vec::new(),
            body: oneclient_content::packages::PackageBody::Raw(String::new()),
            license: None,
            links: Vec::new(),
            version_ids: Vec::new(),
            game_versions: Vec::new(),
            loaders: Vec::new(),
            icon_url: None,
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
            downloads: 0,
        }
    }

    #[test]
    fn the_project_summary_only_fills_an_empty_description() {
        let summary = "  Adventure pack with 300+ mods.  ";

        assert_eq!(
            project_description(None, &project(summary)),
            Some("Adventure pack with 300+ mods.".to_string())
        );
        assert_eq!(
            project_description(Some("   "), &project(summary)),
            Some("Adventure pack with 300+ mods.".to_string())
        );
        assert_eq!(project_description(Some("Mine"), &project(summary)), None);
        assert_eq!(project_description(None, &project("   ")), None);
    }

    #[test]
    fn pack_names_are_cut_down_to_a_valid_instance_name() {
        assert_eq!(
            instance_name("Fabulously Optimized"),
            "Fabulously Optimized"
        );
        assert_eq!(
            instance_name("All the Mods 10: To the Sky"),
            "All the Mods 10 To the Sky"
        );
        assert_eq!(instance_name("Better MC [FORGE]"), "Better MC FORGE");
        assert_eq!(instance_name("✨✨✨"), FALLBACK_NAME);
        assert_eq!(instance_name(""), FALLBACK_NAME);
    }

    #[test]
    fn a_second_copy_of_a_pack_gets_a_numbered_name() {
        let taken = vec!["Better MC FABRIC BMC".to_string()];
        assert_eq!(
            unique_name("Better MC FABRIC BMC", &taken),
            "Better MC FABRIC BMC (2)"
        );

        let taken = vec!["Fabulously".to_string(), "fabulously (2)".to_string()];
        assert_eq!(unique_name("Fabulously", &taken), "Fabulously (3)");
        assert_eq!(unique_name("Fresh", &taken), "Fresh");
        assert!(validate_modpack_instance_name(&unique_name("Fabulously", &taken)).is_ok());
    }

    #[test]
    fn a_pack_without_a_loader_makes_a_vanilla_instance() {
        assert_eq!(kind_for(GameLoader::Vanilla), ClusterKind::Vanilla);
        assert_eq!(kind_for(GameLoader::Fabric), ClusterKind::Modded);
    }
}
