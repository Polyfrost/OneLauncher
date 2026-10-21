use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use oneclient_cluster::naming::validate_tag;
use oneclient_cluster::profiles::get_profile_or_default;
use oneclient_cluster::{Cluster, ClusterKind, ClusterUpdate, CreateClusterOptions, ProfileUpdate};
use oneclient_common::domain::ContentType;
use oneclient_common::patch::Patch;
use oneclient_content::modpacks::{
    self, ClusterShare, ModpackInstallReport, ModpackManifest, ModpackSummary, SHARE_FORMAT,
    ShareExportReport, ShareExtras, ShareIcon, ShareScope, SharedJava, SharedModpack,
};
use oneclient_content::packages::PackageStore;
use oneclient_content::{ContentCtx, ContentError};
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::{ArtifactRow, OverrideType};
use oneclient_events::{GroupedProgressSession, TaskCategory, TaskPhase};
use oneclient_java::JavaVendor;

use super::modpack::{ModpackCluster, PreparedModpack, kind_for, link_project_details};
use crate::LauncherResult;
use crate::state::LauncherState;

const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SharedCluster {
    pub archive_path: PathBuf,
    pub manifest: ModpackManifest,
    pub share: ClusterShare,
    pub extras: ShareExtras,
    pub bundled: Vec<(ContentType, u64)>,
    pub notices: Vec<String>,
    pub substituted: bool,
}

impl SharedCluster {
    #[must_use]
    pub fn summary(&self, base: &ModpackManifest) -> ModpackSummary {
        let own = self.manifest.summary();
        let mut summary = if self.share.modpack.is_none() {
            own
        } else {
            let pack = base.summary();
            ModpackSummary {
                mods: pack.mods + own.mods,
                resource_packs: pack.resource_packs + own.resource_packs,
                shaders: pack.shaders + own.shaders,
                other_files: pack.other_files,
                optional: pack.optional + own.optional,
                manual: pack.manual,
                unresolved: pack.unresolved + own.unresolved,
                unverified: 0,
                download_bytes: pack.download_bytes + own.download_bytes,
            }
        };

        for content_type in &self.extras.embedded {
            count_content(&mut summary, *content_type);
        }
        summary.unverified = self.extras.embedded.len();
        if self.share.kind.is_isolated() {
            summary.other_files += self.extras.other_files;
        }
        for (content_type, size) in self.extras.downloads.iter().chain(&self.bundled) {
            count_content(&mut summary, *content_type);
            summary.download_bytes += size;
        }
        summary
    }
}

pub(super) async fn planned_bundle_content(
    state: &Arc<LauncherState>,
    share: &ClusterShare,
) -> Vec<(ContentType, u64)> {
    if share.kind != ClusterKind::OneClient || share.modpack.is_some() {
        return Vec::new();
    }
    let archives = match state
        .bundles
        .archives_for(
            &state.services.content(),
            &share.mc_version,
            share.mc_loader,
        )
        .await
    {
        Ok(archives) => archives,
        Err(err) => {
            tracing::debug!(mc_version = %share.mc_version, error = %err, "could not read the bundles a shared instance takes");
            return Vec::new();
        }
    };

    let accepted: HashSet<String> = share
        .bundles
        .iter()
        .filter(|choice| choice.accepted)
        .map(|choice| choice.name.clone())
        .collect();
    let overrides: HashMap<(&str, &str), OverrideType> = share
        .overrides
        .iter()
        .filter_map(|o| {
            Some((
                (o.bundle.as_str(), o.package_id.as_str()),
                o.override_type()?,
            ))
        })
        .collect();

    oneclient_content::bundles::wanted_bundle_files(
        archives.iter().map(|archive| &archive.manifest),
        &accepted,
        |bundle, package| overrides.get(&(bundle, package)).copied(),
    )
    .into_iter()
    .map(|file| (file.content_type(), file.size))
    .collect()
}

fn count_content(summary: &mut ModpackSummary, content_type: ContentType) {
    match content_type {
        ContentType::ResourcePack => summary.resource_packs += 1,
        ContentType::Shader => summary.shaders += 1,
        _ => summary.mods += 1,
    }
}

