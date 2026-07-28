use std::collections::HashMap;
use std::sync::Arc;

use interfrost::api::minecraft::{DownloadType, VersionInfo};
use interfrost::api::modded::SidedDataEntry;
use tokio::process::Command;

use oneclient_cluster::Cluster;
use oneclient_cluster::ClusterError;
use oneclient_cluster::ClusterStage;
use crate::game::{
    self, download_minecraft, download_version_info, get_loader_version, resolve_minecraft_version,
};
use oneclient_java::JavaRuntime;
use oneclient_mc::MetadataStore;
use oneclient_events::GroupedProgressSession;
use oneclient_common::paths;
use crate::state::{LauncherServices, LauncherState};
use crate::{GameError, LauncherResult};

/// Locks the metadata store, then prepares the cluster.
///
/// Was `ClusterManager::prepare`, which only existed to take that lock before
/// delegating here. It lives with `prepare` rather than with cluster records,
/// because the metadata store is a download concern.
#[tracing::instrument(skip(state, shared_progress))]
pub async fn prepare_cluster_locked(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    force: bool,
    search_for_java: bool,
    auto_install_java: bool,
    shared_progress: Option<&GroupedProgressSession>,
) -> LauncherResult<Cluster> {
    let mut metadata = state.metadata.lock().await;
    prepare_cluster(
        state,
        &mut metadata,
        cluster_id,
        force,
        search_for_java,
        auto_install_java,
        shared_progress,
    )
    .await
}

pub async fn prepare_cluster(
    state: &Arc<LauncherState>,
    metadata: &mut MetadataStore,
    cluster_id: i64,
    force: bool,
    search_for_java: bool,
    auto_install_java: bool,
    shared_progress: Option<&GroupedProgressSession>,
) -> LauncherResult<Cluster> {
    let cluster = state.clusters.get(cluster_id).await?;
    let continuing = cluster.stage == ClusterStage::Downloading;

    tracing::info!(
        cluster_id,
        mc_version = %cluster.mc_version,
        force,
        continuing,
        "preparing cluster"
    );

    if !continuing {
        state.clusters.set_stage(cluster_id, ClusterStage::Downloading).await?;
    }

    let owned = shared_progress.is_none().then(|| {
        GroupedProgressSession::start(
            &state.services.events,
            format!("Downloading game - {}", cluster.mc_version),
        )
    });
    let progress = shared_progress.or(owned.as_ref()).expect("session present");

    let result = install_cluster(
        state,
        metadata,
        &cluster,
        progress,
        force,
        search_for_java,
        auto_install_java,
    )
    .await;

    if let Some(owned) = owned {
        owned.finish();
    }

    if let Err(err) = result {
        tracing::error!(cluster_id, error = %err, "cluster preparation failed");
        if !continuing {
            let _ = state.clusters.set_stage(cluster_id, ClusterStage::NotReady).await;
        }
        return Err(err);
    }

    let cluster = state.clusters.set_stage(cluster_id, ClusterStage::Ready).await?;
    tracing::debug!(cluster_id, "cluster stage set to Ready");
    Ok(cluster)
}

const JRE_ESTIMATE_BYTES: u64 = 45_000_000;

/// Bytes the game install still needs. Falls back to the manifest's full size
/// when the asset index isn't cached yet, which is also the case where nothing
/// is on disk, so the full size is the right answer anyway.
async fn game_download_bytes(services: &LauncherServices, info: &VersionInfo) -> u64 {
    let Some(assets_index) = cached_assets_index(info).await else {
        let client = info
            .downloads
            .get(&DownloadType::Client)
            .map_or(0, |d| d.size as u64);
        let libraries: u64 = info
            .libraries
            .iter()
            .filter_map(|lib| lib.downloads.as_ref())
            .filter_map(|dl| dl.artifact.as_ref())
            .map(|artifact| artifact.size as u64)
            .sum();
        return client + info.asset_index.total_size as u64 + libraries;
    };

    let _ = services;
    // The estimate runs before a Java runtime is resolved, so rules are checked
    // against the host architecture. That only shifts natives-related edge cases.
    match game::plan_downloads(info, assets_index, std::env::consts::ARCH, false, false).await {
        Ok(plan) => plan.total_bytes(),
        Err(err) => {
            tracing::warn!("download plan failed, estimating full size: {err}");
            info.asset_index.total_size as u64
        }
    }
}

async fn cached_assets_index(
    info: &VersionInfo,
) -> Option<interfrost::api::minecraft::AssetsIndex> {
    let path = paths::assets_index_dir()
        .ok()?
        .join(format!("{}.json", info.asset_index.id));
    polyio::read_json(&path).await.ok()
}

