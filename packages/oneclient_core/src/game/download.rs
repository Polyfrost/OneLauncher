use reqwest::Method;

use crate::crypto::{normalize_hash, sha1_bytes, sha1_file};
use crate::game::GameError;
use crate::http::{RequestClient, ResponseExt, ResponseNotifyOptions, ResponseOptions};
use crate::notification::{GroupedProgressSession, NotificationService, TaskCategory, TaskPhase};
use crate::{LauncherError, LauncherResult};

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip(requester, notifier, progress, label, expected_sha1), fields(%url), level = "debug")]
pub async fn download_to_path(
    requester: &RequestClient,
    notifier: &NotificationService,
    progress: &GroupedProgressSession,
    label: impl Into<String>,
    category: TaskCategory,
    expected_size: u64,
    url: &str,
    dest: &std::path::Path,
    expected_sha1: Option<&str>,
) -> LauncherResult<()> {
    if let Some(parent) = dest.parent() {
        polyio::create_dir_all(parent).await?;
    }

    let label = label.into();
    let parsed_url = url.parse().map_err(LauncherError::UrlError)?;
    let request = reqwest::Request::new(Method::GET, parsed_url);
    let response = requester.send(request).await?;

    // Prefer the manifest size; fall back to Content-Length only when unknown.
    let total = if expected_size > 0 {
        expected_size
    } else {
        response.content_length().unwrap_or(0).max(1)
    };
    let child = progress.child(label, total, category);

    let stream = response
        .stream(
            ResponseOptions {
                notify: Some(ResponseNotifyOptions::grouped(child.clone())),
            },
            notifier,
        )
        .await?;
    let stream = std::pin::pin!(stream);
    polyio::write_stream(dest, stream).await?;

    if let Some(expected) = expected_sha1 {
        child.set_phase(TaskPhase::Verifying);
        let actual = sha1_file(dest).await?;
        if normalize_hash(&actual) != normalize_hash(expected) {
            let _ = polyio::remove_file(dest).await;
            return Err(GameError::HashMismatch {
                path: dest.display().to_string(),
                expected: expected.to_string(),
                actual,
            }
            .into());
        }
    }

    child.finish();
    Ok(())
}

#[tracing::instrument(skip(requester, notifier, progress, label), fields(%url), level = "debug")]
pub async fn fetch_bytes_verified(
    requester: &RequestClient,
    notifier: &NotificationService,
    progress: &GroupedProgressSession,
    label: impl Into<String>,
    category: TaskCategory,
    expected_size: u64,
    url: &str,
    expected_sha1: &str,
) -> LauncherResult<Vec<u8>> {
    let label = label.into();
    let parsed_url: url::Url = url.parse().map_err(LauncherError::UrlError)?;
    let request = reqwest::Request::new(Method::GET, parsed_url.clone());
    let response = requester.send(request).await?;

    let total = if expected_size > 0 {
        expected_size
    } else {
        response.content_length().unwrap_or(0).max(1)
    };
    let child = progress.child(label, total, category);

    let stream = response
        .stream(
            ResponseOptions {
                notify: Some(ResponseNotifyOptions::grouped(child.clone())),
            },
            notifier,
        )
        .await?;

    use futures_util::StreamExt;
    let mut stream = std::pin::pin!(stream);
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
    }

    child.set_phase(TaskPhase::Verifying);
    let actual = sha1_bytes(&bytes);
    if normalize_hash(&actual) != normalize_hash(expected_sha1) {
        return Err(GameError::HashMismatch {
            path: parsed_url.to_string(),
            expected: expected_sha1.to_string(),
            actual,
        }
        .into());
    }

    child.finish();
    Ok(bytes)
}
