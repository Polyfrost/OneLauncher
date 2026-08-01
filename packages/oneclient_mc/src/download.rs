//! Minecraft-flavoured wrappers over [`oneclient_net`]'s verified downloads.
//!
//! All that is left here is the part specific to the game install: naming the
//! grouped-progress child and its category.

use oneclient_events::{EventBus, GroupedProgressSession, TaskCategory};
use oneclient_net::RequestClient;

use crate::error::McResult;

/// Downloads `url` to `dest` as one child of `progress`, verifying SHA-1 as the
/// bytes stream past.
///
/// The child is created *before* the request goes out: waiting on response
/// headers is most of a small file's lifetime, and a child that only existed
/// while bytes were moving would never appear in the UI at all.
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

    // Every hash in a Minecraft manifest is SHA-1, so callers here pass a bare
    // string and the algorithm is implied.
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

/// Downloads `url` into memory, verifying SHA-1 over the collected bytes.
///
/// For payloads that get parsed rather than stored, where writing to disk only
/// to read it straight back would be wasted IO.
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
