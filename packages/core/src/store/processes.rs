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
use crate::api::processes::{ProcessPayload, ProcessPayloadKind};
use crate::api::proxy::event::{LauncherEvent, send_event};
use crate::error::LauncherResult;
use crate::send_error;
use crate::utils::io::IOError;

use super::State;
use super::credentials::MinecraftCredentials;

const PROCESS_OUTPUT_FLUSH_MS: u64 = 50;
const PROCESS_OUTPUT_BATCH_MAX_LINES: usize = 128;
const PROCESS_OUTPUT_BATCH_MAX_CHARS: usize = 16 * 1024;

#[onelauncher_macro::error]
#[derive(Debug, thiserror::Error)]
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
		let processes = {
			let read_guard = self.processes.read().await;
			read_guard
				.iter()
				.map(|(pid, process)| (*pid, process.clone()))
				.collect::<Vec<_>>()
		};

		let mut stale_pids = Vec::new();
		let mut running = false;

		for (pid, process) in processes {
			let (process_cluster_id, process_loop_finished, child_type) = {
				let process_guard = process.read().await;
				(
					process_guard.cluster_id,
					process_guard
						.process_loop
						.as_ref()
						.is_none_or(|handle| handle.is_finished()),
					process_guard.child_type.clone(),
				)
			};
			let is_target_cluster = process_cluster_id == cluster_id;

			if !is_target_cluster {
				continue;
			}

			if process_loop_finished {
				stale_pids.push(pid);
				continue;
			}

			let stopped = {
				let mut child = child_type.write().await;
				match child.try_wait() {
					Ok(Some(_)) => true,
					Ok(None) => {
						let exists = process_exists_os(pid);
						!exists
					}
					Err(err) => {
						tracing::warn!(
							pid,
							cluster_id,
							?err,
							"failed to poll process in has_running; removing stale process",
						);
						true
					}
				}
			};

			if stopped {
				stale_pids.push(pid);
				continue;
			}

			running = true;
		}

		if !stale_pids.is_empty() {
			let mut write_guard = self.processes.write().await;
			for pid in &stale_pids {
				write_guard.remove(pid);
			}
		}

		running
	}

	/// Spawns a new process (Minecraft Instance)
	#[allow(clippy::too_many_lines)]
	pub async fn spawn(
		&self,
		cluster_id: ClusterId,
		cwd: PathBuf,
		creds: MinecraftCredentials,
		censors: CensorMap,
		settings: &setting_profiles::Model,
		mut command: Command,
	) -> LauncherResult<Arc<RwLock<Process>>> {
		send_event(LauncherEvent::Process(ProcessPayload {
			cluster_id,
			kind: ProcessPayloadKind::Starting {
				command: format!("{command:?}"),
			},
		}))
		.await;

		if let Some(pre_hook) = &settings.hook_pre
			&& let Err(err) = run_hook(pre_hook, cwd.clone()).await
		{
			send_error!("{err:?}");
		}

		command
			.stdout(Stdio::piped())
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
			let mut stdout_closed = false;
			let mut stderr_closed = false;
			let mut pending_lines = Vec::with_capacity(PROCESS_OUTPUT_BATCH_MAX_LINES);
			let mut pending_chars = 0usize;

			let mut flush_tick =
				tokio::time::interval(std::time::Duration::from_millis(PROCESS_OUTPUT_FLUSH_MS));
			flush_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			loop {
				if stdout_closed && stderr_closed {
					break;
				}

				tokio::select! {
					line = stdout.next_line(), if !stdout_closed => {
						match line {
							Ok(Some(mut line)) => {
								censor_line(&censors, &mut line);
								pending_chars += line.len();
								pending_lines.push(line);
							}
							Ok(None) => {
								stdout_closed = true;
							}
							Err(err) => {
								stdout_closed = true;
								tracing::warn!("stdout read failed for pid {}: {}", pid, err);
							}
						}
					}

					line = stderr.next_line(), if !stderr_closed => {
						match line {
							Ok(Some(mut line)) => {
								censor_line(&censors, &mut line);
								pending_chars += line.len();
								pending_lines.push(line);
							}
							Ok(None) => {
								stderr_closed = true;
							}
							Err(err) => {
								stderr_closed = true;
								tracing::warn!("stderr read failed for pid {}: {}", pid, err);
							}
						}
					}

					_ = flush_tick.tick(), if !pending_lines.is_empty() => {
						flush_process_output_event(cluster_id, pid, &mut pending_lines, &mut pending_chars).await;
					}
				}

				if pending_lines.len() >= PROCESS_OUTPUT_BATCH_MAX_LINES
					|| pending_chars >= PROCESS_OUTPUT_BATCH_MAX_CHARS
				{
					flush_process_output_event(
						cluster_id,
						pid,
						&mut pending_lines,
						&mut pending_chars,
					)
					.await;
				}
			}

			flush_process_output_event(cluster_id, pid, &mut pending_lines, &mut pending_chars)
				.await;
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

		send_event(LauncherEvent::Process(ProcessPayload {
			cluster_id,
			kind: ProcessPayloadKind::Started {
				pid,
				started_at,
				post_hook: settings.hook_post.clone(),
				account_id: creds.id,
			},
		}))
		.await;

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

		let exit_code = match wait_for_exit(cluster_id, &child).await {
			Ok(exit_code) => exit_code,
			Err(err) => {
				tracing::warn!(
					pid,
					cluster_id,
					?err,
					"failed while waiting for process exit; marking process as stopped",
				);
				-1
			}
		};

		let state = State::get().await?;
		let store = &state.processes;

		{
			let mut write_guard = store.processes.write().await;
			write_guard.remove(&pid);
		}
		tracing::debug!("removed process with pid {}", pid);

		if let Some(post_hook) = post_hook
			&& let Err(err) = run_hook(&post_hook, cwd).await
		{
			send_error!("{err:?}");
		}

		send_event(LauncherEvent::Process(ProcessPayload {
			cluster_id,
			kind: ProcessPayloadKind::Stopped { pid, exit_code },
		}))
		.await;

		Ok(exit_code)
	}
}

