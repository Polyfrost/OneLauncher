use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::api::cluster::dao::ClusterId;
use crate::error::LauncherResult;
use crate::store::State;
use crate::store::processes::Process;

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Serialize)]
pub struct ProcessPayload {
	pub cluster_id: ClusterId,
	pub kind: ProcessPayloadKind,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ProcessPayloadKind {
	Starting {
		command: String,
	},
	Started {
		pid: u32,
		started_at: DateTime<Utc>,
		post_hook: Option<String>,
		account_id: Uuid,
	},
	Stopped {
		pid: u32,
		exit_code: i32,
	},
	Output {
		pid: u32,
		output: String,
	},
}

pub async fn get_running_processes() -> LauncherResult<Vec<Process>> {
	let state = State::get().await?;
	let running = state.processes.get_running().await;

	let processes = futures::future::join_all(
		running
			.into_iter()
			.map(|process| async move { process.read().await.clone() }),
	)
	.await;

	Ok(processes)
}

pub async fn get_running_processes_by_cluster_id(
	cluster_id: ClusterId,
) -> LauncherResult<Vec<Process>> {
	let state = State::get().await?;
	let running = state.processes.get_running_by_cluster(cluster_id).await;

	let processes = futures::future::join_all(
		running
			.into_iter()
			.map(|process| async move { process.read().await.clone() }),
	)
	.await;

	Ok(processes)
}

pub async fn is_cluster_running(cluster_id: ClusterId) -> LauncherResult<bool> {
	let state = State::get().await?;
	Ok(state.processes.has_running(cluster_id).await)
}

pub async fn kill_process(pid: u32) -> LauncherResult<()> {
	let state = State::get().await?;
	if let Some(process) = state.processes.get(pid).await {
		process.read().await.child_type.write().await.kill().await?;
	}

	Ok(())
}
