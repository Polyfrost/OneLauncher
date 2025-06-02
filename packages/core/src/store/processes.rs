use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use onelauncher_entity::setting_profiles;
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::api::cluster;
use crate::api::cluster::dao::ClusterId;
use crate::api::game::log::{CensorMap, censor_line};
use crate::api::processes::ProcessPayload;
use crate::api::proxy::event::{LauncherEvent, send_event};
use crate::error::LauncherResult;
use crate::send_error;
use crate::utils::io::IOError;

use super::State;
use super::credentials::MinecraftCredentials;

#[onelauncher_macro::specta]
#[derive(Debug, thiserror::Error, Serialize)]
pub enum ProcessError {
	#[error("launch hook exited with non-zero code {0}")]
	HookUnsuccessful(i32),
	#[error("pid equaled nothing")]
	NoPid,
}

#[derive(Default)]
pub struct ProcessStore {
	processes: RwLock<HashMap<u32, Arc<RwLock<Process>>>>,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Serialize)]
pub struct Process {
	pub pid: u32,
	pub started_at: DateTime<Utc>,
	pub cluster_id: ClusterId,
	pub post_hook: Option<String>,
	pub account_id: Uuid,
	#[serde(skip)]
	pub child_type: Arc<RwLock<ChildType>>,
	#[serde(skip)]
	process_loop: Option<Arc<JoinHandle<LauncherResult<i32>>>>,
}

#[derive(Debug, Serialize)]
pub enum ChildType {
	Owned(#[serde(skip)] Child),
}

impl Process {
	#[must_use]
	pub const fn is_running(&self) -> bool {
		self.process_loop.is_some()
	}
}

impl ChildType {
	#[must_use]
	pub const fn new(child: Child) -> Self {
		Self::Owned(child)
	}

	#[must_use]
	pub fn id(&self) -> u32 {
		match self {
			Self::Owned(child) => child.id().unwrap_or(0),
		}
	}

	// Attempts to return its exit code, if it has exited.
	pub fn try_wait(&mut self) -> LauncherResult<Option<i32>> {
		Ok(match self {
			Self::Owned(child) => child
				.try_wait()
				.map_err(IOError::from)?
				.and_then(|s| s.code()),
		})
	}

	// async blocking to wait for the process to finish and return exit code
	pub async fn wait(&mut self) -> LauncherResult<i32> {
		Ok(match self {
			Self::Owned(child) => child
				.wait()
				.await
				.map(|s| s.code().unwrap_or_default())
				.map_err(IOError::from)?,
		})
	}

	pub async fn kill(&mut self) -> LauncherResult<()> {
		match self {
			Self::Owned(child) => child.kill().await.map_err(IOError::from)?,
		}

		Ok(())
	}
}

impl ProcessStore {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns a list of all running processes for a given cluster.
	pub async fn get_running_by_cluster(&self, cluster_id: ClusterId) -> Vec<Arc<RwLock<Process>>> {
		let mut list = Vec::new();
		let read_guard = self.processes.read().await;

		for process in read_guard.values() {
			if process.read().await.cluster_id == cluster_id {
				list.push(process.clone());
			}
		}

		list
	}

	/// Returns a list of all running processes for a given account.
	pub async fn get_running_by_account(&self, account_id: Uuid) -> Vec<Arc<RwLock<Process>>> {
		let mut list = Vec::new();
		let read_guard = self.processes.read().await;

		for process in read_guard.values() {
			if process.read().await.account_id == account_id {
				list.push(process.clone());
			}
		}

		list
	}

	/// Returns a list of all running processes.
	pub async fn get_running(&self) -> Vec<Arc<RwLock<Process>>> {
		let mut list = Vec::new();
		let read_guard = self.processes.read().await;

		for process in read_guard.values() {
			list.push(process.clone());
		}

		list
	}

	/// Returns whether a cluster has a running process.
	pub async fn has_running(&self, cluster_id: ClusterId) -> bool {
		let read_guard = self.processes.read().await;

		for process in read_guard.values() {
			if process.read().await.cluster_id == cluster_id {
				return true;
			}
		}

		false
	}

