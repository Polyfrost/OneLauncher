use std::path::Path;
use std::time::Duration;

use bytes::Bytes;
use futures_util::TryStreamExt;
use reqwest::header::{CONTENT_LENGTH, IF_MATCH};
use reqwest::{Method, StatusCode};
use tokio::io::AsyncWriteExt;

use oneclient_auth::MinecraftAccount;

use crate::api::{client, token_for};
use crate::{PlusError, REQUEST_TIMEOUT, base_url, plus_blocked};

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60 * 60);

pub const INSTANCES_PATH: &str = "/v1/oneclient/shares";

#[derive(Debug, Clone)]
pub struct Reply {
    pub status: StatusCode,
    pub body: Bytes,
}

impl Reply {
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}

pub async fn call(
    account: Option<&MinecraftAccount>,
    method: Method,
    path: &str,
    if_match: Option<&str>,
    body: Option<&serde_json::Value>,
) -> Result<Reply, PlusError> {
    if plus_blocked() {
        return Err(PlusError::ConsentRequired);
    }
    if account.is_some_and(|account| !account.is_microsoft()) {
        return Err(PlusError::OfflineAccount);
    }

    let url = format!("{}{path}", base_url());
    let mut stale: Option<String> = None;
    loop {
        let token = match account {
            Some(account) => Some(token_for(account, stale.as_deref()).await?),
            None => None,
        };
        let mut request = client()?
            .request(method.clone(), &url)
            .timeout(REQUEST_TIMEOUT)
            .header("Accept", "application/json");
        if let Some(token) = &token {
            request = request.bearer_auth(token);
        }
        if let Some(if_match) = if_match {
            request = request.header(IF_MATCH, if_match);
        }
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await?;
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED
            && stale.is_none()
            && let Some(token) = token
        {
            stale = Some(token);
            continue;
        }
        return Ok(Reply {
            status,
            body: response.bytes().await?,
        });
    }
}

pub async fn upload_file(url: &str, path: &Path, size: u64) -> Result<(), PlusError> {
    let file = tokio::fs::File::open(path).await?;
    let response = client()?
        .put(url)
        .timeout(TRANSFER_TIMEOUT)
        .header(CONTENT_LENGTH, size)
        .body(reqwest::Body::wrap_stream(
            tokio_util::io::ReaderStream::new(file),
        ))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(PlusError::Api {
            status: response.status().as_u16(),
            body: response.text().await.unwrap_or_default(),
        });
    }
    Ok(())
}

pub async fn download_file(url: &str, dest: &Path, limit: u64) -> Result<(), PlusError> {
    let response = client()?.get(url).timeout(TRANSFER_TIMEOUT).send().await?;
    if !response.status().is_success() {
        return Err(PlusError::Api {
            status: response.status().as_u16(),
            body: response.text().await.unwrap_or_default(),
        });
    }

    let mut file = tokio::fs::File::create(dest).await?;
    let mut written = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.try_next().await? {
        written += chunk.len() as u64;
        if written > limit {
            drop(file);
            tokio::fs::remove_file(dest).await.ok();
            return Err(PlusError::Api {
                status: StatusCode::PAYLOAD_TOO_LARGE.as_u16(),
                body: format!("the download is larger than {limit} bytes"),
            });
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(())
}
