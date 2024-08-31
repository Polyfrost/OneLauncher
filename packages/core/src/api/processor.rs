//! Process management interface

pub use crate::store::{Cluster, InitHooks, JavaOptions, Memory, Resolution, Settings, State};
use crate::store::{ClusterPath, ProcessorChild};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DetailedProcess {
	pub uuid: Uuid,
	pub user: Option<Uuid>,
	pub started_at: DateTime<Utc>,
	pub pid: u32,
}

impl DetailedProcess {
	pub async fn from_processor_child(process: &ProcessorChild) -> Self {
		let pid = process.current_child.read().await.id().unwrap_or(0);
		Self {
			uuid: process.uuid,
			user: process.user,
			started_at: process.started_at,
			pid,
		}
	}
}

/// get detailed processes by a [`ClusterPath`].
#[tracing::instrument]
pub async fn get_process_detailed_by_id(uuid: Uuid) -> crate::Result<DetailedProcess> {
	let state = State::get().await?;
	let processor = state.processor.read().await;

	let child = processor
		.get(uuid)
		.ok_or(anyhow::anyhow!("process not found"))?;
	let child = child.read().await;
	let pid = child
		.current_child
		.read()
		.await
		.id()
		.ok_or(anyhow::anyhow!("process not found"))?;
	Ok(DetailedProcess {
		uuid,
		user: child.user,
		started_at: child.started_at,
		pid,
	})
}

/// get detailed processes by a [`ClusterPath`].
#[tracing::instrument]
pub async fn get_processes_detailed_by_path(
	path: ClusterPath,
) -> crate::Result<Vec<DetailedProcess>> {
	let state = State::get().await?;
	let processor = state.processor.read().await;

	let uuids = processor.running_cluster(path).await?;
	let mut processes = Vec::new();
	for uuid in uuids {
		let child = processor
			.get(uuid)
			.ok_or(anyhow::anyhow!("process not found"))?;
		let child = child.read().await;
		let pid = child
			.current_child
			.read()
			.await
			.id()
			.ok_or(anyhow::anyhow!("process not found"))?;
		processes.push(DetailedProcess {
			uuid,
			user: child.user,
			started_at: child.started_at,
			pid,
		});
	}
	Ok(processes)
}

/// check whether or not a process has completed by its [`Uuid`].
#[tracing::instrument]
pub async fn uuid_is_finished(uuid: Uuid) -> crate::Result<bool> {
	Ok(uuid_exit_status(uuid).await?.is_some())
}

/// get the user from a processor by its [`Uuid`].
#[tracing::instrument]
pub async fn get_user_by_process(uuid: Uuid) -> crate::Result<Option<Uuid>> {
	let state = State::get().await?;
	let processor = state.processor.read().await;
	if let Some(processor) = processor.get(uuid) {
		Ok(processor.read().await.user)
	} else {
		Err(anyhow::anyhow!("process not found").into())
	}
}

/// get the user from a processor by its [`Uuid`].
#[tracing::instrument]
pub async fn get_process_started_at(uuid: Uuid) -> crate::Result<DateTime<Utc>> {
	let state = State::get().await?;
	let processor = state.processor.read().await;
	if let Some(processor) = processor.get(uuid) {
		Ok(processor.read().await.started_at)
	} else {
		Err(anyhow::anyhow!("process not found").into())
	}
}

/// check the exit status of a process by its [`Uuid`].
#[tracing::instrument]
pub async fn uuid_exit_status(uuid: Uuid) -> crate::Result<Option<i32>> {
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
pub async fn get_running_cluster_paths() -> crate::Result<Vec<ClusterPath>> {
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

/// check if a cluster is running by its [`Uuid`].
#[tracing::instrument]
pub async fn is_cluster_running(uuid: Uuid) -> crate::Result<bool> {
	let clusters = get_running_clusters().await?;
	Ok(clusters.iter().any(|cluster| cluster.uuid == uuid))
}

/// get all processes by a [`ClusterPath`].
#[tracing::instrument]
pub async fn get_uuids_by_cluster_path(cluster_path: ClusterPath) -> crate::Result<Vec<Uuid>> {
	let state = State::get().await?;
	let processor = state.processor.read().await;
	processor.running_cluster(cluster_path).await
}

/// kill an existing and running process by its [`Uuid`]
#[tracing::instrument]
pub async fn kill_by_uuid(uuid: Uuid) -> crate::Result<()> {
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
pub async fn wait_for_by_uuid(uuid: Uuid) -> crate::Result<()> {
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
		.ok_or_else(|| anyhow::anyhow!("manager already completed for process {}", process.uuid))?
		.await?
		.map_err(|err| anyhow::anyhow!("failed to run minecraft: {err}"))?;

	Ok(())
}

///  get process pid by its [`Uuid`].
#[tracing::instrument]
pub async fn get_pid_by_uuid(uuid: Uuid) -> crate::Result<u32> {
	let state = State::get().await?;
	let processor = state.processor.read().await;
	Ok(processor
		.get(uuid)
		.ok_or(anyhow::anyhow!("process not found"))?
		.read()
		.await
		.current_child
		.read()
		.await
		.id()
		.ok_or(anyhow::anyhow!("process not found"))?)
}
