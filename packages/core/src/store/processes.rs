use std::collections::HashMap;
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
use crate::api::game::log::censor_line;
use crate::api::processes::ProcessPayload;
use crate::api::proxy::event::{send_event, LauncherEvent};
use crate::error::{DaoError, LauncherResult};
use crate::send_error;
use crate::utils::io::IOError;

use super::credentials::MinecraftCredentials;
use super::State;

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

#[derive(Debug, Serialize)]
pub struct Process {
	pub pid: u32,
	pub started_at: DateTime<Utc>,
	pub cluster_id: u64,
	pub post_hook: Option<String>,
	pub account_id: Uuid,
	#[serde(skip)]
	pub child_type: Arc<RwLock<ChildType>>,
	#[serde(skip)]
	process_loop: Option<JoinHandle<LauncherResult<i32>>>,
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
				.map(|s| s.code().unwrap_or_default()),
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

	pub async fn spawn(
		&self,
		cluster_id: u64,
		creds: MinecraftCredentials,
		settings: &setting_profiles::Model,
		mut command: Command,
	) -> LauncherResult<Arc<RwLock<Process>>> {
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
			if settings.os_extra.as_ref().is_some_and(|s| s.enable_gamemode.is_some_and(|x| x)) {
				if let Err(err) = onelauncher_gamemode::request_start_for_wrapper(pid) {
					tracing::warn!("failed to enable gamemode, continuing: {}", err);
				}
			}
		}

		let account_id = creds.id;
		tokio::spawn(async move {
			let mut stdout = BufReader::new(stdout).lines();
			let mut stderr = BufReader::new(stderr).lines();

			while let Ok(Some(line)) = tokio::select! {
				line = stdout.next_line() => line,
				line = stderr.next_line() => line,
			} {
				let censored = censor_line(&creds, line);

				send_event(LauncherEvent::Process(ProcessPayload::Output {
					pid,
					output: censored,
				})).await;
			}
		});

		let child_type = Arc::new(RwLock::new(ChildType::new(child)));
		let process = Process {
			pid,
			started_at,
			cluster_id,
			post_hook: settings.hook_post.clone(),
			account_id,
			child_type: child_type.clone(),
			process_loop: Some(tokio::task::spawn(Self::process_loop(
				cluster_id, settings.hook_post.clone(), child_type,
			))),
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
		cluster_id: u64,
		post_hook: Option<String>,
		child: Arc<RwLock<ChildType>>,
	) -> LauncherResult<i32> {
		let pid = child.read().await.id();
		let mut last_updated = Utc::now();

		let exit_code = loop {
			let mut child = child.write().await;
			if let Some(status) = child.try_wait()? {
				tracing::info!("process exited with status: {:?}", status);
				break status;
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
		};

		let state = State::get().await?;
		let store = &state.processes;

		let mut write_guard = store.processes.write().await;

		write_guard.remove(&pid);
		tracing::debug!("removed process with pid {}", pid);

		if let Some(post_hook) = post_hook {
			if !post_hook.is_empty() {
				let result: LauncherResult<()> = {
					let cluster = cluster::dao::get_cluster_by_id(cluster_id)
					.await?
					.ok_or(DaoError::NotFound)?;

					if let Err(err) = run_hook(&post_hook, &cluster.path).await {
						send_error!("{err:?}");
					}

					Ok(())
				};

				if let Err(err) = result {
					send_error!("failed to run post hook: {}", err);
				}
			}
		}

		send_event(LauncherEvent::Process(ProcessPayload::Stopped { pid, exit_code })).await;

		Ok(exit_code)
	}
}

pub async fn run_hook(hook: &str, cwd: &str) -> LauncherResult<Option<i32>> {
	let mut split = hook.split(' ');

	let Some(command) = split.next() else {
		return Ok(None);
	};

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