use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::store::{PublishedShare, SharingError};
use crate::clusters::ExportOptions;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushState {
    pub options: ExportOptions,
    pub content: String,
    pub settings: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub cluster_id: i64,
    pub store: String,
    pub share: PublishedShare,
    #[serde(default)]
    pub push: Option<PushState>,
}

#[derive(Debug, Clone)]
pub struct ShareLedger {
    path: PathBuf,
    lock: std::sync::Arc<tokio::sync::Mutex<()>>,
}

impl ShareLedger {
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: std::sync::Arc::default(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn linked(&self) -> super::LinkedLedger {
        super::LinkedLedger::at(self.path.with_file_name(super::LINKED_FILE))
    }

    async fn load(&self) -> Result<Vec<LedgerEntry>, SharingError> {
        if !polyio::try_exists(&self.path).await? {
            return Ok(Vec::new());
        }
        let bytes = polyio::read(&self.path).await?;
        match serde_json::from_slice(&bytes) {
            Ok(entries) => Ok(entries),
            Err(err) => {
                tracing::warn!(path = %self.path.display(), error = %err, "the share ledger is unreadable; starting over");
                Ok(Vec::new())
            }
        }
    }

    async fn save(&self, entries: &[LedgerEntry]) -> Result<(), SharingError> {
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

    pub async fn all(&self) -> Result<Vec<LedgerEntry>, SharingError> {
        let _guard = self.lock.lock().await;
        self.load().await
    }

    pub async fn find(
        &self,
        cluster_id: i64,
        store: &str,
    ) -> Result<Option<LedgerEntry>, SharingError> {
        let _guard = self.lock.lock().await;
        Ok(self
            .load()
            .await?
            .into_iter()
            .find(|entry| entry.cluster_id == cluster_id && entry.store == store))
    }

    pub async fn remember(
        &self,
        cluster_id: i64,
        store: &str,
        share: &PublishedShare,
        push: Option<PushState>,
    ) -> Result<(), SharingError> {
        let _guard = self.lock.lock().await;
        let mut entries = self.load().await?;
        entries.retain(|entry| !(entry.cluster_id == cluster_id && entry.store == store));
        entries.push(LedgerEntry {
            cluster_id,
            store: store.to_string(),
            share: share.clone(),
            push,
        });
        self.save(&entries).await
    }

    pub async fn forget(&self, cluster_id: i64, store: &str) -> Result<(), SharingError> {
        let _guard = self.lock.lock().await;
        let mut entries = self.load().await?;
        let before = entries.len();
        entries.retain(|entry| !(entry.cluster_id == cluster_id && entry.store == store));
        if entries.len() != before {
            self.save(&entries).await?;
        }
        Ok(())
    }
}