pub(super) async fn resolve_base_modpack(
    content: &ContentCtx,
    share: &ClusterShare,
    pack: &SharedModpack,
    notices: &mut Vec<String>,
) -> LauncherResult<(ArtifactRow, bool)> {
    match PackageStore::resolve_or_download(
        pack.provider,
        &pack.project_id,
        &pack.version_id,
        None,
        content,
    )
    .await
    {
        Ok(artifact) => return Ok((artifact, false)),
        Err(err) => {
            tracing::warn!(project_id = %pack.project_id, version_id = %pack.version_id, error = %err, "the shared modpack version is unavailable; looking for another");
        }
    }

    let Some(version) = modpacks::latest_modpack_version(
        pack.provider,
        &pack.project_id,
        &share.mc_version,
        share.mc_loader,
        content,
    )
    .await?
    else {
        return Err(ContentError::InvalidData {
            reason: format!(
                "The modpack this instance is built on is no longer available on {} for Minecraft {}.",
                pack.provider, share.mc_version
            ),
        }
        .into());
    };

    let artifact = PackageStore::resolve_or_download(
        pack.provider,
        &pack.project_id,
        &version.version_id,
        None,
        content,
    )
    .await?;
    notices.push(format!(
        "The modpack version this instance was made with is no longer available, so {} is installed instead.",
        version.version_number
    ));
    Ok((artifact, true))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    OneClient,
    Compatibility,
}

#[tracing::instrument(skip(state, progress))]
pub async fn export_cluster(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    dest: &Path,
    mode: ExportMode,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ShareExportReport> {
    let content = state.services.content();
    let cluster = state.clusters.get(cluster_id).await?;
    let child = progress.map(|session| {
        let child = session.child(cluster.name.clone(), 1, TaskCategory::Packages);
        child.set_phase(TaskPhase::Exporting);
        child
    });

    let compatible = mode == ExportMode::Compatibility;
    let modpack = match cluster.linked_modpack_hash {
        Some(_) if compatible => None,
        Some(_) => modpacks::cluster_modpack(cluster_id, &content)
            .await
            .inspect_err(|err| {
                tracing::debug!(cluster_id, error = %err, "could not identify the linked modpack; exporting every file");
            })
            .ok()
            .flatten()
            .map(|release| SharedModpack {
                provider: release.provider,
                project_id: release.project_id,
                version_id: release.version_id,
            }),
        None => None,
    };

    let scope = ShareScope {
        skip_bundles: cluster.uses_bundles() && !compatible,
        skip_modpack: modpack.is_some(),
    };
    let kind = if compatible && cluster.kind == ClusterKind::OneClient {
        kind_for(cluster.mc_loader)
    } else {
        cluster.kind
    };
    let shared =
        modpacks::collect_share_content(cluster_id, scope, child.as_ref(), &content).await?;

    let share = ClusterShare {
        format: SHARE_FORMAT,
        name: cluster.name.clone(),
        description: cluster.description.clone(),
        tags: cluster.tags.clone(),
        kind,
        mc_version: cluster.mc_version.clone(),
        mc_loader: cluster.mc_loader,
        mc_loader_version: exported_loader_version(state, &cluster).await?,
        dedicated_dir: cluster.uses_dedicated_dir() || kind.is_isolated(),
        java: exported_java(state, &cluster).await,
        modpack,
        bundles: Vec::new(),
        overrides: Vec::new(),
        disabled: Vec::new(),
        downloads: Vec::new(),
        icon: None,
    };
    let icon = exported_icon(&cluster).await;

    let report =
        modpacks::write_share_archive(dest, &share, &shared, icon.as_ref(), child.as_ref()).await?;
    if let Some(child) = child {
        child.finish();
    }
    Ok(report)
}

async fn exported_loader_version(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
) -> LauncherResult<Option<String>> {
    if !cluster.mc_loader.is_modded() {
        return Ok(None);
    }
    let resolved = super::prepare::resolved_loader_version(state, cluster)
        .await
        .inspect_err(|err| {
            tracing::warn!(cluster_id = cluster.id, error = %err, "could not work out the loader version to export");
        })
        .ok()
        .flatten();
    match resolved {
        Some(version) => Ok(Some(version)),
        None => Err(ContentError::InvalidData {
            reason: format!(
                "Couldn't work out which {} version this instance uses. Check your connection and try again.",
                cluster.mc_loader
            ),
        }
        .into()),
    }
}

async fn exported_java(state: &Arc<LauncherState>, cluster: &Cluster) -> Option<SharedJava> {
    let global = state.settings.read().global_game_settings.clone();
    let profile = get_profile_or_default(
        &state.services.db,
        &global,
        cluster.setting_profile_name.as_deref(),
    )
    .await
    .ok()?;
    let java_path = profile
        .java_path
        .filter(|path| global.java_path.as_ref() != Some(path))?;
    let runtime = state
        .java
        .runtime_for_profile(Some(&java_path))
        .await
        .ok()
        .flatten()?;
    let required = super::required_java_major(state, cluster.id)
        .await
        .inspect_err(|err| {
            tracing::debug!(cluster_id = cluster.id, error = %err, "could not tell which java the version needs");
        })
        .ok()
        .flatten();
    if required == Some(runtime.major) {
        return None;
    }
    Some(SharedJava {
        major: runtime.major,
        vendor: Some(runtime.vendor.to_string()),
    })
}

async fn exported_icon(cluster: &Cluster) -> Option<ShareIcon> {
    for path in [cluster.cover_file(), cluster.modpack_icon_file()]
        .into_iter()
        .flatten()
    {
        let Some(entry) = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(modpacks::icon_entry)
        else {
            continue;
        };
        let Ok(meta) = polyio::stat(&path).await else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_ICON_BYTES {
            continue;
        }
        if let Ok(bytes) = polyio::read(&path).await {
            let extension = entry
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_string())
                .unwrap_or_default();
            if extension == "png" {
                return Some(ShareIcon { extension, bytes });
            }
            let converted = tokio::task::spawn_blocking(move || as_png(&bytes))
                .await
                .ok()
                .flatten();
            if let Some(bytes) = converted {
                return Some(ShareIcon {
                    extension: "png".to_string(),
                    bytes,
                });
            }
        }
    }
    None
}

