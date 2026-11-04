use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use super::store::SyncError;

#[derive(Debug, Clone)]
pub struct SyncSettings {
    path: PathBuf,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl SyncSettings {
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::default(),
        }
    }

    async fn load(&self) -> Result<BTreeMap<i64, BTreeSet<String>>, SyncError> {
        if !polyio::try_exists(&self.path).await? {
            return Ok(BTreeMap::new());
        }
        let bytes = polyio::read(&self.path).await?;
        match serde_json::from_slice(&bytes) {
            Ok(entries) => Ok(entries),
            Err(err) => {
                tracing::warn!(path = %self.path.display(), error = %err, "the sync settings are unreadable; syncing every settings file");
                Ok(BTreeMap::new())
            }
        }
    }

    pub async fn left_out(&self, cluster_id: i64) -> Result<BTreeSet<String>, SyncError> {
        let _guard = self.lock.lock().await;
        Ok(self.load().await?.remove(&cluster_id).unwrap_or_default())
    }

    pub async fn set(&self, cluster_id: i64, left_out: BTreeSet<String>) -> Result<(), SyncError> {
        let _guard = self.lock.lock().await;
        let mut entries = self.load().await?;
        let changed = if left_out.is_empty() {
            entries.remove(&cluster_id).is_some()
        } else {
            entries.insert(cluster_id, left_out.clone()) != Some(left_out)
        };
        if !changed {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            polyio::create_dir_all(parent).await?;
        }
        let staging = polyio::temp_sibling(&self.path);
        polyio::write(&staging, serde_json::to_vec_pretty(&entries)?).await?;
        if let Err(err) = polyio::rename(&staging, &self.path).await {
            polyio::remove_file(&staging).await.ok();
            return Err(err.into());
        }
        Ok(())
    }
}
