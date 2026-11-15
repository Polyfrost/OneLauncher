use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::LauncherState;
use crate::clusters::{PreparedImport, PreparedModpack, is_resetting};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Busy {
    #[error("a modpack is still being installed or changed, wait for it to finish")]
    ModpackJob,
    #[error(
        "a modpack is being set up in a cluster sharing its game folder, wait for it to finish"
    )]
    SharedModpackJob,
    #[error("it is being reset, wait for the reset to finish")]
    Resetting,
    #[error("its files are being checked, wait for the check to finish")]
    Verifying,
    #[error("it is syncing from the shared folder, wait for the sync to finish")]
    Syncing,
    #[error("a cluster is syncing from the shared folder, wait for the sync to finish")]
    SharedSync,
    #[error("it is being exported, wait for the export to finish")]
    Exporting,
    #[error("an export is already running, wait for it to finish")]
    OtherExport,
    #[error("an import is already running, wait for it to finish")]
    Importing,
    #[error("it is already launching")]
    Launching,
    #[error("the game is running, close it first")]
    Running,
    #[error("its updates are being checked, wait for the check to finish")]
    CheckingUpdates,
}

pub enum JobSlot {
    Idle,
    Preparing(Option<i64>),
    AwaitingConfirm(Box<PreparedModpack>),
    AwaitingImport(Box<PreparedImport>),
    Running(i64),
}

static JOB: Mutex<JobSlot> = Mutex::new(JobSlot::Idle);

pub fn with_modpack_job<R>(change: impl FnOnce(&mut JobSlot) -> R) -> R {
    let mut slot = JOB.lock().unwrap_or_else(PoisonError::into_inner);
    change(&mut slot)
}

#[must_use]
pub fn modpack_job_idle() -> bool {
    with_modpack_job(|slot| matches!(slot, JobSlot::Idle))
}

#[must_use]
pub fn modpack_job_cluster() -> Option<i64> {
    with_modpack_job(|slot| match slot {
        JobSlot::Running(id) | JobSlot::Preparing(Some(id)) => Some(*id),
        JobSlot::AwaitingImport(import) => Some(import.cluster_id),
        _ => None,
    })
}

#[must_use]
pub fn modpack_job_running(cluster_id: i64) -> bool {
    modpack_job_cluster() == Some(cluster_id)
}

pub fn claim_modpack_job(target: Option<i64>) -> Result<(), Busy> {
    if import_running() {
        return Err(Busy::Importing);
    }
    with_modpack_job(|slot| match slot {
        JobSlot::Idle => {
            *slot = JobSlot::Preparing(target);
            Ok(())
        }
        _ => Err(Busy::ModpackJob),
    })
}

pub fn lock_modpack_job(cluster_id: i64) {
    with_modpack_job(|slot| *slot = JobSlot::Running(cluster_id));
}

pub fn release_modpack_job() {
    with_modpack_job(|slot| *slot = JobSlot::Idle);
}

pub async fn modpack_job_in_shared_dir(state: &LauncherState, cluster_id: i64) -> bool {
    let Some(busy) = modpack_job_cluster().filter(|id| *id != cluster_id) else {
        return false;
    };
    match (
        state.clusters.get(cluster_id).await,
        state.clusters.get(busy).await,
    ) {
        (Ok(cluster), Ok(busy)) => cluster.shares_dir_with(&busy),
        _ => false,
    }
}

#[must_use]
pub fn modpack_change_block(state: &LauncherState, cluster_id: i64) -> Option<Busy> {
    if exporting(cluster_id) {
        Some(Busy::Exporting)
    } else if is_resetting(cluster_id) {
        Some(Busy::Resetting)
    } else if is_launching(cluster_id) || state.games.is_active(cluster_id) {
        Some(Busy::Running)
    } else {
        None
    }
}

const NOT_EXPORTING: i64 = -1;

static EXPORTING: AtomicI64 = AtomicI64::new(NOT_EXPORTING);

pub struct ExportSlot;

