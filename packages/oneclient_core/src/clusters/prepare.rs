use std::collections::HashMap;
use std::sync::Arc;

use interfrost::api::minecraft::{DownloadType, VersionInfo};
use interfrost::api::modded::SidedDataEntry;
use tokio::process::Command;
use tracing::instrument;

use crate::clusters::cluster::Cluster;
use crate::clusters::error::ClusterError;
use crate::clusters::stage::ClusterStage;
use crate::clusters::ClusterManager;
use crate::game::{
    self, download_minecraft, download_version_info, get_loader_version, resolve_minecraft_version,
};
use crate::java::JavaRuntime;
use crate::java::JavaManager;
use crate::metadata::MetadataStore;
use crate::notification::GroupedProgressSession;
use crate::paths;
use crate::state::LauncherState;
use crate::{GameError, LauncherResult};

#[instrument(skip(state, metadata, shared_progress))]
pub async fn prepare_cluster(
    state: &Arc<LauncherState>,
    metadata: &mut MetadataStore,
    cluster_id: i64,
    force: bool,
    search_for_java: bool,
    auto_install_java: bool,
    shared_progress: Option<&GroupedProgressSession>,
) -> LauncherResult<Cluster> {
    let cluster = ClusterManager::get(state, cluster_id).await?;
    let continuing = cluster.stage == ClusterStage::Downloading;

    if !continuing {
        ClusterManager::set_stage(state, cluster_id, ClusterStage::Downloading).await?;
    }

    let owned = shared_progress.is_none().then(|| {
        GroupedProgressSession::start(
            &state.services.notifier,
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
        if !continuing {
            let _ = ClusterManager::set_stage(state, cluster_id, ClusterStage::NotReady).await;
        }
        return Err(err);
    }

    let cluster = ClusterManager::set_stage(state, cluster_id, ClusterStage::Ready).await?;
    Ok(cluster)
}

const JRE_ESTIMATE_BYTES: u64 = 45_000_000;

fn game_download_bytes(info: &VersionInfo) -> u64 {
    let client = info
        .downloads
        .get(&DownloadType::Client)
        .map(|d| d.size as u64)
        .unwrap_or(0);
    let assets = info.asset_index.total_size as u64;
    let libraries: u64 = info
        .libraries
        .iter()
        .filter_map(|lib| lib.downloads.as_ref())
        .filter_map(|dl| dl.artifact.as_ref())
        .map(|artifact| artifact.size as u64)
        .sum();
    client + assets + libraries
}

pub async fn estimate_cluster_download(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    bundles: &crate::bundles::BundlesManager,
) -> LauncherResult<u64> {
    let cluster = ClusterManager::get(state, cluster_id).await?;
    let mc_version = crate::version::normalize_mc_version_input(&cluster.mc_version);

    let info = {
        let mut metadata = state.metadata.lock().await;
        let (version, _index, _updated) =
            resolve_minecraft_version(&mut metadata, &state.services, &mc_version)
                .await
                .map_err(|_| ClusterError::InvalidVersion(cluster.mc_version.clone()))?;
        let loader_version = get_loader_version(
            &mut metadata,
            &state.services,
            &mc_version,
            cluster.mc_loader,
            cluster.mc_loader_version.as_deref(),
        )
        .await?;
        download_version_info(&state.services, None, &version, loader_version.as_ref(), false).await?
    };

    let mut total = game_download_bytes(&info);

    if let Some(java) = &info.java_version {
        let installed = JavaManager::list_runtimes(&state.services.db)
            .await
            .unwrap_or_default();
        if !installed.iter().any(|rt| rt.major == java.major_version) {
            total += JRE_ESTIMATE_BYTES;
        }
    }

    total += crate::bundles::enabled_bundle_bytes(cluster_id, bundles, &state.services)
        .await
        .unwrap_or(0);

    Ok(total)
}

async fn install_cluster(
    state: &Arc<LauncherState>,
    metadata: &mut MetadataStore,
    cluster: &Cluster,
    progress: &GroupedProgressSession,
    force: bool,
    search_for_java: bool,
    auto_install_java: bool,
) -> LauncherResult<()> {
    let profile = ClusterManager::resolve_settings(state, cluster).await?;

    let mc_version = crate::version::normalize_mc_version_input(&cluster.mc_version);

    let (version, _version_index, minecraft_updated) =
        resolve_minecraft_version(metadata, &state.services, &mc_version)
            .await
            .map_err(|_| ClusterError::InvalidVersion(cluster.mc_version.clone()))?;

    let loader_version = get_loader_version(
        metadata,
        &state.services,
        &mc_version,
        cluster.mc_loader,
        cluster.mc_loader_version.as_deref(),
    )
    .await?;

    let mut version_info = download_version_info(
        &state.services,
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
        JavaManager::java_for_profile(&state.services.db, profile.java_path.as_deref()).await?
    {
        runtime
    } else {
        JavaManager::prepare_java_with_services(
            &state.services,
            java_major,
            search_for_java,
            auto_install_java,
        )
        .await?
    };

    download_minecraft(
        &state.services,
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
