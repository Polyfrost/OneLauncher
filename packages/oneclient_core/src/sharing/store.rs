use std::path::Path;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use oneclient_common::domain::GameLoader;
use oneclient_db::models::ClusterKind;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::code::ShareCode;

pub const MAX_SHARE_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_NAME_CHARS: usize = 64;

#[derive(Debug, Error)]
pub enum SharingError {
    #[error("That isn't a share code or link. Codes look like ABCD-EFGH.")]
    InvalidCode,
    #[error("No instance is shared under {0}. Check the code and try again.")]
    NotFound(ShareCode),
    #[error("The share {0} has expired or was taken down by its owner.")]
    Gone(ShareCode),
    #[error("This instance is {size} bytes, over the {limit} byte limit for sharing.")]
    TooLarge { size: u64, limit: u64 },
    #[error("You can't change the share {0}.")]
    Forbidden(ShareCode),
    #[error("Someone else updated {0} since your copy. Update your copy first, then try again.")]
    Outdated(ShareCode),
    #[error("{0}")]
    Refused(String),
    #[error("The shared instance is damaged: {0}")]
    Corrupt(String),
    #[error("The sharing service is unavailable: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Io(#[from] polyio::IOError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareMeta {
    pub format: u32,
    pub name: String,
    pub mc_version: String,
    pub mc_loader: GameLoader,
    pub kind: ClusterKind,
    pub size: u64,
    pub sha512: String,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedShare {
    pub code: ShareCode,
    pub link: String,
    pub manage_token: String,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentEntry {
    pub path: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub source: Option<String>,
}

impl ContentEntry {
    #[must_use]
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    #[must_use]
    pub fn is_verified(&self) -> bool {
        matches!(self.source.as_deref(), Some("modrinth" | "curseforge"))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareContents {
    #[serde(default)]
    pub files: Vec<ContentEntry>,
    #[serde(default)]
    pub settings: Vec<ContentEntry>,
    #[serde(default)]
    pub bundles: Vec<String>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareRole {
    Owner,
    Collaborator,
    #[default]
    Viewer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteShare {
    pub meta: ShareMeta,
    pub revision: u64,
    pub contents: Option<ShareContents>,
    pub expires_at: Option<DateTime<Utc>>,
    pub role: ShareRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collaborator {
    pub uuid: uuid::Uuid,
    #[serde(default)]
    pub username: Option<String>,
}

#[async_trait]
pub trait ShareStore: Send + Sync {
    fn name(&self) -> &'static str;

    async fn publish(
        &self,
        payload: &Path,
        meta: &ShareMeta,
    ) -> Result<PublishedShare, SharingError>;

    async fn update(
        &self,
        share: &PublishedShare,
        payload: &Path,
        meta: &ShareMeta,
    ) -> Result<PublishedShare, SharingError>;

    async fn describe(&self, code: &ShareCode) -> Result<RemoteShare, SharingError>;

    async fn fetch(&self, code: &ShareCode, dest: &Path) -> Result<RemoteShare, SharingError>;

    async fn revoke(&self, share: &PublishedShare) -> Result<(), SharingError>;

    async fn update_from(
        &self,
        share: &PublishedShare,
        base_revision: u64,
        payload: &Path,
        meta: &ShareMeta,
    ) -> Result<PublishedShare, SharingError> {
        let current = self.describe(&share.code).await?;
        if current.revision != base_revision {
            return Err(SharingError::Outdated(share.code.clone()));
        }
        self.update(share, payload, meta).await
    }

    async fn collaborators(&self, share: &ShareCode) -> Result<Vec<Collaborator>, SharingError> {
        Err(SharingError::Forbidden(share.clone()))
    }

    async fn add_collaborator(
        &self,
        share: &ShareCode,
        _username: &str,
    ) -> Result<Collaborator, SharingError> {
        Err(SharingError::Forbidden(share.clone()))
    }

    async fn remove_collaborator(
        &self,
        share: &ShareCode,
        _uuid: uuid::Uuid,
    ) -> Result<(), SharingError> {
        Err(SharingError::Forbidden(share.clone()))
    }
}