	/// Spawns a new process (Minecraft Instance)
	pub async fn spawn(
		&self,
		cluster_id: ClusterId,
		cwd: PathBuf,
		creds: MinecraftCredentials,
		censors: CensorMap,
		settings: &setting_profiles::Model,
		mut command: Command,
	) -> LauncherResult<Arc<RwLock<Process>>> {
		send_event(LauncherEvent::Process(ProcessPayload::Starting {
			command: format!("{command:?}"),
		}))
		.await;

		if let Some(pre_hook) = &settings.hook_pre
			&& let Err(err) = run_hook(pre_hook, cwd.clone()).await
		{
			send_error!("{err:?}");
		}

		command.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.stdin(Stdio::null());

		let mut child = command.spawn().map_err(IOError::from)?;

		let started_at = Utc::now();
		let Some(pid) = child.id() else {
			return Err(ProcessError::NoPid.into());
		};

		let stdout = child.stdout.take().expect("couldn't get stdout of child");
		let stderr = child.stderr.take().expect("couldn't get stderr of child");

		#[cfg(target_os = "linux")]
		{
			if settings
				.os_extra
				.as_ref()
				.is_some_and(|s| s.enable_gamemode.is_some_and(|x| x))
				&& let Err(err) = onelauncher_gamemode::request_start_for_wrapper(pid)
			{
				tracing::warn!("failed to enable gamemode, continuing: {}", err);
			}
		}

		tokio::spawn(async move {
			let mut stdout = BufReader::new(stdout).lines();
			let mut stderr = BufReader::new(stderr).lines();

			while let Ok(Some(mut line)) = tokio::select! {
				line = stdout.next_line() => line,
				line = stderr.next_line() => line,
			} {
				censor_line(&censors, &mut line);

				send_event(LauncherEvent::Process(ProcessPayload::Output {
					pid,
					output: line,
				}))
				.await;
			}
		});

		let child_type = Arc::new(RwLock::new(ChildType::new(child)));
		let process = Process {
			pid,
			started_at,
			cluster_id,
			post_hook: settings.hook_post.clone(),
			account_id: creds.id,
			child_type: child_type.clone(),
			process_loop: Some(Arc::new(tokio::task::spawn(Self::process_loop(
				cluster_id,
				cwd,
				settings.hook_post.clone(),
				child_type,
			)))),
		};

		let process = Arc::new(RwLock::new(process));
		let mut write_guard = self.processes.write().await;
		write_guard.insert(pid, process.clone());
		tracing::info!("spawned process with pid {}", pid);

		Ok(process)
	}

	pub async fn get(&self, pid: u32) -> Option<Arc<RwLock<Process>>> {
		let read_guard = self.processes.read().await;
		read_guard.get(&pid).cloned()
	}

	async fn process_loop(
		cluster_id: ClusterId,
		cwd: PathBuf,
		post_hook: Option<String>,
		child: Arc<RwLock<ChildType>>,
	) -> LauncherResult<i32> {
		let pid = child.read().await.id();

		let exit_code = wait_for_exit(cluster_id, &child).await?;

		let state = State::get().await?;
		let store = &state.processes;

		let mut write_guard = store.processes.write().await;

		write_guard.remove(&pid);
		tracing::debug!("removed process with pid {}", pid);

		if let Some(post_hook) = post_hook
			&& let Err(err) = run_hook(&post_hook, cwd).await
		{
			send_error!("{err:?}");
		}

		send_event(LauncherEvent::Process(ProcessPayload::Stopped {
			pid,
			exit_code,
		}))
		.await;

		Ok(exit_code)
	}
}

async fn run_hook(hook: &str, cwd: PathBuf) -> LauncherResult<Option<i32>> {
	let mut split = hook.split(' ');

	let Some(command) = split.next() else {
		return Ok(None);
	};

	tracing::debug!("running hook");
	let result = Command::new(command)
		.args(split)
		.current_dir(cwd)
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.map_err(IOError::from)?
		.wait()
		.await
		.map_err(IOError::from)?;

	let exit_code = result
		.code()
		.unwrap_or_else(|| i32::from(!result.success()));

	if result.success() {
		Ok(Some(exit_code))
	} else {
		Err(ProcessError::HookUnsuccessful(exit_code).into())
	}
}

async fn wait_for_exit(cluster_id: ClusterId, child: &Arc<RwLock<ChildType>>) -> LauncherResult<i32> {
	let mut last_updated = Utc::now();

	loop {
		let mut child = child.write().await;
		if let Some(status) = child.try_wait()? {
			tracing::info!("process exited with status: {:?}", status);
			return Ok(status);
		}

		// Every 60 seconds, update the playtime of the process
		let duration = Utc::now().signed_duration_since(last_updated).num_seconds();
		if duration > 60 {
			last_updated = Utc::now();

			if let Err(err) = cluster::update_playtime(cluster_id, duration).await {
				tracing::error!("failed to update playtime: {}", err);
			}
		}

		tokio::time::sleep(std::time::Duration::from_millis(100)).await;
	}
}
