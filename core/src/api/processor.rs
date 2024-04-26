//! Process management interface

use uuid::Uuid;
use crate::store::{ProcessorChild, ClusterPath};
pub use crate::store::{InitHooks, JavaOptions, Memory, Cluster, Settings, Resolution, State};

/// check whether or not a process has completed by its [`Uuid`].
#[tracing::instrument]
pub async fn uuid_is_finished(uuid: &Uuid) -> crate::Result<bool> {
    Ok(uuid_exit_status(uuid).await?.is_some())
}

/// check the exit status of a process by its [`Uuid`].
#[tracing::instrument]
pub async fn uuid_exit_status(uuid: &Uuid) -> crate::Result<Option<i32>> {
    let state = State::get().await?;
    let processor = state.processor.read().await;
    processor.exit_status(uuid).await
}

/// get all process [`Uuid`]s.
#[tracing::instrument]
pub async fn get_uuids() -> crate::Result<Vec<Uuid>> {
    let state = State::get().await?;
    let processor = state.processor.read().await;
    Ok(processor.keys())
}

/// get all running process [`Uuid`]s.
#[tracing::instrument]
pub async fn get_running() -> crate::Result<Vec<Uuid>> {
    let state = State::get().await?;
    let processor = state.processor.read().await;
    processor.running().await
}

/// get all running process' [`ClusterPath`]s.
#[tracing::instrument]
pub async fn get_running_cluster_paths() -> crate::Result<Vec<ClusterPath>>
{
    let state = State::get().await?;
    let processor = state.processor.read().await;
    processor.running_cluster_paths().await
}

/// get all running process' [`Cluster`]s.
#[tracing::instrument]
pub async fn get_running_clusters() -> crate::Result<Vec<Cluster>> {
    let state = State::get().await?;
    let processor = state.processor.read().await;
    processor.running_clusters().await
}

/// get all processes by a [`ClusterPath`].
#[tracing::instrument]
pub async fn get_uuids_by_cluster_path(
    cluster_path: ClusterPath,
) -> crate::Result<Vec<Uuid>> {
    let state = State::get().await?;
    let processor = state.processor.read().await;
    processor.running_cluster(cluster_path).await
}

/// kill an existing and running process by its [`Uuid`]
#[tracing::instrument]
pub async fn kill_by_uuid(uuid: &Uuid) -> crate::Result<()> {
    let state = State::get().await?;
    let processor = state.processor.read().await;

    if let Some(process) = processor.get(uuid) {
        let mut process = process.write().await;
        kill(&mut process).await
    } else {
        Ok(())
    }
}

/// wait for a child process to finish running its manager by its [`Uuid`].
#[tracing::instrument]
pub async fn wait_for_by_uuid(uuid: &Uuid) -> crate::Result<()> {
    let state = State::get().await?;
    let processor = state.processor.read().await;

    if let Some(process) = processor.get(uuid) {
        let mut process = process.write().await;
        wait_for(&mut process).await
    } else {
        Ok(())
    }
}

/// kill an existing and running process
#[tracing::instrument(skip(process))]
pub async fn kill(process: &mut ProcessorChild) -> crate::Result<()> {
    process.current_child.write().await.kill().await?;
    Ok(())
}

/// wait for a child process to finish running its manager
#[tracing::instrument(skip(process))]
pub async fn wait_for(process: &mut ProcessorChild) -> crate::Result<()> {
    process
        .manager
        .take()
        .ok_or_else(||anyhow::anyhow!("manager already completed for process {}", process.uuid))?
        .await?
        .map_err(|err| anyhow::anyhow!("failed to run minecraft: {err}"))?;

    Ok(())
}
