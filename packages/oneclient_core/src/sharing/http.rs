use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use oneclient_auth::{AccountKind, AuthService, MinecraftAccount};
use oneclient_polyplus::PlusError;
use oneclient_polyplus::instances::{self, INSTANCES_PATH, Reply};
use reqwest::Method;
use serde::Deserialize;
use uuid::Uuid;

use super::code::ShareCode;
use super::store::{
    Collaborator, MAX_SHARE_BYTES, PublishedShare, RemoteShare, ShareContents, ShareMeta,
    ShareRole, ShareStore, SharingError,
};

const PUBLIC: &str = "public";

#[derive(Debug)]
pub(crate) enum Remote {
    Plus(PlusError),
    Api(ApiError),
    Invalid(String),
}

impl From<PlusError> for Remote {
    fn from(err: PlusError) -> Self {
        Self::Plus(err)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ApiError {
    pub status: u16,
    pub message: String,
    pub revision: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    #[serde(default)]
    message: String,
    #[serde(default)]
    revision: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Reserved {
    id: ShareCode,
    upload: UploadTarget,
}

#[derive(Debug, Deserialize)]
struct UploadTarget {
    id: Uuid,
    url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RemoteRecord {
    pub id: ShareCode,
    #[serde(default)]
    pub revision: u64,
    pub meta: ShareMeta,
    #[serde(default)]
    pub contents: Option<ShareContents>,
    #[serde(default)]
    pub role: ShareRole,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub download_url: Option<String>,
}

pub(crate) fn api_error(reply: &Reply) -> ApiError {
    let body = serde_json::from_slice::<ErrorBody>(&reply.body).ok();
    let message = body
        .as_ref()
        .map(|body| body.message.clone())
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| String::from_utf8_lossy(&reply.body).trim().to_string());
    ApiError {
        status: reply.status.as_u16(),
        message,
        revision: body.and_then(|body| body.revision),
    }
}

pub(crate) fn parse<T: serde::de::DeserializeOwned>(reply: &Reply) -> Result<T, Remote> {
    if !reply.is_success() {
        return Err(Remote::Api(api_error(reply)));
    }
    serde_json::from_slice(&reply.body)
        .map_err(|err| Remote::Invalid(format!("unexpected response: {err}")))
}

pub(crate) fn record_path(code: &ShareCode) -> String {
    format!("{INSTANCES_PATH}/{}", code.as_str())
}

pub(crate) fn upload_body(
    meta: &ShareMeta,
    visibility: &str,
    provisioned: bool,
    device: Option<&str>,
) -> serde_json::Value {
    let mut body = serde_json::to_value(meta).unwrap_or_default();
    if let Some(fields) = body.as_object_mut() {
        fields.remove("created_at");
        fields.insert("visibility".into(), visibility.into());
        fields.insert("provisioned".into(), provisioned.into());
        if let Some(device) = device {
            fields.insert("device".into(), device.into());
        }
    }
    body
}

pub(crate) async fn push(
    account: &MinecraftAccount,
    method: Method,
    path: &str,
    if_match: Option<&str>,
    body: &serde_json::Value,
    payload: &Path,
    size: u64,
) -> Result<Reply, Remote> {
    let reserved: Reserved =
        parse(&instances::call(Some(account), method, path, if_match, Some(body)).await?)?;
    instances::upload_file(&reserved.upload.url, payload, size).await?;
    let committed = instances::call(
        Some(account),
        Method::POST,
        &format!("{}/commit", record_path(&reserved.id)),
        None,
        Some(&serde_json::json!({ "upload_id": reserved.upload.id })),
    )
    .await?;
    if !committed.is_success() {
        return Err(Remote::Api(api_error(&committed)));
    }
    Ok(committed)
}

pub(crate) async fn fetch_record(
    account: Option<&MinecraftAccount>,
    code: &ShareCode,
) -> Result<RemoteRecord, Remote> {
    parse(&instances::call(account, Method::GET, &record_path(code), None, None).await?)
}

pub(crate) async fn download_record(record: &RemoteRecord, dest: &Path) -> Result<(), Remote> {
    let url = record
        .download_url
        .as_deref()
        .ok_or_else(|| Remote::Invalid("the server sent no download link".into()))?;
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| Remote::Plus(err.into()))?;
    }
    instances::download_file(url, dest, record.meta.size.min(MAX_SHARE_BYTES)).await?;
    Ok(())
}

#[derive(Clone)]
pub struct HttpShareStore {
    auth: Arc<AuthService>,
}

impl HttpShareStore {
    #[must_use]
    pub fn new(auth: Arc<AuthService>) -> Self {
        Self { auth }
    }

