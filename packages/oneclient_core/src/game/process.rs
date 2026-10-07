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
    natives: Mutex<HashMap<i64, PathBuf>>,
    launching: tokio::sync::Mutex<()>,
}

impl GameProcessManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// One launch prepares at a time two launches at once would install the
    /// same Java or download the same game files into the same paths
    /// Held from the checks until the process spawns not for the whole session
    pub async fn launch_slot(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.launching.lock().await
    }

    /// `None` while another launch holds the slot
    pub fn try_launch_slot(&self) -> Option<tokio::sync::MutexGuard<'_, ()>> {
        self.launching.try_lock().ok()
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
        self.natives.lock().unwrap().remove(&cluster_id);
    }

    pub fn set_natives(&self, cluster_id: i64, dir: PathBuf) {
        self.natives.lock().unwrap().insert(cluster_id, dir);
    }

    pub fn natives_in_use_by(&self, dir: &Path, exclude: Option<i64>) -> Option<i64> {
        self.natives
            .lock()
            .unwrap()
            .iter()
            .find(|(id, d)| Some(**id) != exclude && d.as_path() == dir)
            .map(|(id, _)| *id)
    }

    pub fn set_dir(&self, cluster_id: i64, dir: PathBuf) {
        self.dirs.lock().unwrap().insert(cluster_id, dir);
    }

    /// Check and record under one lock so two launches racing on separate
    /// threads cannot both find the directory free
    /// `exclusive` is for the shared game directory which holds one game at a
    /// time whoever it belongs to counting the cluster's own session too
    /// Returns the cluster already holding `dir`
    pub fn claim_dir(&self, cluster_id: i64, dir: &Path, exclusive: bool) -> Result<(), i64> {
        let mut dirs = self.dirs.lock().unwrap();

        let holder = dirs
            .iter()
            .find(|(id, d)| (exclusive || **id != cluster_id) && d.as_path() == dir)
            .map(|(id, _)| *id);
        if let Some(holder) = holder {
            return Err(holder);
        }

        dirs.insert(cluster_id, dir.to_path_buf());
        Ok(())
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
        self.inner
            .lock()
            .unwrap()
            .get(&cluster_id)
            .and_then(|p| p.pid)
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

    /// Every cluster the manager knows about not just the ones already running
    /// a launch still resolving its files is as much a reason to leave its
    /// natives alone as a live game
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_dir_takes_one_game() {
        let games = GameProcessManager::new();
        let shared = Path::new("shared");

        assert_eq!(games.claim_dir(1, shared, true), Ok(()));
        assert_eq!(games.claim_dir(2, shared, true), Err(1));

        games.remove(1);
        assert_eq!(games.claim_dir(2, shared, true), Ok(()));
    }

    #[test]
    fn dedicated_dirs_do_not_get_in_each_others_way() {
        let games = GameProcessManager::new();

        assert_eq!(games.claim_dir(1, Path::new("one"), false), Ok(()));
        assert_eq!(games.claim_dir(2, Path::new("two"), false), Ok(()));
        assert_eq!(games.claim_dir(3, Path::new("one"), false), Err(1));
    }

    #[test]
    fn natives_are_held_until_the_game_is_removed() {
        let games = GameProcessManager::new();
        let natives = Path::new("natives/1.8.9");

        games.set_natives(1, natives.to_path_buf());
        assert_eq!(games.natives_in_use_by(natives, None), Some(1));
        assert_eq!(games.natives_in_use_by(natives, Some(1)), None);
        assert_eq!(games.natives_in_use_by(Path::new("natives/1.21"), None), None);

        games.remove(1);
        assert_eq!(games.natives_in_use_by(natives, None), None);
    }
}