fn as_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?;
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

pub(super) fn shared_options(
    prepared: &PreparedModpack,
    shared: &SharedCluster,
) -> CreateClusterOptions {
    let share = &shared.share;
    let kind = if share.modpack.is_some() {
        kind_for(share.mc_loader)
    } else {
        share.kind
    };
    let tags = share
        .tags
        .iter()
        .filter(|tag| validate_tag(tag).is_ok())
        .cloned()
        .collect();

    let mut options = CreateClusterOptions::new(
        prepared.instance_name.clone(),
        share.mc_version.clone(),
        share.mc_loader,
    )
    .kind(kind)
    .user_created(true)
    .modpack(true)
    .tags(tags);
    options.mc_loader_version = if shared.substituted {
        prepared.manifest.loader_version.clone()
    } else {
        share.mc_loader_version.clone()
    };
    options.description = share
        .description
        .clone()
        .filter(|text| !text.trim().is_empty());
    options
}

#[tracing::instrument(skip(state, prepared, shared, progress), fields(name = %shared.share.name))]
pub(super) async fn install_shared_instance(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    prepared: &PreparedModpack,
    shared: &SharedCluster,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<ModpackCluster> {
    let content = state.services.content();
    let share = &shared.share;

    let overrides: Vec<_> = share
        .overrides
        .iter()
        .filter_map(|o| Some((o.bundle.clone(), o.package_id.clone(), o.override_type()?)))
        .collect();
    bundle_dao::save_overrides(&state.services.db, cluster_id, &overrides).await?;

    let mut report = ModpackInstallReport::default();
    if share.modpack.is_some() {
        report = modpacks::install_modpack(
            &prepared.archive_path,
            &prepared.manifest,
            cluster_id,
            modpacks::MODPACK_BUNDLE_NAME,
            progress,
            &content,
        )
        .await?;
        link_project_details(state, cluster_id, &prepared.artifact_hash).await;
    }

    let cluster = state.clusters.get(cluster_id).await?;
    if cluster.uses_bundles() {
        report
            .failed
            .extend(install_shared_bundles(state, &cluster, share, progress).await);
    }

    let extra = modpacks::install_shared_content(
        &shared.archive_path,
        &shared.manifest,
        share,
        cluster_id,
        progress,
        &content,
    )
    .await?;
    report.installed += extra.installed;
    report.failed.extend(extra.failed);
    report.blocked.extend(extra.blocked);

    report
        .failed
        .extend(apply_shared_settings(state, &cluster, shared, progress).await);

    let cluster = state.clusters.get(cluster_id).await?;
    Ok(ModpackCluster { cluster, report })
}

async fn install_shared_bundles(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    share: &ClusterShare,
    progress: Option<&GroupedProgressSession>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let content = state.services.content();
    let choices: Vec<(String, bool)> = share
        .bundles
        .iter()
        .map(|choice| (choice.name.clone(), choice.accepted))
        .collect();
    if let Err(err) =
        oneclient_content::bundles::set_bundle_choices(cluster.id, &choices, &content).await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "could not record the shared bundle choices");
        problems.push("its bundle choices".to_string());
    }
    if let Err(err) = oneclient_content::bundles::install_cluster_bundles(
        cluster.id,
        state.bundles.as_ref(),
        progress,
        &content,
    )
    .await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "could not install bundle content for the shared instance");
        problems.push("its bundle mods".to_string());
    }
    problems
}

