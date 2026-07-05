
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use tokio::sync::oneshot;

use crate::notification::LaunchStage;

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
}

impl GameProcessManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_kill(&self, cluster_id: i64, tx: oneshot::Sender<()>) {
        self.kills.lock().unwrap().insert(cluster_id, tx);
    }

    pub fn kill(&self, cluster_id: i64) -> bool {
        match self.kills.lock().unwrap().remove(&cluster_id) {
            Some(tx) => tx.send(()).is_ok(),
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
