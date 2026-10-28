use std::path::Path;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::sharing::{ShareCode, ShareMeta, SharingError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncAccount {
    pub id: Uuid,
    pub name: String,
    pub token: Option<String>,
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Sign in with a Microsoft account to sync instances to the cloud.")]
    NotSignedIn,
    #[error("The cloud copy of this instance changed on another computer.")]
    Conflict { id: ShareCode, revision: u64 },
    #[error("This instance is no longer in the cloud.")]
    NotFound(ShareCode),
    #[error("This instance was removed from the cloud on another computer.")]
    Deleted(ShareCode),
    #[error("This instance isn't synced to the cloud.")]
    NotSynced,
    #[error("Cloud sync is unavailable: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Sharing(#[from] SharingError),
    #[error(transparent)]
    Io(#[from] polyio::IOError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRecord {
    pub id: ShareCode,
    pub revision: u64,
    pub meta: ShareMeta,
    #[serde(default)]
    pub provisioned: bool,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub updated_by: String,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct SyncUpload<'a> {
    pub meta: &'a ShareMeta,
    pub provisioned: bool,
    pub device: &'a str,
}

#[async_trait]
pub trait SyncStore: Send + Sync {
    fn name(&self) -> &'static str;

    async fn list(&self, account: &SyncAccount) -> Result<Vec<SyncRecord>, SyncError>;

    async fn get(&self, account: &SyncAccount, id: &ShareCode) -> Result<SyncRecord, SyncError>;

    async fn create(
        &self,
        account: &SyncAccount,
        payload: &Path,
        upload: SyncUpload<'_>,
    ) -> Result<SyncRecord, SyncError>;

    async fn put(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
        if_match: u64,
        payload: &Path,
        upload: SyncUpload<'_>,
    ) -> Result<SyncRecord, SyncError>;

    async fn download(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
        dest: &Path,
    ) -> Result<SyncRecord, SyncError>;

    async fn delete(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
        if_match: Option<u64>,
    ) -> Result<(), SyncError>;
}