async fn apply_shared_settings(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    shared: &SharedCluster,
    progress: Option<&GroupedProgressSession>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let share = &shared.share;

    if let Some(description) = share
        .description
        .as_ref()
        .filter(|text| !text.trim().is_empty())
        && cluster.description.as_ref() != Some(description)
        && let Err(err) = state
            .clusters
            .update(
                cluster.id,
                ClusterUpdate {
                    description: Patch::Set(description.clone()),
                    ..Default::default()
                },
            )
            .await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "could not set the shared description");
        problems.push("its description".to_string());
    }

    if let Err(err) = apply_shared_icon(state, cluster.id, shared).await {
        tracing::warn!(cluster_id = cluster.id, error = %err, "could not set the shared cover");
        problems.push("its cover image".to_string());
    }

    if !cluster.is_isolated()
        && let Err(err) = state
            .clusters
            .set_dedicated_dir(cluster.id, share.dedicated_dir, false)
            .await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "could not apply the shared game folder setting");
        problems.push("its game folder setting".to_string());
    }

    if let Some(java) = &share.java
        && let Err(err) = apply_shared_java(state, cluster.id, java).await
    {
        tracing::warn!(cluster_id = cluster.id, major = java.major, error = %err, "could not set up the shared java runtime");
        problems.push(format!("its Java {} setting", java.major));
    }

    if cluster.kind == ClusterKind::OneClient
        && let Err(err) =
            super::apply_bundle_java_override(state, cluster.id, true, true, progress).await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "could not apply the bundle java override");
        problems.push("the Java version its bundles ask for".to_string());
    }
    problems
}

async fn apply_shared_icon(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    shared: &SharedCluster,
) -> LauncherResult<()> {
    let Some(icon) = modpacks::read_share_icon(&shared.archive_path, &shared.share).await else {
        return Ok(());
    };
    let staging = polyio::tempdir().await?;
    let path = staging.dir_path().join(format!("cover.{}", icon.extension));
    polyio::write(&path, &icon.bytes).await?;
    state
        .clusters
        .set_cover_from_file(cluster_id, &path)
        .await?;
    Ok(())
}

async fn apply_shared_java(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    java: &SharedJava,
) -> LauncherResult<()> {
    let vendor = java
        .vendor
        .as_deref()
        .and_then(|vendor| JavaVendor::from_str(vendor).ok());
    let installed = state
        .java
        .list_runtimes()
        .await?
        .into_iter()
        .filter(|runtime| {
            runtime.major == java.major && Path::new(&runtime.absolute_path).is_file()
        })
        .max_by_key(|runtime| vendor.as_ref() == Some(&runtime.vendor));
    let Some(runtime) = installed else {
        tracing::info!(
            cluster_id,
            major = java.major,
            "no installed java matches the shared one; leaving the instance on its default"
        );
        return Ok(());
    };

    state
        .clusters
        .update_profile(
            cluster_id,
            ProfileUpdate {
                java_path: Patch::Set(runtime.absolute_path.clone()),
                ..Default::default()
            },
        )
        .await?;
    tracing::info!(cluster_id, major = java.major, java_path = %runtime.absolute_path, "applied the shared java runtime");
    Ok(())
}
