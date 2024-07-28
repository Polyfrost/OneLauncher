//! Handles child processes in a unified API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::api::cluster;
use crate::constants::PROCESSOR_FILE;
use crate::proxy::send::send_process;
use crate::proxy::ProcessPayloadType;
use crate::utils::http::read_json;
use crate::utils::io::IOError;
use crate::State;

use super::{Cluster, ClusterPath};

/// Wrapper over a HashMap of PIDs to ProcessorChildren and unified apis
pub struct Processor(HashMap<Uuid, Arc<RwLock<ProcessorChild>>>);

/// Manager for safe processes like ingress feeds which need to be safely shut down.
pub struct IngressProcessor {
	/// A vector of ingress feed uuids.
	pub ingress_feeds: Vec<Uuid>,
}

/// A type of Ingress safety process.
#[derive(Debug, Copy, Clone)]
pub enum IngressProcessType {
	/// A single Ingress feed.
	IngressFeed,
	// Writes,
	// Reads,
	// Fetch,
}

impl Default for IngressProcessor {
	fn default() -> Self {
		Self::new()
	}
}

impl IngressProcessor {
	/// Initialize a new ingresss process manager.
	pub fn new() -> Self {
		Self {
			ingress_feeds: Vec::new(),
		}
	}

	/// Add an ingress safety process to the HashMap.
	pub async fn add_ingress(r#type: IngressProcessType, uuid: Uuid) -> crate::Result<Uuid> {
		let state = State::get().await?;
		let mut ingress_processor = state.ingress_processor.write().await;
		match r#type {
			IngressProcessType::IngressFeed => {
				ingress_processor.ingress_feeds.push(uuid);
			}
		}

		Ok(uuid)
	}

	/// Complete and retain an ingress safety process from the HashMap of ingress feeds.
	pub async fn finish(r#type: IngressProcessType, uuid: Uuid) -> crate::Result<()> {
		let state = State::get().await?;
		let mut ingress_processor = state.ingress_processor.write().await;
		match r#type {
			IngressProcessType::IngressFeed => {
				ingress_processor.ingress_feeds.retain(|f| *f != uuid);
			}
		}
		Ok(())
	}

	/// Finish and check if an ingress safety processes ingress feed is empty and finished.
	pub async fn finished(r#type: IngressProcessType) -> crate::Result<bool> {
		let state = State::get().await?;
		let ingress_processor = state.ingress_processor.read().await;
		match r#type {
			IngressProcessType::IngressFeed => {
				if ingress_processor.ingress_feeds.is_empty() {
					return Ok(true);
				}
			}
		}

		Ok(false)
	}
}

/// A child process type.
#[derive(Debug)]
pub enum ChildType {
	/// A tokio-managed child process.
	ChildProcess(Child),
	/// A restored child from a process cache that needs to be re-registered.
	RescuedChild(u32),
}

/// A structure representing a cache of a running process.
#[derive(Serialize, Deserialize, Debug)]
pub struct ProcessorCache {
	/// The PID of a process.
	pub pid: u32,
	/// The stored UUID of a process.
	pub uuid: Uuid,
	/// The time a process started.
	pub start_time: u64,
	/// The name of a process.
	pub name: String,
	/// The executable file running the process.
	pub exe: String,
	/// The associated [`ClusterPath`] to a process.
	pub cluster_path: ClusterPath,
	/// A post hook to be run once a process has completed.
	pub post: Option<String>,
}

impl ChildType {
	/// Get the PID of a specific ChildType process.
	pub fn id(&self) -> Option<u32> {
		match self {
			ChildType::ChildProcess(child) => child.id(),
			ChildType::RescuedChild(pid) => Some(*pid),
		}
	}

