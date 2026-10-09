use std::path::Path;

use serde::Deserialize;

use crate::error::ClusterResult;
use oneclient_net::RequestError;

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

#[tracing::instrument(skip(net))]
pub async fn upload_log_at(
    net: &oneclient_net::RequestClient,
    path: &Path,
) -> ClusterResult<MclogsUploadResponse> {
    let path = ensure_allowed(path)?;
    let mut content = super::censor(&read_file_string(&path).await?).into_owned();

    let mut truncated = false;
    let line_count = content.lines().count();
    if line_count > MAX_LINES {
        truncated = true;
        content = content
            .lines()
            .skip(line_count - MAX_LINES)
            .collect::<Vec<_>>()
            .join("\n");
    }
    let mut encoded = form_encoded_len(content.as_bytes());
    if encoded > MAX_BYTES {
        truncated = true;
        let bytes = content.as_bytes();
        let mut cut = 0;
        while encoded > MAX_BYTES && cut < bytes.len() {
            encoded -= form_encoded_byte_len(bytes[cut]);
            cut += 1;
        }
        while cut < content.len() && !content.is_char_boundary(cut) {
            cut += 1;
        }
        content = content[cut..].to_string();
    }

    let response = net
        .http()
        .post(MCLOGS_URL)
        .form(&[("content", content.as_str())])
        .send()
        .await
        .map_err(RequestError::ReqwestError)?;

    let status = response.status();
    let bytes = response.bytes().await.map_err(RequestError::ReqwestError)?;

    if !status.is_success() {
        let reason = serde_json::from_slice::<MclogsResponse>(&bytes)
            .ok()
            .and_then(|r| r.error)
            .unwrap_or_else(|| status.to_string());
        tracing::warn!(
            %status,
            body = %String::from_utf8_lossy(&bytes[..bytes.len().min(512)]),
            "mclogs upload rejected"
        );
        return Err(LogsError::Upload(reason).into());
    }

    let parsed: MclogsResponse = serde_json::from_slice(&bytes)?;

    if !parsed.success {
        let reason = parsed.error.unwrap_or_else(|| "unknown error".into());
        tracing::warn!(reason = %reason, "mclogs upload failed");
        return Err(LogsError::Upload(reason).into());
    }

    tracing::info!(
        url = parsed.url.as_deref().unwrap_or(""),
        "uploaded log to mclo.gs"
    );

    Ok(MclogsUploadResponse {
        id: parsed.id.unwrap_or_default(),
        url: parsed.url.unwrap_or_default(),
        raw: parsed.raw.unwrap_or_default(),
        truncated,
    })
}

fn form_encoded_byte_len(byte: u8) -> usize {
    match byte {
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' | b' ' => 1,
        _ => 3,
    }
}

fn form_encoded_len(bytes: &[u8]) -> usize {
    bytes.iter().map(|&b| form_encoded_byte_len(b)).sum()
}
