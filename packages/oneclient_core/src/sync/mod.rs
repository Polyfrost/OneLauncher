mod http;
mod ledger;
mod local;
mod settings;
mod store;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_tempfile::TempDir;
use chrono::Utc;
use oneclient_auth::{AccountKind, AuthService};
use oneclient_cluster::ClusterUpdate;
use oneclient_common::domain::ContentType;
use oneclient_common::patch::Patch;
use oneclient_content::bundles::remove_artifact_from_cluster;
use oneclient_content::modpacks::SHARE_EXTENSION;
use oneclient_content::packages::PackageStore;
use oneclient_db::dao::cluster_bundle as bundle_dao;

pub use http::HttpSyncStore;
pub use ledger::{SyncEntry, SyncLedger, SyncStatus};
pub use local::LocalSyncStore;
pub use settings::SyncSettings;
pub use store::{SyncAccount, SyncError, SyncRecord, SyncStore, SyncUpload};

use crate::LauncherResult;
use crate::clusters::{
    ExportMode, ExportOptions, ModpackSource, SettingsChoice, create_modpack_instance,
    export_cluster_with, install_modpack_instance, prepare_modpack,
};
use crate::sharing::{self, ShareCode, ShareMeta, SharingError};
use crate::state::LauncherState;

pub const SYNC_DIR_ENV: &str = "ONECLIENT_SYNC_DIR";

const OVERRIDES_PREFIX: &str = "overrides/";
const CONTENT_DIRS: [&str; 3] = ["mods/", "resourcepacks/", "shaderpacks/"];

const SYNCED_CONTENT: [ContentType; 3] = [
    ContentType::Mod,
    ContentType::ResourcePack,
    ContentType::Shader,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    KeepLocal,
    KeepCloud,
    KeepBoth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    Unchanged,
    Pushed {
        revision: u64,
    },
    Pulled {
        cluster_id: i64,
        revision: u64,
    },
    Replaced {
        old_cluster_id: i64,
        cluster_id: i64,
    },
    Copied {
        copy_cluster_id: i64,
    },
    Conflict {
        revision: u64,
    },
    RemovedFromCloud,
}