	/// Store a cache of a specific ChildType.
	pub async fn cache(
		&self,
		uuid: Uuid,
		cluster_path: ClusterPath,
		post: Option<String>,
	) -> crate::Result<()> {
		let pid = match self {
			ChildType::ChildProcess(child) => child.id().unwrap_or(0),
			ChildType::RescuedChild(pid) => *pid,
		};

		let state = crate::State::get().await?;
		let mut system = sysinfo::System::new();

		system.refresh_processes();
		let process = system
			.process(sysinfo::Pid::from_u32(pid))
			.ok_or_else(|| anyhow::anyhow!("could not find process {}", pid))?;
		let start_time = process.start_time();
		let name = process.name().to_string();
		let Some(path) = process.exe() else {
			return Err(anyhow::anyhow!("cached process {} has no path", pid).into());
		};

		let exe = path.to_string_lossy().to_string();
		let cached_proccess = ProcessorCache {
			pid,
			uuid,
			start_time,
			name,
			exe,
			cluster_path,
			post,
		};

		let path = state.directories.caches_dir().await.join(PROCESSOR_FILE);
		let mut caches = if let Ok(proc_json) =
			read_json::<HashMap<Uuid, ProcessorCache>>(&path, &state.io_semaphore).await
		{
			proc_json
		} else {
			HashMap::new()
		};

		caches.insert(uuid, cached_proccess);
		crate::utils::http::write(&path, &serde_json::to_vec(&caches)?, &state.io_semaphore)
			.await?;

		Ok(())
	}

	/// Remove a child from the global store of processes.
	pub async fn remove(&self, uuid: Uuid) -> crate::Result<()> {
		let state = crate::State::get().await?;
		let path = state.directories.caches_dir().await.join(PROCESSOR_FILE);
		let mut caches = if let Ok(proc_json) =
			read_json::<HashMap<Uuid, ProcessorCache>>(&path, &state.io_semaphore).await
		{
			proc_json
		} else {
			HashMap::new()
		};
		caches.remove(&uuid);
		crate::utils::http::write(&path, &serde_json::to_vec(&caches)?, &state.io_semaphore)
			.await?;

		Ok(())
	}

	/// Kill a process in the global store of processes.
	pub async fn kill(&mut self) -> crate::Result<()> {
		match self {
			ChildType::ChildProcess(child) => Ok(child.kill().await.map_err(IOError::from)?),
			ChildType::RescuedChild(pid) => {
				let mut system = sysinfo::System::new();
				if system.refresh_process(sysinfo::Pid::from_u32(*pid)) {
					let process = system.process(sysinfo::Pid::from_u32(*pid));
					if let Some(process) = process {
						process.kill();
					}
				}
				Ok(())
			}
		}
	}

	/// Wait for a process to complete.
	pub async fn try_wait(&mut self) -> crate::Result<Option<i32>> {
		match self {
			ChildType::ChildProcess(child) => Ok(child
				.try_wait()
				.map_err(IOError::from)?
				.map(|x| x.code().unwrap_or(0))),
			ChildType::RescuedChild(pid) => {
				let mut system = sysinfo::System::new();
				if !system.refresh_process(sysinfo::Pid::from_u32(*pid)) {
					return Ok(Some(0));
				}

				let process = system.process(sysinfo::Pid::from_u32(*pid));
				if let Some(process) = process {
					if process.status() == sysinfo::ProcessStatus::Run {
						Ok(None)
					} else {
						Ok(Some(0))
					}
				} else {
					Ok(Some(0))
				}
			}
		}
	}
}

/// A [`Processor`] child process.
#[derive(Debug)]
pub struct ProcessorChild {
	/// The uuid of a process.
	pub uuid: Uuid,
	/// The associated [`ClusterPath`] to a process.
	pub cluster_path: ClusterPath,
	/// The process manager managing this process.
	pub manager: Option<JoinHandle<crate::Result<i32>>>,
	/// The [`ChildType`] of this process.
	pub current_child: Arc<RwLock<ChildType>>,
	/// When this process was last updated in [`Utc`].
	pub last_updated: DateTime<Utc>,
}

