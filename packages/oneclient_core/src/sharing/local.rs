use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::code::ShareCode;
use super::store::{
    MAX_SHARE_BYTES, PublishedShare, RemoteShare, ShareMeta, ShareStore, SharingError,
};

const PAYLOAD_EXTENSION: &str = "mrpack";
const RECORD_EXTENSION: &str = "json";
const MAX_CODE_ATTEMPTS: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalRecord {
    meta: ShareMeta,
    token_sha1: String,
    #[serde(default = "first_revision")]
    revision: u64,
}

fn first_revision() -> u64 {
    1
}

impl LocalRecord {
    fn remote(self) -> RemoteShare {
        RemoteShare {
            meta: self.meta,
            revision: self.revision,
            contents: None,
            expires_at: None,
            role: super::store::ShareRole::Viewer,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalShareStore {
    root: PathBuf,
}

impl LocalShareStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn payload_path(&self, code: &ShareCode) -> PathBuf {
        self.root
            .join(format!("{}.{PAYLOAD_EXTENSION}", code.as_str()))
    }

    fn record_path(&self, code: &ShareCode) -> PathBuf {
        self.root
            .join(format!("{}.{RECORD_EXTENSION}", code.as_str()))
    }

    async fn read_record(&self, code: &ShareCode) -> Result<LocalRecord, SharingError> {
        let path = self.record_path(code);
        if !polyio::try_exists(&path).await? {
            return Err(SharingError::NotFound(code.clone()));
        }
        let bytes = polyio::read(&path).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn write_atomic(&self, dest: &Path, bytes: &[u8]) -> Result<(), SharingError> {
        let staging = polyio::temp_sibling(dest);
        polyio::write(&staging, bytes).await?;
        if let Err(err) = polyio::rename(&staging, dest).await {
            polyio::remove_file(&staging).await.ok();
            return Err(err.into());
        }
        Ok(())
    }

    async fn store_payload(&self, code: &ShareCode, payload: &Path) -> Result<(), SharingError> {
        let dest = self.payload_path(code);
        let staging = polyio::temp_sibling(&dest);
        polyio::copy(payload, &staging).await?;
        if let Err(err) = polyio::rename(&staging, &dest).await {
            polyio::remove_file(&staging).await.ok();
            return Err(err.into());
        }
        Ok(())
    }

    async fn check_owner(&self, share: &PublishedShare) -> Result<LocalRecord, SharingError> {
        let record = self.read_record(&share.code).await?;
        if polyio::sha1_bytes(share.manage_token.as_bytes()) != record.token_sha1 {
            return Err(SharingError::Forbidden(share.code.clone()));
        }
        Ok(record)
    }

    fn published(code: ShareCode, manage_token: String) -> PublishedShare {
        PublishedShare {
            link: code.web_link(),
            code,
            manage_token,
            expires_at: None,
        }
    }
}

fn check_size(meta: &ShareMeta) -> Result<(), SharingError> {
    if meta.size > MAX_SHARE_BYTES {
        return Err(SharingError::TooLarge {
            size: meta.size,
            limit: MAX_SHARE_BYTES,
        });
    }
    Ok(())
}

fn new_token() -> String {
    let bytes: [u8; 32] = rand::random();
    polyio::to_hex(&bytes)
}

#[async_trait]
impl ShareStore for LocalShareStore {
    fn name(&self) -> &'static str {
        "local"
    }

    async fn publish(
        &self,
        payload: &Path,
        meta: &ShareMeta,
    ) -> Result<PublishedShare, SharingError> {
        check_size(meta)?;
        polyio::create_dir_all(&self.root).await?;

        let mut code = None;
        for _ in 0..MAX_CODE_ATTEMPTS {
            let candidate = ShareCode::generate();
            if !polyio::try_exists(self.record_path(&candidate)).await? {
                code = Some(candidate);
                break;
            }
        }
        let code = code.ok_or_else(|| {
            SharingError::Unavailable("could not pick a free share code".to_string())
        })?;

        let token = new_token();
        let mut meta = meta.clone();
        meta.created_at = Some(Utc::now());
        self.store_payload(&code, payload).await?;
        let record = LocalRecord {
            meta,
            token_sha1: polyio::sha1_bytes(token.as_bytes()),
            revision: 1,
        };
        self.write_atomic(
            &self.record_path(&code),
            &serde_json::to_vec_pretty(&record)?,
        )
        .await?;
        tracing::info!(code = %code, root = %self.root.display(), "published a share locally");
        Ok(Self::published(code, token))
    }

    async fn update(
        &self,
        share: &PublishedShare,
        payload: &Path,
        meta: &ShareMeta,
    ) -> Result<PublishedShare, SharingError> {
        check_size(meta)?;
        let mut record = self.check_owner(share).await?;
        let created_at = record.meta.created_at;
        record.meta = meta.clone();
        record.meta.created_at = created_at;
        record.revision += 1;
        self.store_payload(&share.code, payload).await?;
        self.write_atomic(
            &self.record_path(&share.code),
            &serde_json::to_vec_pretty(&record)?,
        )
        .await?;
        Ok(Self::published(
            share.code.clone(),
            share.manage_token.clone(),
        ))
    }

    async fn describe(&self, code: &ShareCode) -> Result<RemoteShare, SharingError> {
        Ok(self.read_record(code).await?.remote())
    }

    async fn fetch(&self, code: &ShareCode, dest: &Path) -> Result<RemoteShare, SharingError> {
        let record = self.read_record(code).await?;
        let payload = self.payload_path(code);
        if !polyio::try_exists(&payload).await? {
            return Err(SharingError::Gone(code.clone()));
        }
        if let Some(parent) = dest.parent() {
            polyio::create_dir_all(parent).await?;
        }
        polyio::copy(&payload, dest).await?;
        Ok(record.remote())
    }

    async fn revoke(&self, share: &PublishedShare) -> Result<(), SharingError> {
        self.check_owner(share).await?;
        polyio::remove_file(self.payload_path(&share.code))
            .await
            .ok();
        polyio::remove_file(self.record_path(&share.code)).await?;
        Ok(())
    }
}
