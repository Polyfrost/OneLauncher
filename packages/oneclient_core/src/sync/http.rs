use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use oneclient_auth::{AuthService, MinecraftAccount};
use oneclient_polyplus::PlusError;
use oneclient_polyplus::instances::{self, INSTANCES_PATH};
use reqwest::Method;
use serde::Deserialize;

use super::store::{SyncAccount, SyncError, SyncRecord, SyncStore, SyncUpload};
use crate::sharing::http::{
    Remote, RemoteRecord, api_error, download_record, parse, push, record_path, upload_body,
};
use crate::sharing::{MAX_SHARE_BYTES, ShareCode, SharingError};

const PRIVATE: &str = "private";
const MAX_PAGES: usize = 50;

#[derive(Debug, Deserialize)]
struct Page {
    records: Vec<SyncRecord>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Clone)]
pub struct HttpSyncStore {
    auth: Arc<AuthService>,
}

impl HttpSyncStore {
    #[must_use]
    pub fn new(auth: Arc<AuthService>) -> Self {
        Self { auth }
    }

    async fn account(&self, account: &SyncAccount) -> Result<MinecraftAccount, SyncError> {
        if self.auth.get_account(account.id).await.is_none() {
            return Err(SyncError::NotSignedIn);
        }
        self.auth
            .account_for_launch(account.id)
            .await
            .map_err(|err| SyncError::Unavailable(err.to_string()))
    }
}

fn etag(revision: u64) -> String {
    format!("\"r{revision}\"")
}

fn sync_error(err: Remote, id: Option<&ShareCode>, size: u64) -> SyncError {
    match err {
        Remote::Plus(PlusError::OfflineAccount) => SyncError::NotSignedIn,
        Remote::Plus(PlusError::ConsentRequired) => SyncError::Unavailable(
            "it needs the Terms of Service and Privacy Policy accepted in settings".into(),
        ),
        Remote::Plus(PlusError::JoinRejected { .. } | PlusError::LoginRejected { .. }) => {
            SyncError::NotSignedIn
        }
        Remote::Plus(err) => SyncError::Unavailable(err.to_string()),
        Remote::Invalid(message) => SyncError::Unavailable(message),
        Remote::Api(err) => match (err.status, id) {
            (401, _) => SyncError::NotSignedIn,
            (404, Some(id)) => SyncError::NotFound(id.clone()),
            (410, Some(id)) => SyncError::Deleted(id.clone()),
            (412, Some(id)) => SyncError::Conflict {
                id: id.clone(),
                revision: err.revision.unwrap_or_default(),
            },
            (413, _) => SharingError::TooLarge {
                size,
                limit: MAX_SHARE_BYTES,
            }
            .into(),
            (422, _) => SharingError::Corrupt(err.message).into(),
            (428, _) => {
                tracing::error!("the sync server wanted a revision the client did not send");
                SyncError::Unavailable(err.message)
            }
            _ => SyncError::Unavailable(err.message),
        },
    }
}

fn committed(
    reply: &instances::Reply,
    id: Option<&ShareCode>,
    size: u64,
) -> Result<SyncRecord, SyncError> {
    #[derive(Deserialize)]
    struct Committed {
        #[serde(flatten)]
        record: SyncRecord,
        #[serde(default)]
        removed: Vec<String>,
    }

    let committed: Committed = parse(reply).map_err(|err| sync_error(err, id, size))?;
    if !committed.removed.is_empty() {
        tracing::info!(
            id = %committed.record.id,
            removed = ?committed.removed,
            "the sync server left files out of the cloud copy"
        );
    }
    Ok(committed.record)
}

#[async_trait]
impl SyncStore for HttpSyncStore {
    fn name(&self) -> &'static str {
        "polyfrost"
    }

    async fn list(&self, account: &SyncAccount) -> Result<Vec<SyncRecord>, SyncError> {
        let player = self.account(account).await?;
        let mut records = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let mut path = format!("{INSTANCES_PATH}?mine=1&visibility={PRIVATE}");
            if let Some(cursor) = &cursor {
                path.push_str("&cursor=");
                path.push_str(&cursor_param(cursor));
            }
            let reply = instances::call(Some(&player), Method::GET, &path, None, None)
                .await
                .map_err(|err| sync_error(err.into(), None, 0))?;
            let page: Page = parse(&reply).map_err(|err| sync_error(err, None, 0))?;
            records.extend(page.records);
            match page.cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        records.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(records)
    }

    async fn get(&self, account: &SyncAccount, id: &ShareCode) -> Result<SyncRecord, SyncError> {
        let player = self.account(account).await?;
        let reply = instances::call(Some(&player), Method::GET, &record_path(id), None, None)
            .await
            .map_err(|err| sync_error(err.into(), Some(id), 0))?;
        parse(&reply).map_err(|err| sync_error(err, Some(id), 0))
    }

    async fn create(
        &self,
        account: &SyncAccount,
        payload: &Path,
        upload: SyncUpload<'_>,
    ) -> Result<SyncRecord, SyncError> {
        let player = self.account(account).await?;
        let body = upload_body(
            upload.meta,
            PRIVATE,
            upload.provisioned,
            Some(upload.device),
        );
        let reply = push(
            &player,
            Method::POST,
            INSTANCES_PATH,
            None,
            &body,
            payload,
            upload.meta.size,
        )
        .await
        .map_err(|err| sync_error(err, None, upload.meta.size))?;
        committed(&reply, None, upload.meta.size)
    }

    async fn put(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
        if_match: u64,
        payload: &Path,
        upload: SyncUpload<'_>,
    ) -> Result<SyncRecord, SyncError> {
        let player = self.account(account).await?;
        let body = upload_body(
            upload.meta,
            PRIVATE,
            upload.provisioned,
            Some(upload.device),
        );
        let reply = push(
            &player,
            Method::PUT,
            &record_path(id),
            Some(&etag(if_match)),
            &body,
            payload,
            upload.meta.size,
        )
        .await
        .map_err(|err| sync_error(err, Some(id), upload.meta.size))?;
        committed(&reply, Some(id), upload.meta.size)
    }

    async fn download(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
        dest: &Path,
    ) -> Result<SyncRecord, SyncError> {
        let player = self.account(account).await?;
        let reply = instances::call(Some(&player), Method::GET, &record_path(id), None, None)
            .await
            .map_err(|err| sync_error(err.into(), Some(id), 0))?;
        let record: SyncRecord = parse(&reply).map_err(|err| sync_error(err, Some(id), 0))?;
        if record.deleted {
            return Err(SyncError::Deleted(id.clone()));
        }
        let remote: RemoteRecord = parse(&reply).map_err(|err| sync_error(err, Some(id), 0))?;
        download_record(&remote, dest)
            .await
            .map_err(|err| sync_error(err, Some(id), record.meta.size))?;
        Ok(record)
    }

    async fn delete(
        &self,
        account: &SyncAccount,
        id: &ShareCode,
        if_match: Option<u64>,
    ) -> Result<(), SyncError> {
        let player = self.account(account).await?;
        let if_match = if_match.map(etag);
        let reply = instances::call(
            Some(&player),
            Method::DELETE,
            &record_path(id),
            if_match.as_deref(),
            None,
        )
        .await
        .map_err(|err| sync_error(err.into(), Some(id), 0))?;
        if reply.is_success() {
            return Ok(());
        }
        Err(sync_error(Remote::Api(api_error(&reply)), Some(id), 0))
    }
}

fn cursor_param(cursor: &str) -> String {
    cursor.chars().filter(char::is_ascii_alphanumeric).collect()
}