async fn flush_process_output_event(
	cluster_id: ClusterId,
	pid: u32,
	pending_lines: &mut Vec<String>,
	pending_chars: &mut usize,
) -> usize {
	if pending_lines.is_empty() {
		return 0;
	}

	let emitted_lines = pending_lines.len();
	let output = pending_lines.join("\n");
	pending_lines.clear();
	*pending_chars = 0;

	send_event(LauncherEvent::Process(ProcessPayload {
		cluster_id,
		kind: ProcessPayloadKind::Output { pid, output },
	}))
	.await;

	emitted_lines
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

async fn wait_for_exit(
	cluster_id: ClusterId,
	child: &Arc<RwLock<ChildType>>,
) -> LauncherResult<i32> {
	let pid = child.read().await.id();

	let mut last_updated = Utc::now();

	loop {
		let exit_status = {
			let mut child = child.write().await;
			child.try_wait()
		};

		match exit_status {
			Ok(Some(status)) => {
				tracing::info!("process exited with status: {:?}", status);
				return Ok(status);
			}
			Ok(None) => {
				if !process_exists_os(pid) {
					return Ok(-1);
				}
			}
			Err(err) => {
				tracing::warn!(
					cluster_id,
					?err,
					"failed to poll process status; treating as already stopped",
				);
				return Ok(-1);
			}
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

#[cfg(unix)]
fn process_exists_os(pid: u32) -> bool {
	// SAFETY: `kill` with signal 0 performs existence/permission checks only and
	// does not deliver a signal.
	#[allow(clippy::cast_possible_wrap)]
	let rc = unsafe { libc::kill(pid as i32, 0) };
	if rc == 0 {
		return true;
	}

	let err = std::io::Error::last_os_error()
		.raw_os_error()
		.unwrap_or_default();
	err == libc::EPERM
}

#[cfg(windows)]
fn process_exists_os(pid: u32) -> bool {
	let pid = sysinfo::Pid::from_u32(pid);
	let mut system = sysinfo::System::new();

	system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
	system.process(pid).is_some()
}

#[cfg(not(any(unix, windows)))]
fn process_exists_os(pid: u32) -> bool {
	let _ = pid;
	false
}