#[tracing::instrument(level = "debug", skip(state, bundles))]
pub async fn estimate_cluster_download(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    bundles: &oneclient_content::bundles::BundlesManager,
) -> LauncherResult<u64> {
    let cluster = state.clusters.get(cluster_id).await?;
    let mc_version = oneclient_common::version::normalize_mc_version_input(&cluster.mc_version);

    let info = {
        let mut metadata = state.metadata.lock().await;
        let (version, _index, _updated) =
            resolve_minecraft_version(&mut metadata, &state.services.mc(), &mc_version)
                .await
                .map_err(|_| ClusterError::InvalidVersion(cluster.mc_version.clone()))?;
        let loader_version = get_loader_version(
            &mut metadata,
            &state.services.mc(),
            &mc_version,
            cluster.mc_loader,
            cluster.mc_loader_version.as_deref(),
        )
        .await?;
        download_version_info(&state.services.mc(), None, &version, loader_version.as_ref(), false).await?
    };

    let mut total = game_download_bytes(&state.services, &info).await;

    if let Some(java) = &info.java_version {
        let installed = state.java.list_runtimes()
            .await
            .unwrap_or_default();
        if !installed.iter().any(|rt| rt.major == java.major_version) {
            total += JRE_ESTIMATE_BYTES;
        }
    }

    total += oneclient_content::bundles::enabled_bundle_bytes(cluster_id, bundles, &state.services.content())
        .await
        .unwrap_or(0);

    Ok(total)
}

#[tracing::instrument(skip(state, metadata, cluster, progress), fields(cluster_id = cluster.id))]
async fn install_cluster(
    state: &Arc<LauncherState>,
    metadata: &mut MetadataStore,
    cluster: &Cluster,
    progress: &GroupedProgressSession,
    force: bool,
    search_for_java: bool,
    auto_install_java: bool,
) -> LauncherResult<()> {
    let global = state.settings.read().global_game_settings.clone();
    let profile = state.clusters.resolve_settings(&global, cluster).await?;

    let mc_version = oneclient_common::version::normalize_mc_version_input(&cluster.mc_version);

    let (version, _version_index, minecraft_updated) =
        resolve_minecraft_version(metadata, &state.services.mc(), &mc_version)
            .await
            .map_err(|_| ClusterError::InvalidVersion(cluster.mc_version.clone()))?;

    let loader_version = get_loader_version(
        metadata,
        &state.services.mc(),
        &mc_version,
        cluster.mc_loader,
        cluster.mc_loader_version.as_deref(),
    )
    .await?;

    let mut version_info = download_version_info(
        &state.services.mc(),
        Some(progress),
        &version,
        loader_version.as_ref(),
        force,
    )
    .await?;

    let java_major = version_info
        .java_version
        .as_ref()
        .map(|v| v.major_version)
        .ok_or(ClusterError::MissingJavaVersion)?;

    let java = if let Some(runtime) =
        state.java.runtime_for_profile(profile.java_path.as_deref()).await?
    {
        runtime
    } else {
        state
            .java
            .prepare(java_major, search_for_java, auto_install_java, Some(progress))
            .await?
    };

    download_minecraft(
        &state.services.mc(),
        progress,
        &version_info,
        &java.os_arch,
        minecraft_updated,
        force,
    )
    .await?;

    run_forge_processors(cluster, &mut version_info, &java).await?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip(cluster, version_info, java), fields(cluster_id = cluster.id))]
async fn run_forge_processors(
    cluster: &Cluster,
    version_info: &mut VersionInfo,
    java: &JavaRuntime,
) -> LauncherResult<()> {
    let Some(processors) = &version_info.processors else {
        return Ok(());
    };

    let client = paths::versions_dir()?
        .join(&version_info.id)
        .join(format!("{}.jar", version_info.id));
    let libraries = paths::libraries_dir()?;
    let cluster_dir = cluster.game_dir()?;

    let Some(data) = &mut version_info.data else {
        return Ok(());
    };

    macro_rules! data_entry {
        ($dest:expr; $($name:literal: client => $client:expr, server => $server:expr;)+) => {
            $(HashMap::insert(
                $dest,
                String::from($name),
                SidedDataEntry {
                    client: String::from($client),
                    server: String::from($server),
                },
            );)+
        };
    }

    data_entry! {
        data;
        "SIDE":
            client => "client",
            server => "";
        "MINECRAFT_JAR":
            client => client.to_string_lossy(),
            server => "";
        "MINECRAFT_VERSION":
            client => cluster.mc_version.clone(),
            server => "";
        "ROOT":
            client => cluster_dir.to_string_lossy(),
            server => "";
        "LIBRARY_DIR":
            client => libraries.to_string_lossy(),
            server => "";
    }

    let total = processors.len();
    for (index, processor) in processors.iter().enumerate() {
        if let Some(sides) = &processor.sides
            && !sides.contains(&String::from("client"))
        {
            continue;
        }

        let mut cp = processor.classpath.clone();
        cp.push(processor.jar.clone());

        let processor_jar = game::get_library(&libraries, &processor.jar, false)?;
        let main = game::main_class(&processor_jar)
            .await?
            .ok_or_else(|| GameError::ProcessorMainClass(processor.jar.clone()))?;

        let output = Command::new(&java.absolute_path)
            .arg("-cp")
            .arg(game::get_classpath_library(&libraries, &cp)?)
            .arg(&main)
            .args(game::processor_arguments(
                &libraries,
                &processor.args,
                data,
            )?)
            .output()
            .await?;

        if !output.status.success() {
            return Err(GameError::ProcessorFailed(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            )
            .into());
        }

        tracing::debug!(
            "ran forge processor {}/{} for {}",
            index + 1,
            total,
            version_info.id
        );
    }

    Ok(())
}