impl Processor {
	/// Get a new [`Processor`] instance.
	pub fn new() -> Self {
		Processor(HashMap::new())
	}

	/// Returns a reference to the value corresponding to the key.
	pub fn get(&self, uuid: Uuid) -> Option<Arc<RwLock<ProcessorChild>>> {
		self.0.get(&uuid).cloned()
	}

	/// An collection visiting all keys in arbitrary order.
	pub fn keys(&self) -> Vec<Uuid> {
		self.0.keys().cloned().collect()
	}

	/// Get all running processes.
	pub async fn running(&self) -> crate::Result<Vec<Uuid>> {
		let mut keys = Vec::new();
		for key in self.keys() {
			if let Some(process) = self.get(key) {
				let process = process.clone();
				let process = process.write().await;
				if process
					.current_child
					.write()
					.await
					.try_wait()
					.await?
					.is_none()
				{
					keys.push(key);
				}
			}
		}
		Ok(keys)
	}

	/// Get all running cluster processes.
	pub async fn running_cluster(&self, cluster_path: ClusterPath) -> crate::Result<Vec<Uuid>> {
		let running = self.running().await?;
		let mut keys = Vec::new();
		for key in running {
			if let Some(process) = self.get(key) {
				let process = process.clone();
				let process = process.read().await;
				if process.cluster_path == cluster_path {
					keys.push(key);
				}
			}
		}
		Ok(keys)
	}

	/// get all running cluster paths.
	pub async fn running_cluster_paths(&self) -> crate::Result<Vec<ClusterPath>> {
		let mut clusters = Vec::new();
		for key in self.keys() {
			if let Some(process) = self.get(key) {
				let process = process.clone();
				let process = process.write().await;
				if process
					.current_child
					.write()
					.await
					.try_wait()
					.await?
					.is_none()
				{
					clusters.push(process.cluster_path.clone());
				}
			}
		}
		Ok(clusters)
	}

	/// get all running clusters.
	pub async fn running_clusters(&self) -> crate::Result<Vec<Cluster>> {
		let mut clusters = Vec::new();
		for key in self.keys() {
			if let Some(process) = self.get(key) {
				let process = process.clone();
				let process = process.write().await;
				if process
					.current_child
					.write()
					.await
					.try_wait()
					.await?
					.is_none()
				{
					if let Some(cluster) = cluster::get(&process.cluster_path.clone(), None).await?
					{
						clusters.push(cluster);
					}
				}
			}
		}
		Ok(clusters)
	}

	/// get the exit status of a process by its [`Uuid`].
	pub async fn exit_status(&self, uuid: Uuid) -> crate::Result<Option<i32>> {
		if let Some(process) = self.get(uuid) {
			let process = process.write().await;
			let status = process.current_child.write().await.try_wait().await?;
			Ok(status)
		} else {
			Ok(None)
		}
	}

	/// restore processes from the cache.
	pub async fn restore(&mut self) -> crate::Result<()> {
		let state = crate::State::get().await?;
		let processor_path = state.directories.caches_dir().await.join(PROCESSOR_FILE);
		let mut processor_caches = if let Ok(processes_json) =
			read_json::<HashMap<uuid::Uuid, ProcessorCache>>(&processor_path, &state.io_semaphore)
				.await
		{
			let zeros = HashMap::<uuid::Uuid, ProcessorCache>::new();
			crate::utils::http::write(
				&processor_path,
				&serde_json::to_vec(&zeros)?,
				&state.io_semaphore,
			)
			.await?;
			processes_json
		} else {
			HashMap::new()
		};

		for (_, cache) in processor_caches.drain() {
			let uuid = cache.uuid;
			match self.insert_restored(cache).await {
				Ok(process) => {
					self.0.insert(uuid, process);
				}
				Err(e) => tracing::warn!("failed to restore process {}: {}", uuid, e),
			}
		}

		Ok(())
	}

