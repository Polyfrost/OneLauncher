use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::store::SyncError;
use crate::sharing::ShareCode;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SyncStatus {
    Synced,
    Pending,
    Syncing,
    Conflict { revision: u64 },
    Error { message: String },
}

impl SyncStatus {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Synced => "Synced",
            Self::Pending => "Waiting to sync",
            Self::Syncing => "Syncing",
            Self::Conflict { .. } => "Conflict",
            Self::Error { .. } => "Sync failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncEntry {
    pub account: Uuid,
    pub cluster_id: i64,
    pub remote: ShareCode,
    pub revision: u64,
    pub fingerprint: String,
    pub status: SyncStatus,
    #[serde(default)]
    pub synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SyncLedger {
    path: PathBuf,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl SyncLedger {
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::default(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    async fn load(&self) -> Result<Vec<SyncEntry>, SyncError> {
        if !polyio::try_exists(&self.path).await? {
            return Ok(Vec::new());
        }
        let bytes = polyio::read(&self.path).await?;
        match serde_json::from_slice(&bytes) {
            Ok(entries) => Ok(entries),
            Err(err) => {
                tracing::warn!(path = %self.path.display(), error = %err, "the sync ledger is unreadable; starting over");
                Ok(Vec::new())
            }
        }
    }

    async fn save(&self, entries: &[SyncEntry]) -> Result<(), SyncError> {
        if let Some(parent) = self.path.parent() {
            polyio::create_dir_all(parent).await?;
        }
        let staging = polyio::temp_sibling(&self.path);
        polyio::write(&staging, serde_json::to_vec_pretty(entries)?).await?;
        if let Err(err) = polyio::rename(&staging, &self.path).await {
            polyio::remove_file(&staging).await.ok();
            return Err(err.into());
        }
        Ok(())
    }

    pub async fn for_account(&self, account: Uuid) -> Result<Vec<SyncEntry>, SyncError> {
        let _guard = self.lock.lock().await;
        Ok(self
            .load()
            .await?
            .into_iter()
            .filter(|entry| entry.account == account)
            .collect())
    }

    pub async fn find(
        &self,
        account: Uuid,
        cluster_id: i64,
    ) -> Result<Option<SyncEntry>, SyncError> {
        let _guard = self.lock.lock().await;
        Ok(self
            .load()
            .await?
            .into_iter()
            .find(|entry| entry.account == account && entry.cluster_id == cluster_id))
    }

    pub async fn upsert(&self, entry: SyncEntry) -> Result<(), SyncError> {
        let _guard = self.lock.lock().await;
        let mut entries = self.load().await?;
        entries.retain(|other| {
            !(other.account == entry.account
                && (other.cluster_id == entry.cluster_id || other.remote == entry.remote))
        });
        entries.push(entry);
        self.save(&entries).await
    }

    pub async fn set_status(
        &self,
        account: Uuid,
        cluster_id: i64,
        status: SyncStatus,
    ) -> Result<(), SyncError> {
        let _guard = self.lock.lock().await;
        let mut entries = self.load().await?;
        let mut changed = false;
        for entry in &mut entries {
            if entry.account == account && entry.cluster_id == cluster_id && entry.status != status
            {
                entry.status = status.clone();
                changed = true;
            }
        }
        if changed {
            self.save(&entries).await?;
        }
        Ok(())
    }

    pub async fn remove(&self, account: Uuid, cluster_id: i64) -> Result<(), SyncError> {
        let _guard = self.lock.lock().await;
        let mut entries = self.load().await?;
        let before = entries.len();
        entries.retain(|entry| !(entry.account == account && entry.cluster_id == cluster_id));
        if entries.len() != before {
            self.save(&entries).await?;
        }
        Ok(())
    }

    pub async fn remove_cluster(&self, cluster_id: i64) -> Result<Vec<SyncEntry>, SyncError> {
        let _guard = self.lock.lock().await;
        let mut entries = self.load().await?;
        let (removed, kept): (Vec<_>, Vec<_>) = entries
            .drain(..)
            .partition(|entry| entry.cluster_id == cluster_id);
        if !removed.is_empty() {
            self.save(&kept).await?;
        }
        Ok(removed)
    }
}