    async fn account(&self) -> Result<MinecraftAccount, SharingError> {
        let account = self
            .auth
            .default_account_for_launch()
            .await
            .map_err(|err| SharingError::Unavailable(err.to_string()))?
            .filter(|account| account.kind == AccountKind::Microsoft && !account.signed_out);
        account.ok_or_else(|| {
            SharingError::Unavailable("sign in with a Microsoft account to share instances".into())
        })
    }

    async fn reader(&self) -> Option<MinecraftAccount> {
        self.auth
            .default_account_for_launch()
            .await
            .ok()
            .flatten()
            .filter(|account| account.kind == AccountKind::Microsoft && !account.signed_out)
    }

    fn remote(record: RemoteRecord) -> RemoteShare {
        RemoteShare {
            meta: record.meta,
            revision: record.revision,
            contents: record.contents,
            expires_at: record.expires_at,
            role: record.role,
        }
    }

    fn published(record: RemoteRecord) -> PublishedShare {
        PublishedShare {
            link: record.id.web_link(),
            code: record.id,
            manage_token: String::new(),
            expires_at: record.expires_at,
        }
    }
}

fn sharing_error(err: Remote, code: Option<&ShareCode>, size: u64) -> SharingError {
    match err {
        Remote::Plus(PlusError::ConsentRequired) => SharingError::Unavailable(
            "sharing needs the Terms of Service and Privacy Policy accepted in settings".into(),
        ),
        Remote::Plus(err) => SharingError::Unavailable(err.to_string()),
        Remote::Invalid(message) => SharingError::Unavailable(message),
        Remote::Api(err) => match (err.status, code) {
            (404, Some(code)) => SharingError::NotFound(code.clone()),
            (410, Some(code)) => SharingError::Gone(code.clone()),
            (403, Some(code)) => SharingError::Forbidden(code.clone()),
            (412 | 428, Some(code)) => SharingError::Outdated(code.clone()),
            (413, _) => SharingError::TooLarge {
                size,
                limit: MAX_SHARE_BYTES,
            },
            (422, _) => SharingError::Corrupt(err.message),
            (401, _) => SharingError::Unavailable(
                "sign in with a Microsoft account to share instances".into(),
            ),
            _ => SharingError::Unavailable(err.message),
        },
    }
}

#[async_trait]
impl ShareStore for HttpShareStore {
    fn name(&self) -> &'static str {
        "polyfrost"
    }

    async fn publish(
        &self,
        payload: &Path,
        meta: &ShareMeta,
    ) -> Result<PublishedShare, SharingError> {
        let account = self.account().await?;
        let body = upload_body(meta, PUBLIC, false, None);
        let committed = push(
            &account,
            Method::POST,
            INSTANCES_PATH,
            None,
            &body,
            payload,
            meta.size,
        )
        .await
        .map_err(|err| sharing_error(err, None, meta.size))?;
        let record: RemoteRecord =
            parse(&committed).map_err(|err| sharing_error(err, None, meta.size))?;
        Ok(Self::published(record))
    }

    async fn update(
        &self,
        share: &PublishedShare,
        payload: &Path,
        meta: &ShareMeta,
    ) -> Result<PublishedShare, SharingError> {
        let account = self.account().await?;
        let body = upload_body(meta, PUBLIC, false, None);
        let committed = push(
            &account,
            Method::PUT,
            &record_path(&share.code),
            None,
            &body,
            payload,
            meta.size,
        )
        .await
        .map_err(|err| sharing_error(err, Some(&share.code), meta.size))?;
        let record: RemoteRecord =
            parse(&committed).map_err(|err| sharing_error(err, Some(&share.code), meta.size))?;
        Ok(Self::published(record))
    }

