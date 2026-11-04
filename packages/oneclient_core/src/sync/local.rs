use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;

use super::store::{SyncAccount, SyncError, SyncRecord, SyncStore, SyncUpload};
use crate::sharing::{MAX_SHARE_BYTES, ShareCode, ShareMeta, SharingError};

const MAX_ID_ATTEMPTS: usize = 16;

static LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Clone)]
pub struct LocalSyncStore {
    root: PathBuf,
}

impl LocalSyncStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn account_dir(&self, account: &SyncAccount) -> PathBuf {
        self.root.join(account.id.simple().to_string())
    }

    fn record_path(&self, account: &SyncAccount, id: &ShareCode) -> PathBuf {
        self.account_dir(account)
            .join(format!("{}.json", id.as_str()))
    }

    fn payload_path(&self, account: &SyncAccount, id: &ShareCode, revision: u64) -> PathBuf {
        self.account_dir(account)
            .join(format!("{}-{revision}.mrpack", id.as_str()))
    }

    async fn read_record(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
    ) -> Result<SyncRecord, SyncError> {
        let path = self.record_path(account, id);
        if !polyio::try_exists(&path).await? {
            return Err(SyncError::NotFound(id.clone()));
        }
        Ok(serde_json::from_slice(&polyio::read(&path).await?)?)
    }

    async fn write_record(
        &self,
        account: &SyncAccount,
        record: &SyncRecord,
    ) -> Result<(), SyncError> {
        let dest = self.record_path(account, &record.id);
        let staging = polyio::temp_sibling(&dest);
        polyio::write(&staging, serde_json::to_vec_pretty(record)?).await?;
        if let Err(err) = polyio::rename(&staging, &dest).await {
            polyio::remove_file(&staging).await.ok();
            return Err(err.into());
        }
        Ok(())
    }

    async fn store_payload(&self, payload: &Path, dest: &Path) -> Result<(), SyncError> {
        let staging = polyio::temp_sibling(dest);
        polyio::copy(payload, &staging).await?;
        if let Err(err) = polyio::rename(&staging, dest).await {
            polyio::remove_file(&staging).await.ok();
            return Err(err.into());
        }
        Ok(())
    }

    fn record(id: ShareCode, revision: u64, upload: SyncUpload<'_>) -> SyncRecord {
        let mut meta: ShareMeta = upload.meta.clone();
        meta.created_at = None;
        SyncRecord {
            id,
            revision,
            meta,
            provisioned: upload.provisioned,
            updated_at: Utc::now(),
            updated_by: upload.device.to_string(),
            deleted: false,
        }
    }
}

fn check_size(meta: &ShareMeta) -> Result<(), SyncError> {
    if meta.size > MAX_SHARE_BYTES {
        return Err(SharingError::TooLarge {
            size: meta.size,
            limit: MAX_SHARE_BYTES,
        }
        .into());
    }
    Ok(())
}

#[async_trait]
impl SyncStore for LocalSyncStore {
    fn name(&self) -> &'static str {
        "local"
    }

    async fn list(&self, account: &SyncAccount) -> Result<Vec<SyncRecord>, SyncError> {
        let _guard = LOCK.lock().await;
        let dir = self.account_dir(account);
        if !polyio::try_exists(&dir).await? {
            return Ok(Vec::new());
        }
        let mut records = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir)
            .await
            .map_err(|err| SyncError::Unavailable(err.to_string()))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|err| SyncError::Unavailable(err.to_string()))?
        {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            match serde_json::from_slice::<SyncRecord>(&polyio::read(&path).await?) {
                Ok(record) => records.push(record),
                Err(err) => {
                    tracing::warn!(path = %path.display(), error = %err, "skipping an unreadable sync record");
                }
            }
        }
        records.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(records)
    }

    async fn get(&self, account: &SyncAccount, id: &ShareCode) -> Result<SyncRecord, SyncError> {
        let _guard = LOCK.lock().await;
        self.read_record(account, id).await
    }

    async fn create(
        &self,
        account: &SyncAccount,
        payload: &Path,
        upload: SyncUpload<'_>,
    ) -> Result<SyncRecord, SyncError> {
        check_size(upload.meta)?;
        let _guard = LOCK.lock().await;
        polyio::create_dir_all(self.account_dir(account)).await?;

        let mut id = None;
        for _ in 0..MAX_ID_ATTEMPTS {
            let candidate = ShareCode::generate();
            if !polyio::try_exists(self.record_path(account, &candidate)).await? {
                id = Some(candidate);
                break;
            }
        }
        let id = id.ok_or_else(|| SyncError::Unavailable("could not pick a free id".into()))?;

        let record = Self::record(id, 1, upload);
        self.store_payload(payload, &self.payload_path(account, &record.id, 1))
            .await?;
        self.write_record(account, &record).await?;
        Ok(record)
    }

    async fn put(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
        if_match: u64,
        payload: &Path,
        upload: SyncUpload<'_>,
    ) -> Result<SyncRecord, SyncError> {
        check_size(upload.meta)?;
        let _guard = LOCK.lock().await;
        let current = self.read_record(account, id).await?;
        if current.deleted {
            return Err(SyncError::Deleted(id.clone()));
        }
        if current.revision != if_match {
            return Err(SyncError::Conflict {
                id: id.clone(),
                revision: current.revision,
            });
        }

        let record = Self::record(id.clone(), current.revision + 1, upload);
        self.store_payload(payload, &self.payload_path(account, id, record.revision))
            .await?;
        self.write_record(account, &record).await?;
        polyio::remove_file(self.payload_path(account, id, current.revision))
            .await
            .ok();
        Ok(record)
    }

    async fn download(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
        dest: &Path,
    ) -> Result<SyncRecord, SyncError> {
        let _guard = LOCK.lock().await;
        let record = self.read_record(account, id).await?;
        if record.deleted {
            return Err(SyncError::Deleted(id.clone()));
        }
        let payload = self.payload_path(account, id, record.revision);
        if !polyio::try_exists(&payload).await? {
            return Err(SyncError::NotFound(id.clone()));
        }
        if let Some(parent) = dest.parent() {
            polyio::create_dir_all(parent).await?;
        }
        polyio::copy(&payload, dest).await?;
        Ok(record)
    }

    async fn delete(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
        if_match: Option<u64>,
    ) -> Result<(), SyncError> {
        let _guard = LOCK.lock().await;
        let mut record = self.read_record(account, id).await?;
        if record.deleted {
            return Ok(());
        }
        if let Some(expected) = if_match
            && expected != record.revision
        {
            return Err(SyncError::Conflict {
                id: id.clone(),
                revision: record.revision,
            });
        }
        polyio::remove_file(self.payload_path(account, id, record.revision))
            .await
            .ok();
        record.revision += 1;
        record.deleted = true;
        record.updated_at = Utc::now();
        self.write_record(account, &record).await
    }
}