	/// insert a process with a command and [`ClusterPath`].
	#[tracing::instrument(skip(self, uuid, command, post, censors))]
	#[tracing::instrument(level = "trace", skip(self))]
	#[onelauncher_macros::memory]
	pub async fn insert_process(
		&mut self,
		uuid: Uuid,
		cluster_path: ClusterPath,
		mut command: Command,
		post: Option<String>,
		censors: HashMap<String, String>,
	) -> crate::Result<Arc<RwLock<ProcessorChild>>> {
		let proc = command.spawn().map_err(IOError::from)?;
		let process = ChildType::ChildProcess(proc);
		let pid = process
			.id()
			.ok_or_else(|| anyhow::anyhow!("process failed, couldn't get pid"))?;
		process
			.cache(uuid, cluster_path.clone(), post.clone())
			.await?;

		let current_child = Arc::new(RwLock::new(process));
		let manager = Some(tokio::spawn(Self::manager(
			uuid,
			post,
			pid,
			current_child.clone(),
			cluster_path.clone(),
		)));

		send_process(uuid, pid, ProcessPayloadType::Started, "started process").await?;
		let last_updated = Utc::now();
		let child = ProcessorChild {
			uuid,
			cluster_path,
			current_child,
			manager,
			last_updated,
		};
		let child = Arc::new(RwLock::new(child));
		self.0.insert(uuid, child.clone());
		Ok(child)
	}

	/// Insert a cached process with a [`ProcessorCache`].
	#[tracing::instrument(skip(self, cache))]
	#[tracing::instrument(level = "trace", skip(self))]
	#[onelauncher_macros::memory]
	pub async fn insert_restored(
		&mut self,
		cache: ProcessorCache,
	) -> crate::Result<Arc<RwLock<ProcessorChild>>> {
		let _state = crate::State::get().await?;

		// ensure located stray process matches with our pid recorded process
		{
			let mut system = sysinfo::System::new();
			system.refresh_processes();
			let process = system
				.process(sysinfo::Pid::from_u32(cache.pid))
				.ok_or_else(|| anyhow::anyhow!("could not find pid {}", cache.pid))?;

			if cache.start_time != process.start_time() {
				return Err(anyhow::anyhow!(
					"restored process {} has a mismatched start time {}",
					cache.pid,
					process.start_time()
				)
				.into());
			}
			if cache.name != process.name() {
				return Err(anyhow::anyhow!(
					"restored process {} has a mismatched name {}",
					cache.pid,
					process.name()
				)
				.into());
			}

			if let Some(path) = process.exe() {
				if cache.exe != path.to_string_lossy() {
					return Err(anyhow::anyhow!(
						"restored process {} has a mismatched exe {}",
						cache.pid,
						path.to_string_lossy()
					)
					.into());
				}
			} else {
				return Err(
					anyhow::anyhow!("restored process {} has no exe path", cache.pid).into(),
				);
			}
		}

		let process = ChildType::RescuedChild(cache.pid);
		let pid = process
			.id()
			.ok_or_else(|| anyhow::anyhow!("process failed, couldnt get pid"))?;
		process
			.cache(cache.uuid, cache.cluster_path.clone(), cache.post.clone())
			.await?;
		let current_child = Arc::new(RwLock::new(process));
		let manager = Some(tokio::spawn(Self::manager(
			cache.uuid,
			cache.post,
			pid,
			current_child.clone(),
			cache.cluster_path.clone(),
		)));

		send_process(
			cache.uuid,
			pid,
			ProcessPayloadType::Started,
			"started process",
		)
		.await?;

		let last_updated = Utc::now();
		let child = ProcessorChild {
			uuid: cache.uuid,
			cluster_path: cache.cluster_path,
			current_child,
			manager,
			last_updated,
		};

		let child = Arc::new(RwLock::new(child));
		self.0.insert(cache.uuid, child.clone());
		Ok(child)
	}

