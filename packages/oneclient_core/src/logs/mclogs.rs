use std::path::Path;

use serde::Deserialize;

use crate::http::RequestError;
use crate::{LauncherResult, LauncherServices};

use super::manage::{ensure_allowed, read_file_string};
use super::{LogsError, MclogsUploadResponse};

const MCLOGS_URL: &str = "https://api.mclo.gs/1/log";

const MAX_LINES: usize = 25_000;
const MAX_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize)]
struct MclogsResponse {
    success: bool,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    raw: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[tracing::instrument(skip(services))]
pub async fn upload_log_at(
    services: &LauncherServices,
    path: &Path,
) -> LauncherResult<MclogsUploadResponse> {
    let path = ensure_allowed(path)?;
    let mut content = read_file_string(&path).await?;

    let line_count = content.lines().count();
    if line_count > MAX_LINES {
        content = content
            .lines()
            .skip(line_count - MAX_LINES)
            .collect::<Vec<_>>()
            .join("\n");
    }
    if content.len() > MAX_BYTES {
        let mut cut = content.len() - MAX_BYTES;
        while cut < content.len() && !content.is_char_boundary(cut) {
            cut += 1;
        }
        content = content[cut..].to_string();
    }

    let response = services
        .requester
        .http()
        .post(MCLOGS_URL)
        .form(&[("content", content.as_str())])
        .send()
        .await
        .map_err(RequestError::ReqwestError)?;

    let bytes = response.bytes().await.map_err(RequestError::ReqwestError)?;
    let parsed: MclogsResponse = serde_json::from_slice(&bytes)?;

    if !parsed.success {
        let reason = parsed.error.unwrap_or_else(|| "unknown error".into());
        tracing::warn!(reason = %reason, "mclogs upload failed");
        return Err(LogsError::Upload(reason).into());
    }

    tracing::info!(url = parsed.url.as_deref().unwrap_or(""), "uploaded log to mclo.gs");

    Ok(MclogsUploadResponse {
        id: parsed.id.unwrap_or_default(),
        url: parsed.url.unwrap_or_default(),
        raw: parsed.raw.unwrap_or_default(),
    })
}
