
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use sysinfo::{Pid, ProcessesToUpdate, Signal, System};
use tokio::sync::oneshot;

use oneclient_events::LaunchStage;

fn probe(pid: u32) -> Option<(System, Pid)> {
	let pid = Pid::from_u32(pid);
	let mut sys = System::new();
	sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
	sys.process(pid)?;
	Some((sys, pid))
}

/// Unix seconds Pids get recycled so this is what pins process identity
pub fn process_start_time(pid: u32) -> Option<u64> {
	let (sys, pid) = probe(pid)?;
	Some(sys.process(pid)?.start_time())
}

/// A matching pid whose start time differs is a recycled pid belonging to
/// someone else never ours
pub fn is_process_alive(pid: u32, started_at: Option<u64>) -> bool {
	let Some(actual) = process_start_time(pid) else {
		return false;
	};
	match started_at {
		Some(expected) => actual == expected,
		// Pre-migration sessions have no recorded start time assuming running
		// is the safer error as the exit time is recovered on a later start
		None => true,
	}
}

/// For games re-adopted after a launcher restart where no `Child` handle
/// survives to kill through
pub fn kill_process(pid: u32) -> bool {
	let Some((sys, pid)) = probe(pid) else {
		return false;
	};
	let Some(process) = sys.process(pid) else {
		return false;
	};
	// Prefer a graceful terminate so the game can save before shutting down
	process
		.kill_with(Signal::Term)
		.unwrap_or_else(|| process.kill())
}

#[derive(Debug, Clone)]
pub struct GameProcess {
    pub pid: Option<u32>,
    pub stage: LaunchStage,
    pub started: Instant,
}

#[derive(Default)]
pub struct GameProcessManager {
    inner: Mutex<HashMap<i64, GameProcess>>,
    kills: Mutex<HashMap<i64, oneshot::Sender<()>>>,
    dirs: Mutex<HashMap<i64, PathBuf>>,
}

impl GameProcessManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_kill(&self, cluster_id: i64, tx: oneshot::Sender<()>) {
        self.kills.lock().unwrap().insert(cluster_id, tx);
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub fn kill(&self, cluster_id: i64) -> bool {
        match self.kills.lock().unwrap().remove(&cluster_id) {
            Some(tx) => {
                tracing::debug!(cluster_id, "signalling kill to running game");
                tx.send(()).is_ok()
            }
            None => false,
        }
    }

    pub fn set_stage(&self, cluster_id: i64, stage: LaunchStage) {
        let mut map = self.inner.lock().unwrap();
        if stage == LaunchStage::Exited {
            map.remove(&cluster_id);
            return;
        }
        map.entry(cluster_id)
            .and_modify(|p| p.stage = stage)
            .or_insert_with(|| GameProcess {
                pid: None,
                stage,
                started: Instant::now(),
            });
    }

    pub fn set_pid(&self, cluster_id: i64, pid: Option<u32>) {
        if let Some(p) = self.inner.lock().unwrap().get_mut(&cluster_id) {
            p.pid = pid;
        }
    }

    pub fn remove(&self, cluster_id: i64) {
        self.inner.lock().unwrap().remove(&cluster_id);
        self.kills.lock().unwrap().remove(&cluster_id);
        self.dirs.lock().unwrap().remove(&cluster_id);
    }

    pub fn set_dir(&self, cluster_id: i64, dir: PathBuf) {
        self.dirs.lock().unwrap().insert(cluster_id, dir);
    }

    /// Like [`Self::dir_in_use_by`] but counts a cluster's own session too the
    /// shared game directory holds one game at a time whoever it belongs to
    pub fn dir_in_use(&self, dir: &Path) -> Option<i64> {
        self.dirs
            .lock()
            .unwrap()
            .iter()
            .find(|(_, d)| d.as_path() == dir)
            .map(|(id, _)| *id)
    }

    pub fn dir_in_use_by(&self, dir: &Path, exclude: i64) -> Option<i64> {
        self.dirs
            .lock()
            .unwrap()
            .iter()
            .find(|(id, d)| **id != exclude && d.as_path() == dir)
            .map(|(id, _)| *id)
    }

    pub fn pid(&self, cluster_id: i64) -> Option<u32> {
        self.inner.lock().unwrap().get(&cluster_id).and_then(|p| p.pid)
    }

    pub fn is_running(&self, cluster_id: i64) -> bool {
        self.inner
            .lock()
            .unwrap()
            .get(&cluster_id)
            .is_some_and(|p| p.stage == LaunchStage::Running)
    }

    pub fn is_active(&self, cluster_id: i64) -> bool {
        self.inner.lock().unwrap().contains_key(&cluster_id)
    }

    pub fn stage(&self, cluster_id: i64) -> Option<LaunchStage> {
        self.inner.lock().unwrap().get(&cluster_id).map(|p| p.stage)
    }

    pub fn active_ids(&self) -> Vec<i64> {
        self.inner.lock().unwrap().keys().copied().collect()
    }

    pub fn running_ids(&self) -> Vec<i64> {
        self.inner
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, p)| p.stage == LaunchStage::Running)
            .map(|(id, _)| *id)
            .collect()
    }
}