impl ExportSlot {
    #[must_use]
    pub fn claim(cluster_id: i64) -> Option<Self> {
        EXPORTING
            .compare_exchange(
                NOT_EXPORTING,
                cluster_id,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
            .then_some(Self)
    }
}

impl Drop for ExportSlot {
    fn drop(&mut self) {
        EXPORTING.store(NOT_EXPORTING, Ordering::Release);
    }
}

#[must_use]
pub fn exporting(cluster_id: i64) -> bool {
    EXPORTING.load(Ordering::Acquire) == cluster_id
}

pub fn claim_export(cluster_id: i64) -> Result<ExportSlot, Busy> {
    if modpack_job_running(cluster_id) {
        Err(Busy::ModpackJob)
    } else if is_resetting(cluster_id) {
        Err(Busy::Resetting)
    } else if import_running() {
        Err(Busy::Importing)
    } else if update_check_running(cluster_id) {
        Err(Busy::CheckingUpdates)
    } else {
        ExportSlot::claim(cluster_id).ok_or(Busy::OtherExport)
    }
}

static IMPORTING: AtomicBool = AtomicBool::new(false);

#[must_use]
pub fn import_running() -> bool {
    IMPORTING.load(Ordering::Acquire)
}

pub fn set_import_running(running: bool) {
    IMPORTING.store(running, Ordering::Release);
}

pub struct ImportSlot;

impl ImportSlot {
    pub fn claim() -> Result<Self, Busy> {
        if !modpack_job_idle() {
            return Err(Busy::ModpackJob);
        }
        IMPORTING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| Busy::Importing)
    }
}

impl Drop for ImportSlot {
    fn drop(&mut self) {
        set_import_running(false);
    }
}

static LAUNCHING: Mutex<Option<HashSet<i64>>> = Mutex::new(None);

fn launching() -> MutexGuard<'static, Option<HashSet<i64>>> {
    LAUNCHING.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct LaunchClaim(i64);

impl LaunchClaim {
    #[must_use]
    pub fn take(cluster_id: i64) -> Option<Self> {
        launching()
            .get_or_insert_default()
            .insert(cluster_id)
            .then_some(Self(cluster_id))
    }
}

impl Drop for LaunchClaim {
    fn drop(&mut self) {
        if let Some(set) = launching().as_mut() {
            set.remove(&self.0);
        }
    }
}

#[must_use]
pub fn is_launching(cluster_id: i64) -> bool {
    launching()
        .as_ref()
        .is_some_and(|set| set.contains(&cluster_id))
}

static UPDATE_CHECKS: Mutex<Option<HashSet<i64>>> = Mutex::new(None);

fn update_checks() -> MutexGuard<'static, Option<HashSet<i64>>> {
    UPDATE_CHECKS.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct UpdateCheckClaim(i64);

impl UpdateCheckClaim {
    #[must_use]
    pub fn take(cluster_id: i64) -> Option<Self> {
        update_checks()
            .get_or_insert_default()
            .insert(cluster_id)
            .then_some(Self(cluster_id))
    }
}

impl Drop for UpdateCheckClaim {
    fn drop(&mut self) {
        if let Some(set) = update_checks().as_mut() {
            set.remove(&self.0);
        }
    }
}

#[must_use]
pub fn update_check_running(cluster_id: i64) -> bool {
    update_checks()
        .as_ref()
        .is_some_and(|set| set.contains(&cluster_id))
}

pub async fn sync_launch_block(state: &LauncherState, cluster_id: i64) -> Option<Busy> {
    if state.games.is_syncing(cluster_id) {
        return Some(Busy::Syncing);
    }
    if !state.games.any_syncing() {
        return None;
    }
    let folder_syncing = state
        .clusters
        .get(cluster_id)
        .await
        .and_then(|cluster| cluster.game_dir())
        .is_ok_and(|dir| state.games.is_dir_syncing(&dir));
    folder_syncing.then_some(Busy::SharedSync)
}

pub async fn launch_block(state: &LauncherState, cluster_id: i64) -> Option<Busy> {
    if modpack_job_running(cluster_id) {
        Some(Busy::ModpackJob)
    } else if is_resetting(cluster_id) {
        Some(Busy::Resetting)
    } else if crate::verify::is_verifying(cluster_id) {
        Some(Busy::Verifying)
    } else if let Some(busy) = sync_launch_block(state, cluster_id).await {
        Some(busy)
    } else if modpack_job_in_shared_dir(state, cluster_id).await {
        Some(Busy::SharedModpackJob)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_export_runs_at_a_time() {
        let first = ExportSlot::claim(7).expect("nothing is exporting yet");
        assert!(exporting(7));
        assert!(!exporting(8));
        assert!(ExportSlot::claim(8).is_none());
        drop(first);
        assert!(!exporting(7));
        assert!(ExportSlot::claim(8).is_some());
    }
}