impl SyncOutcome {
    #[must_use]
    pub fn status(&self) -> Option<SyncStatus> {
        match self {
            Self::Conflict { revision } => Some(SyncStatus::Conflict {
                revision: *revision,
            }),
            Self::RemovedFromCloud => None,
            _ => Some(SyncStatus::Synced),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub created: Vec<i64>,
    pub pushed: Vec<i64>,
    pub pulled: Vec<i64>,
    pub replaced: Vec<(i64, i64)>,
    pub conflicts: Vec<i64>,
    pub removed: Vec<i64>,
    pub failed: Vec<(i64, String)>,
}

struct Snapshot {
    _dir: TempDir,
    path: PathBuf,
    meta: ShareMeta,
    fingerprint: String,
    provisioned: bool,
}

pub fn sync_dir() -> LauncherResult<PathBuf> {
    Ok(oneclient_common::paths::data_dir()?.join("sync"))
}

pub fn default_store(auth: Arc<AuthService>) -> Arc<dyn SyncStore> {
    match std::env::var_os(SYNC_DIR_ENV).filter(|dir| !dir.is_empty()) {
        Some(dir) => Arc::new(LocalSyncStore::new(PathBuf::from(dir))),
        None => Arc::new(HttpSyncStore::new(auth)),
    }
}

#[must_use]
pub fn device_name() -> String {
    sysinfo::System::host_name()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Another computer".to_string())
}

pub async fn current_account(state: &LauncherState) -> Result<SyncAccount, SyncError> {
    let account = state
        .auth
        .default_account()
        .await
        .map_err(|err| SyncError::Unavailable(err.to_string()))?
        .filter(|account| account.kind == AccountKind::Microsoft && !account.signed_out)
        .ok_or(SyncError::NotSignedIn)?;
    Ok(SyncAccount {
        id: account.id,
        name: account.username,
        token: None,
    })
}

pub async fn fingerprint(payload: &Path) -> LauncherResult<String> {
    let mut entries = polyio::read_zip_file_entries(payload, |_| true).await?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut joined = Vec::new();
    for (name, bytes) in entries {
        joined.extend_from_slice(name.as_bytes());
        joined.push(0);
        joined.extend_from_slice(polyio::sha1_bytes(&bytes).as_bytes());
        joined.push(b'\n');
    }
    Ok(polyio::sha1_bytes(&joined))
}

pub struct CloudSync {
    store: Arc<dyn SyncStore>,
    ledger: SyncLedger,
    settings: SyncSettings,
    device: String,
    incoming: PathBuf,
    op: tokio::sync::Mutex<()>,
}

impl CloudSync {
    #[must_use]
    pub fn new(
        store: Arc<dyn SyncStore>,
        ledger: SyncLedger,
        device: impl Into<String>,
        incoming: impl Into<PathBuf>,
    ) -> Self {
        Self {
            store,
            settings: SyncSettings::at(ledger.path().with_file_name("settings.json")),
            ledger,
            device: device.into(),
            incoming: incoming.into(),
            op: tokio::sync::Mutex::new(()),
        }
    }

    pub fn from_defaults(auth: Arc<AuthService>) -> LauncherResult<Self> {
        let dir = sync_dir()?;
        Ok(Self::new(
            default_store(auth),
            SyncLedger::at(dir.join("state.json")),
            device_name(),
            dir.join("incoming"),
        ))
    }

    #[must_use]
    pub fn ledger(&self) -> &SyncLedger {
        &self.ledger
    }

    pub async fn left_out(&self, cluster_id: i64) -> Result<BTreeSet<String>, SyncError> {
        self.settings.left_out(cluster_id).await
    }

    #[tracing::instrument(skip(self, state, account, left_out))]
    pub async fn set_left_out(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        cluster_id: i64,
        left_out: BTreeSet<String>,
    ) -> LauncherResult<SyncOutcome> {
        self.settings.set(cluster_id, left_out).await?;
        if self.ledger.find(account.id, cluster_id).await?.is_none() {
            return Ok(SyncOutcome::Unchanged);
        }
        Box::pin(self.sync_cluster(state, account, cluster_id)).await
    }

    async fn snapshot(
        &self,
        state: &Arc<LauncherState>,
        cluster_id: i64,
    ) -> LauncherResult<Snapshot> {
        let left_out = self.settings.left_out(cluster_id).await?;
        Box::pin(take_snapshot(state, cluster_id, left_out)).await
    }

    async fn adopt_settings(&self, cluster_id: i64, payload: &Path) -> LauncherResult<()> {
        let left_out = oneclient_content::modpacks::read_cluster_share(payload)
            .await?
            .map(|share| share.left_out_settings.into_iter().collect())
            .unwrap_or_default();
        self.settings.set(cluster_id, left_out).await?;
        Ok(())
    }

    #[must_use]
    pub fn store(&self) -> &Arc<dyn SyncStore> {
        &self.store
    }

    pub async fn statuses(
        &self,
        account: &SyncAccount,
    ) -> Result<HashMap<i64, SyncStatus>, SyncError> {
        Ok(self
            .ledger
            .for_account(account.id)
            .await?
            .into_iter()
            .map(|entry| (entry.cluster_id, entry.status))
            .collect())
    }

    #[tracing::instrument(skip(self, state, account))]
    pub async fn enable(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        cluster_id: i64,
    ) -> LauncherResult<SyncOutcome> {
        let _op = self.op.lock().await;
        if let Some(entry) = self.ledger.find(account.id, cluster_id).await? {
            drop(_op);
            tracing::debug!(cluster_id, remote = %entry.remote, "already synced");
            return Box::pin(self.sync_cluster(state, account, cluster_id)).await;
        }
        let snapshot = self.snapshot(state, cluster_id).await?;
        let record = self
            .store
            .create(account, &snapshot.path, self.upload(&snapshot))
            .await?;
        self.remember(account, cluster_id, &record, snapshot.fingerprint)
            .await?;
        tracing::info!(cluster_id, remote = %record.id, "started syncing an instance");
        Ok(SyncOutcome::Pushed {
            revision: record.revision,
        })
    }

    #[tracing::instrument(skip(self, account))]
    pub async fn disable(&self, account: &SyncAccount, cluster_id: i64) -> LauncherResult<()> {
        let _op = self.op.lock().await;
        let Some(entry) = self.ledger.find(account.id, cluster_id).await? else {
            return Ok(());
        };
        match self.store.delete(account, &entry.remote, None).await {
            Ok(()) | Err(SyncError::NotFound(_) | SyncError::Deleted(_)) => {}
            Err(err) => return Err(err.into()),
        }
        self.ledger.remove(account.id, cluster_id).await?;
        tracing::info!(cluster_id, remote = %entry.remote, "stopped syncing an instance");
        Ok(())
    }

    #[tracing::instrument(skip(self, account))]
    pub async fn cluster_deleted(
        &self,
        account: Option<&SyncAccount>,
        cluster_id: i64,
    ) -> LauncherResult<()> {
        let _op = self.op.lock().await;
        let removed = self.ledger.remove_cluster(cluster_id).await?;
        let Some(account) = account else {
            return Ok(());
        };
        for entry in removed.iter().filter(|entry| entry.account == account.id) {
            match self.store.delete(account, &entry.remote, None).await {
                Ok(()) | Err(SyncError::NotFound(_) | SyncError::Deleted(_)) => {}
                Err(err) => {
                    tracing::warn!(cluster_id, remote = %entry.remote, error = %err, "could not remove a deleted instance from the cloud");
                }
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, state, account))]
    pub async fn sync_cluster(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        cluster_id: i64,
    ) -> LauncherResult<SyncOutcome> {
        let _op = self.op.lock().await;
        let Some(entry) = self.ledger.find(account.id, cluster_id).await? else {
            return Err(SyncError::NotSynced.into());
        };
        Box::pin(self.sync_entry(state, account, entry, None)).await
    }

    #[tracing::instrument(skip(self, state, account))]
    pub async fn sync_all(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
    ) -> LauncherResult<SyncReport> {
        let _op = self.op.lock().await;
        let records = self.store.list(account).await?;
        let mut report = SyncReport::default();
        let entries = self.ledger.for_account(account.id).await?;
        let linked: HashSet<ShareCode> = entries.iter().map(|entry| entry.remote.clone()).collect();

        for entry in entries {
            let cluster_id = entry.cluster_id;
            if state.clusters.get(cluster_id).await.is_err() {
                self.ledger.remove(account.id, cluster_id).await?;
                continue;
            }
            let listed = records
                .iter()
                .find(|record| record.id == entry.remote)
                .cloned();
            let listed = match listed {
                Some(record) => Some(record),
                None => {
                    self.ledger.remove(account.id, cluster_id).await?;
                    report.removed.push(cluster_id);
                    continue;
                }
            };
            match Box::pin(self.sync_entry(state, account, entry, listed)).await {
                Ok(SyncOutcome::Unchanged) => {}
                Ok(SyncOutcome::Pushed { .. }) => report.pushed.push(cluster_id),
                Ok(SyncOutcome::Pulled { cluster_id, .. }) => report.pulled.push(cluster_id),
                Ok(SyncOutcome::Replaced {
                    old_cluster_id,
                    cluster_id,
                }) => report.replaced.push((old_cluster_id, cluster_id)),
                Ok(SyncOutcome::Conflict { .. }) => report.conflicts.push(cluster_id),
                Ok(SyncOutcome::RemovedFromCloud) => report.removed.push(cluster_id),
                Ok(SyncOutcome::Copied { .. }) => {}
                Err(err) => report.failed.push((cluster_id, err.to_string())),
            }
        }

        for record in records
            .iter()
            .filter(|record| !record.deleted && !linked.contains(&record.id))
        {
            match Box::pin(self.pull_new(state, account, record)).await {
                Ok(cluster_id) => report.created.push(cluster_id),
                Err(err) => {
                    tracing::warn!(remote = %record.id, name = %record.meta.name, error = %err, "could not set up a synced instance");
                    report
                        .failed
                        .push((-1, format!("{}: {err}", record.meta.name)));
                }
            }
        }
        Ok(report)
    }

    #[tracing::instrument(skip(self, state, account))]
    pub async fn resolve(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        cluster_id: i64,
        resolution: Resolution,
    ) -> LauncherResult<SyncOutcome> {
        let _op = self.op.lock().await;
        let Some(entry) = self.ledger.find(account.id, cluster_id).await? else {
            return Err(SyncError::NotSynced.into());
        };
        let outcome = Box::pin(self.resolve_entry(state, account, &entry, resolution)).await;
        self.settle(account, cluster_id, &outcome).await;
        outcome
    }

    async fn resolve_entry(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        entry: &SyncEntry,
        resolution: Resolution,
    ) -> LauncherResult<SyncOutcome> {
        let cluster_id = entry.cluster_id;
        let remote = match self.store.get(account, &entry.remote).await {
            Ok(record) if !record.deleted => record,
            Ok(_) | Err(SyncError::NotFound(_) | SyncError::Deleted(_)) => {
                self.ledger.remove(account.id, cluster_id).await?;
                return Ok(SyncOutcome::RemovedFromCloud);
            }
            Err(err) => return Err(err.into()),
        };

        match resolution {
            Resolution::KeepLocal => {
                let snapshot = self.snapshot(state, cluster_id).await?;
                self.push(account, entry, remote.revision, snapshot).await
            }
            Resolution::KeepCloud => Box::pin(self.pull(state, account, entry, &remote)).await,
            Resolution::KeepBoth => {
                let snapshot = self.snapshot(state, cluster_id).await?;
                let fresh = self
                    .store
                    .create(account, &snapshot.path, self.upload(&snapshot))
                    .await?;
                self.remember(account, cluster_id, &fresh, snapshot.fingerprint)
                    .await?;

                let (_dir, path, record) = Box::pin(self.download(account, &remote.id)).await?;
                let copy = Box::pin(apply_new(state, &path)).await?;
                self.adopt_settings(copy, &path).await?;
                let fingerprint = self.snapshot(state, copy).await?.fingerprint;
                self.remember(account, copy, &record, fingerprint).await?;
                tracing::info!(
                    cluster_id,
                    copy,
                    "kept both copies of a conflicted instance"
                );
                Ok(SyncOutcome::Copied {
                    copy_cluster_id: copy,
                })
            }
        }
    }

    async fn sync_entry(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        entry: SyncEntry,
        listed: Option<SyncRecord>,
    ) -> LauncherResult<SyncOutcome> {
        let cluster_id = entry.cluster_id;
        let outcome = Box::pin(self.sync_entry_inner(state, account, &entry, listed)).await;
        self.settle(account, cluster_id, &outcome).await;
        outcome
    }

    async fn settle(
        &self,
        account: &SyncAccount,
        cluster_id: i64,
        outcome: &LauncherResult<SyncOutcome>,
    ) {
        let status = match outcome {
            Ok(outcome) => outcome.status(),
            Err(err) => Some(SyncStatus::Error {
                message: err.to_string(),
            }),
        };
        if let Some(status) = status
            && let Err(err) = self.ledger.set_status(account.id, cluster_id, status).await
        {
            tracing::warn!(cluster_id, error = %err, "could not record the sync status");
        }
    }

    async fn sync_entry_inner(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        entry: &SyncEntry,
        listed: Option<SyncRecord>,
    ) -> LauncherResult<SyncOutcome> {
        let cluster_id = entry.cluster_id;
        let remote = match listed {
            Some(record) => Ok(record),
            None => self.store.get(account, &entry.remote).await,
        };
        let remote = match remote {
            Ok(record) if !record.deleted => record,
            Ok(_) | Err(SyncError::NotFound(_) | SyncError::Deleted(_)) => {
                self.ledger.remove(account.id, cluster_id).await?;
                tracing::info!(cluster_id, remote = %entry.remote, "the instance was removed from the cloud; keeping the local copy");
                return Ok(SyncOutcome::RemovedFromCloud);
            }
            Err(err) => return Err(err.into()),
        };

        let snapshot = self.snapshot(state, cluster_id).await?;
        let local_changed = snapshot.fingerprint != entry.fingerprint;
        let remote_changed = remote.revision != entry.revision;

        match (local_changed, remote_changed) {
            (false, false) => Ok(SyncOutcome::Unchanged),
            (true, false) => self.push(account, entry, entry.revision, snapshot).await,
            (false, true) => Box::pin(self.pull(state, account, entry, &remote)).await,
            (true, true) => {
                tracing::info!(
                    cluster_id,
                    local = entry.revision,
                    remote = remote.revision,
                    "sync conflict"
                );
                Ok(SyncOutcome::Conflict {
                    revision: remote.revision,
                })
            }
        }
    }

    async fn push(
        &self,
        account: &SyncAccount,
        entry: &SyncEntry,
        if_match: u64,
        snapshot: Snapshot,
    ) -> LauncherResult<SyncOutcome> {
        match self
            .store
            .put(
                account,
                &entry.remote,
                if_match,
                &snapshot.path,
                self.upload(&snapshot),
            )
            .await
        {
            Ok(record) => {
                self.remember(account, entry.cluster_id, &record, snapshot.fingerprint)
                    .await?;
                Ok(SyncOutcome::Pushed {
                    revision: record.revision,
                })
            }
            Err(SyncError::Conflict { revision, .. }) => Ok(SyncOutcome::Conflict { revision }),
            Err(SyncError::NotFound(_) | SyncError::Deleted(_)) => {
                self.ledger.remove(account.id, entry.cluster_id).await?;
                Ok(SyncOutcome::RemovedFromCloud)
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn pull(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        entry: &SyncEntry,
        remote: &SyncRecord,
    ) -> LauncherResult<SyncOutcome> {
        let (_dir, path, record) = Box::pin(self.download(account, &remote.id)).await?;
        let applied = Box::pin(apply_existing(
            state,
            entry.cluster_id,
            &path,
            ClearContent::Synced,
        ))
        .await?;
        self.adopt_settings(applied, &path).await?;
        let fingerprint = self.snapshot(state, applied).await?.fingerprint;
        if applied != entry.cluster_id {
            self.ledger.remove(account.id, entry.cluster_id).await?;
        }
        self.remember(account, applied, &record, fingerprint)
            .await?;
        if applied == entry.cluster_id {
            Ok(SyncOutcome::Pulled {
                cluster_id: applied,
                revision: record.revision,
            })
        } else {
            Ok(SyncOutcome::Replaced {
                old_cluster_id: entry.cluster_id,
                cluster_id: applied,
            })
        }
    }

    async fn pull_new(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        record: &SyncRecord,
    ) -> LauncherResult<i64> {
        let (_dir, path, record) = Box::pin(self.download(account, &record.id)).await?;
        let target = if record.provisioned {
            self.matching_provisioned(state, account, &record).await?
        } else {
            None
        };
        let cluster_id = match target {
            Some(cluster_id) => {
                Box::pin(apply_existing(
                    state,
                    cluster_id,
                    &path,
                    ClearContent::Synced,
                ))
                .await?
            }
            None => Box::pin(apply_new(state, &path)).await?,
        };
        self.adopt_settings(cluster_id, &path).await?;
        let fingerprint = self.snapshot(state, cluster_id).await?.fingerprint;
        self.remember(account, cluster_id, &record, fingerprint)
            .await?;
        tracing::info!(cluster_id, remote = %record.id, "set up a synced instance from the cloud");
        Ok(cluster_id)
    }

    async fn matching_provisioned(
        &self,
        state: &Arc<LauncherState>,
        account: &SyncAccount,
        record: &SyncRecord,
    ) -> LauncherResult<Option<i64>> {
        let linked: HashSet<i64> = self
            .ledger
            .for_account(account.id)
            .await?
            .into_iter()
            .map(|entry| entry.cluster_id)
            .collect();
        Ok(state
            .clusters
            .list()
            .await?
            .into_iter()
            .find(|cluster| {
                !cluster.user_created
                    && !linked.contains(&cluster.id)
                    && cluster.kind == record.meta.kind
                    && cluster.mc_version == record.meta.mc_version
                    && cluster.mc_loader == record.meta.mc_loader
            })
            .map(|cluster| cluster.id))
    }

    async fn download(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
    ) -> LauncherResult<(TempDir, PathBuf, SyncRecord)> {
        polyio::create_dir_all(&self.incoming).await?;
        let dir = TempDir::new_in(self.incoming.clone())
            .await
            .map_err(|err| SyncError::Unavailable(err.to_string()))?;
        let path = dir
            .dir_path()
            .join(format!("{}.{SHARE_EXTENSION}", id.as_str()));
        let record = self.store.download(account, id, &path).await?;
        sharing::verify_payload(&path, &record.meta).await?;
        Ok((dir, path, record))
    }

    async fn remember(
        &self,
        account: &SyncAccount,
        cluster_id: i64,
        record: &SyncRecord,
        fingerprint: String,
    ) -> Result<(), SyncError> {
        self.ledger
            .upsert(SyncEntry {
                account: account.id,
                cluster_id,
                remote: record.id.clone(),
                revision: record.revision,
                fingerprint,
                status: SyncStatus::Synced,
                synced_at: Some(Utc::now()),
            })
            .await
    }

    fn upload<'a>(&'a self, snapshot: &'a Snapshot) -> SyncUpload<'a> {
        SyncUpload {
            meta: &snapshot.meta,
            provisioned: snapshot.provisioned,
            device: &self.device,
        }
    }
}

async fn take_snapshot(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    left_out: BTreeSet<String>,
) -> LauncherResult<Snapshot> {
    let cluster = state.clusters.get(cluster_id).await?;
    let dir = polyio::tempdir().await?;
    let path = dir.dir_path().join(format!("sync.{SHARE_EXTENSION}"));
    let settings = if left_out.is_empty() {
        SettingsChoice::All
    } else {
        SettingsChoice::AllExcept(left_out)
    };
    let options = ExportOptions::new(ExportMode::OneClient).with_setting_choice(settings);
    export_cluster_with(state, cluster_id, &path, options, None).await?;
    let meta = sharing::describe_payload(&path).await?;
    let fingerprint = fingerprint(&path).await?;
    Ok(Snapshot {
        _dir: dir,
        path,
        meta,
        fingerprint,
        provisioned: !cluster.user_created,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClearContent {
    Synced,
    Nothing,
}

pub(crate) async fn apply_new(state: &Arc<LauncherState>, payload: &Path) -> LauncherResult<i64> {
    let prepared =
        prepare_modpack(state, &ModpackSource::File(payload.to_path_buf()), None).await?;
    let cluster = create_modpack_instance(state, &prepared).await?;
    let installed = install_modpack_instance(state, cluster.id, &prepared, None).await?;
    if !installed.report.failed.is_empty() {
        tracing::warn!(cluster_id = cluster.id, failed = ?installed.report.failed, "some synced content could not be installed");
    }
    Ok(cluster.id)
}

pub(crate) async fn apply_existing(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    payload: &Path,
    clear: ClearContent,
) -> LauncherResult<i64> {
    let prepared =
        prepare_modpack(state, &ModpackSource::File(payload.to_path_buf()), None).await?;
    let share = prepared
        .share
        .as_ref()
        .map(|shared| shared.share.clone())
        .ok_or_else(|| {
            SharingError::Corrupt("the cloud copy is not a OneClient instance".into())
        })?;
    let cluster = state.clusters.get(cluster_id).await?;

    if cluster.mc_version != share.mc_version
        || cluster.mc_loader != share.mc_loader
        || (cluster.kind != share.kind && share.modpack.is_none())
    {
        let created = create_modpack_instance(state, &prepared).await?;
        install_modpack_instance(state, created.id, &prepared, None).await?;
        tracing::info!(
            old = cluster_id,
            new = created.id,
            "the cloud copy uses another game version; installed it as a new instance and kept the old one"
        );
        return Ok(created.id);
    }

    if clear == ClearContent::Synced {
        clear_synced_content(state, cluster_id).await?;
    }
    clear_shared_files(&cluster, payload).await?;

    let linked_modpack_hash = if share.modpack.is_some() {
        Patch::Set(prepared.artifact_hash.clone())
    } else if cluster.linked_modpack_hash.is_some() {
        Patch::Clear
    } else {
        Patch::Unchanged
    };
    let mc_loader_version = match &share.mc_loader_version {
        Some(version) if cluster.mc_loader_version.as_ref() != Some(version) => {
            Patch::Set(version.clone())
        }
        _ => Patch::Unchanged,
    };
    let name = (cluster.user_created && cluster.name != share.name).then(|| share.name.clone());
    let tags = (cluster.tags != share.tags).then(|| share.tags.clone());
    state
        .clusters
        .update(
            cluster_id,
            ClusterUpdate {
                name,
                tags,
                mc_loader_version,
                linked_modpack_hash,
                ..Default::default()
            },
        )
        .await?;

    let installed = install_modpack_instance(state, cluster_id, &prepared, None).await?;
    if !installed.report.failed.is_empty() {
        tracing::warn!(cluster_id, failed = ?installed.report.failed, "some synced content could not be installed");
    }
    Ok(cluster_id)
}

async fn clear_shared_files(
    cluster: &oneclient_cluster::Cluster,
    payload: &Path,
) -> LauncherResult<()> {
    if !cluster.is_isolated() && !cluster.uses_dedicated_dir() {
        return Ok(());
    }
    let root = cluster.game_dir()?;
    for name in polyio::zip_entry_names(payload).await? {
        let Some(rel) = name.strip_prefix(OVERRIDES_PREFIX) else {
            continue;
        };
        if rel.is_empty()
            || rel.ends_with('/')
            || rel.split('/').any(|part| part == "..")
            || rel.ends_with(".jar")
            || CONTENT_DIRS.iter().any(|dir| rel.starts_with(dir))
        {
            continue;
        }
        let dest = root.join(polyio::sanitize_path(rel));
        if polyio::try_exists(&dest).await? {
            polyio::remove_file(&dest).await?;
        }
    }
    Ok(())
}

async fn clear_synced_content(state: &Arc<LauncherState>, cluster_id: i64) -> LauncherResult<()> {
    let content = state.services.content();
    for link in PackageStore::list_linked_artifacts(cluster_id, &content).await? {
        if !SYNCED_CONTENT.contains(&link.content_type) {
            continue;
        }
        if bundle_dao::get_bundle_tracked(&state.services.db, cluster_id, &link.hash)
            .await?
            .is_some()
        {
            continue;
        }
        remove_artifact_from_cluster(cluster_id, &link.hash, false, &content).await?;
    }
    Ok(())
}