    async fn describe(&self, code: &ShareCode) -> Result<RemoteShare, SharingError> {
        let reader = self.reader().await;
        let record = fetch_record(reader.as_ref(), code)
            .await
            .map_err(|err| sharing_error(err, Some(code), 0))?;
        if record.deleted {
            return Err(SharingError::Gone(code.clone()));
        }
        Ok(Self::remote(record))
    }

    async fn fetch(&self, code: &ShareCode, dest: &Path) -> Result<RemoteShare, SharingError> {
        let reader = self.reader().await;
        let record = fetch_record(reader.as_ref(), code)
            .await
            .map_err(|err| sharing_error(err, Some(code), 0))?;
        if record.deleted {
            return Err(SharingError::Gone(code.clone()));
        }
        download_record(&record, dest)
            .await
            .map_err(|err| sharing_error(err, Some(code), record.meta.size))?;
        Ok(Self::remote(record))
    }

    async fn update_from(
        &self,
        share: &PublishedShare,
        base_revision: u64,
        payload: &Path,
        meta: &ShareMeta,
    ) -> Result<PublishedShare, SharingError> {
        let account = self.account().await?;
        let body = upload_body(meta, PUBLIC, false, None);
        let if_match = format!("\"r{base_revision}\"");
        let committed = push(
            &account,
            Method::PUT,
            &record_path(&share.code),
            Some(&if_match),
            &body,
            payload,
            meta.size,
        )
        .await
        .map_err(|err| sharing_error(err, Some(&share.code), meta.size))?;
        let record: RemoteRecord =
            parse(&committed).map_err(|err| sharing_error(err, Some(&share.code), meta.size))?;
        Ok(Self::published(record))
    }

    async fn collaborators(&self, share: &ShareCode) -> Result<Vec<Collaborator>, SharingError> {
        #[derive(Deserialize)]
        struct Listed {
            collaborators: Vec<Collaborator>,
        }
        let account = self.account().await?;
        let reply = instances::call(
            Some(&account),
            Method::GET,
            &format!("{}/collaborators", record_path(share)),
            None,
            None,
        )
        .await
        .map_err(|err| sharing_error(err.into(), Some(share), 0))?;
        let listed: Listed = parse(&reply).map_err(|err| sharing_error(err, Some(share), 0))?;
        Ok(listed.collaborators)
    }

    async fn add_collaborator(
        &self,
        share: &ShareCode,
        username: &str,
    ) -> Result<Collaborator, SharingError> {
        let account = self.account().await?;
        let reply = instances::call(
            Some(&account),
            Method::POST,
            &format!("{}/collaborators", record_path(share)),
            None,
            Some(&serde_json::json!({ "username": username })),
        )
        .await
        .map_err(|err| sharing_error(err.into(), Some(share), 0))?;
        if reply.status.as_u16() == 400 {
            return Err(SharingError::Refused(api_error(&reply).message));
        }
        parse(&reply).map_err(|err| sharing_error(err, Some(share), 0))
    }

    async fn remove_collaborator(
        &self,
        share: &ShareCode,
        uuid: uuid::Uuid,
    ) -> Result<(), SharingError> {
        let account = self.account().await?;
        let reply = instances::call(
            Some(&account),
            Method::DELETE,
            &format!("{}/collaborators/{uuid}", record_path(share)),
            None,
            None,
        )
        .await
        .map_err(|err| sharing_error(err.into(), Some(share), 0))?;
        if reply.is_success() {
            return Ok(());
        }
        Err(sharing_error(
            Remote::Api(api_error(&reply)),
            Some(share),
            0,
        ))
    }

    async fn revoke(&self, share: &PublishedShare) -> Result<(), SharingError> {
        let account = self.account().await?;
        let reply = instances::call(
            Some(&account),
            Method::DELETE,
            &record_path(&share.code),
            None,
            None,
        )
        .await
        .map_err(|err| sharing_error(err.into(), Some(&share.code), 0))?;
        if reply.is_success() {
            return Ok(());
        }
        Err(sharing_error(
            Remote::Api(api_error(&reply)),
            Some(&share.code),
            0,
        ))
    }
}
