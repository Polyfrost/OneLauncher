use oneclient_events::{EventBus, GroupedProgressSession, TaskCategory};
use oneclient_net::RequestClient;

use crate::error::McResult;

/// The progress child is created before the request goes out otherwise small
/// files dominated by header wait never appear in the UI
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip(requester, events, progress, label, expected_sha1), fields(%url), level = "debug")]
pub async fn download_to_path(
    requester: &RequestClient,
    events: &EventBus,
    progress: &GroupedProgressSession,
    label: impl Into<String>,
    category: TaskCategory,
    expected_size: u64,
    url: &str,
    dest: &std::path::Path,
    expected_sha1: Option<&str>,
) -> McResult<()> {
    let child = progress.child(label, expected_size, category);

    // Every hash in a Minecraft manifest is SHA-1 so the algorithm is implied
    let expected = expected_sha1.map(polyio::Checksum::sha1);

    oneclient_net::download_verified(
        requester,
        events,
        url,
        dest,
        expected.as_ref(),
        expected_size,
        Some(oneclient_net::ResponseNotifyOptions::grouped(child)),
    )
    .await?;

    Ok(())
}

/// Downloads into memory instead of to disk for payloads that are parsed
/// rather than stored
#[tracing::instrument(skip(requester, events, progress, label), fields(%url), level = "debug")]
#[allow(clippy::too_many_arguments)]
pub async fn fetch_bytes_verified(
    requester: &RequestClient,
    events: &EventBus,
    progress: &GroupedProgressSession,
    label: impl Into<String>,
    category: TaskCategory,
    expected_size: u64,
    url: &str,
    expected_sha1: &str,
) -> McResult<Vec<u8>> {
    let child = progress.child(label, expected_size, category);

    Ok(oneclient_net::fetch_verified(requester, events, url, expected_sha1, Some(child)).await?)
}