	/// Get a process manager and runner awaiting for the exit status.
	#[tracing::instrument(skip(current_child))]
	#[onelauncher_macros::memory]
	async fn manager(
		uuid: Uuid,
		post: Option<String>,
		mut current_pid: u32,
		current_child: Arc<RwLock<ChildType>>,
		cluster_path: ClusterPath,
	) -> crate::Result<i32> {
		let current_child = current_child.clone();
		let mut exit_status;
		let mut last_updated = Utc::now();

		// core main process loop, managed by tokio
		loop {
			if let Some(stat) = current_child.write().await.try_wait().await? {
				exit_status = stat;
				break;
			}

			tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
			let update = Utc::now().signed_duration_since(last_updated).num_seconds();
			if update >= 60 {
				if let Err(err) = cluster::edit(&cluster_path, |cluster| {
					cluster.meta.recently_played += update as u64;
					async { Ok(()) }
				})
				.await
				{
					tracing::warn!(
						"failed to update playtime for cluster {}: {}",
						&cluster_path,
						err
					);
				}
				last_updated = Utc::now();
			}
		}

		let update = Utc::now().signed_duration_since(last_updated).num_seconds();
		if let Err(err) = cluster::edit(&cluster_path, |cluster| {
			cluster.meta.recently_played += update as u64;
			async { Ok(()) }
		})
		.await
		{
			tracing::warn!(
				"failed to update playtime for cluster {}: {}",
				&cluster_path,
				err
			);
		}

		let cluster_path_in = cluster_path.clone();
		tokio::spawn(async move {
			if let Err(err) = cluster::update_playtime(&cluster_path_in).await {
				tracing::warn!(
					"failed to update playtime for cluster {}: {}",
					&cluster_path_in,
					err
				);
			}
		});

		tokio::spawn(async {
			let state = match crate::State::get().await {
				Ok(state) => state,
				Err(err) => {
					tracing::warn!("failed to get state: {}", err);
					return;
				}
			};
			let _ = state.discord_rpc.clear(true).await;
		});

		#[cfg(feature = "tauri")]
		{
			let window = crate::ProxyState::get_main_window().await?;
			window.unminimize()?;
		}

		{
			let current_child = current_child.write().await;
			current_child.remove(uuid).await?;
		}

		if !exit_status == 0 {
			send_process(
				uuid,
				current_pid,
				ProcessPayloadType::Finished,
				"exited process",
			)
			.await?;

			return Ok(exit_status);
		}

		let post = if let Some(hook) = post {
			let mut cmd = hook.split(' ');
			if let Some(c) = cmd.next() {
				let mut c = Command::new(c);
				c.args(&cmd.collect::<Vec<&str>>())
					.current_dir(cluster_path.full_path().await?);
				Some(c)
			} else {
				None
			}
		} else {
			None
		};

		if let Some(mut m_c) = post {
			{
				let mut current_child: tokio::sync::RwLockWriteGuard<'_, ChildType> =
					current_child.write().await;
				let new_child = m_c.spawn().map_err(IOError::from)?;
				current_pid = new_child
					.id()
					.ok_or_else(|| anyhow::anyhow!("process failed, couldnt get pid"))?;
				*current_child = ChildType::ChildProcess(new_child);
			}

			send_process(
				uuid,
				current_pid,
				ProcessPayloadType::Modified,
				"running post hook",
			)
			.await?;

			loop {
				if let Some(stat) = current_child.write().await.try_wait().await? {
					exit_status = stat;
					break;
				}

				tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
			}
		}

		send_process(
			uuid,
			current_pid,
			ProcessPayloadType::Finished,
			"exited process",
		)
		.await?;

		Ok(exit_status)
	}
}

impl Default for Processor {
	fn default() -> Self {
		Self::new()
	}
}
