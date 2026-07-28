//! Downloading with content verification.
//!
//! Streams and hashes in one pass, so nothing has to re-read a finished file
//! just to verify it.

use std::path::Path;

use oneclient_events::{EventBus, GroupedProgressChild, TaskPhase};
use polyio::{Sha1Stream, normalize_hash};
use reqwest::Method;

use crate::error::RequestError;
use crate::response::{ResponseExt, ResponseNotifyOptions, ResponseOptions};
use crate::service::RequestClient;

/// Whether a file already on disk matches `expected_sha1`.
///
/// Hashing an existing file is cheaper than re-downloading it, so every caller
/// wants this check before fetching. Returns `false` when the file is missing or
/// unreadable, in which case the caller should just download it.
pub async fn matches_on_disk(path: &Path, expected_sha1: &str) -> bool {
	if !path.is_file() {
		return false;
	}

	match polyio::sha1_file(path).await {
		Ok(actual) => normalize_hash(&actual) == normalize_hash(expected_sha1),
		Err(err) => {
			tracing::debug!("could not hash {}: {err}", path.display());
			false
		}
	}
}

/// Downloads `url` to `dest`, verifying SHA-1 from the bytes as they stream past.
///
/// On a mismatch the partial file is removed, so a retry cannot mistake it for a
/// complete download. `progress` is optional: pass a child when the caller is
/// running a grouped session, `None` for a one-off.
#[tracing::instrument(level = "debug", skip(client, events, progress), fields(%url))]
pub async fn download_verified(
	client: &RequestClient,
	events: &EventBus,
	url: &str,
	dest: &Path,
	expected_sha1: Option<&str>,
	expected_size: u64,
	progress: Option<GroupedProgressChild>,
) -> Result<(), RequestError> {
	// Callers that fan out over thousands of files (assets) pre-create the tree,
	// so this is usually a stat that avoids a blocking-pool round trip per file.
	if let Some(parent) = dest.parent()
		&& !parent.is_dir()
	{
		polyio::create_dir_all(parent).await?;
	}

	let request = reqwest::Request::new(Method::GET, url.parse()?);
	let response = client.send(request).await?;

	// Prefer the manifest size; fall back to Content-Length only when unknown.
	let total = if expected_size > 0 {
		expected_size
	} else {
		response.content_length().unwrap_or(0).max(1)
	};

	let options = ResponseOptions {
		notify: progress.clone().map(ResponseNotifyOptions::grouped),
	};
	let stream = response.stream(options, events).await?;

	let mut hasher = expected_sha1.map(|_| Sha1Stream::new());
	{
		let stream = futures_lite::StreamExt::map(stream, |item| {
			if let (Ok(chunk), Some(hasher)) = (&item, hasher.as_mut()) {
				hasher.update(chunk);
			}
			item
		});
		let stream = std::pin::pin!(stream);
		polyio::write_stream(dest, stream, Some(total)).await?;
	}

	if let (Some(expected), Some(hasher)) = (expected_sha1, hasher) {
		if let Some(child) = &progress {
			child.set_phase(TaskPhase::Verifying);
		}

		let actual = hasher.finish();
		if actual != normalize_hash(expected) {
			let _ = polyio::remove_file(dest).await;
			return Err(RequestError::HashMismatch {
				source_desc: dest.display().to_string(),
				expected: expected.to_string(),
				actual,
			});
		}
	}

	if let Some(child) = progress {
		child.finish();
	}

	Ok(())
}

/// Downloads `url` into memory, verifying SHA-1 over the collected bytes.
///
/// For payloads that are parsed rather than stored (version manifests), where
/// writing to disk only to read it straight back would be wasted IO.
#[tracing::instrument(level = "debug", skip(client, events, progress), fields(%url))]
pub async fn fetch_verified(
	client: &RequestClient,
	events: &EventBus,
	url: &str,
	expected_sha1: &str,
	progress: Option<GroupedProgressChild>,
) -> Result<Vec<u8>, RequestError> {
	let request = reqwest::Request::new(Method::GET, url.parse()?);
	let response = client.send(request).await?;

	let options = ResponseOptions {
		notify: progress.clone().map(ResponseNotifyOptions::grouped),
	};
	let stream = response.stream(options, events).await?;

	use futures_util::StreamExt;
	let mut stream = std::pin::pin!(stream);
	let mut bytes = Vec::new();
	while let Some(chunk) = stream.next().await {
		bytes.extend_from_slice(&chunk?);
	}

	if let Some(child) = &progress {
		child.set_phase(TaskPhase::Verifying);
	}

	let actual = polyio::sha1_bytes(&bytes);
	if normalize_hash(&actual) != normalize_hash(expected_sha1) {
		return Err(RequestError::HashMismatch {
			source_desc: url.to_string(),
			expected: expected_sha1.to_string(),
			actual,
		});
	}

	if let Some(child) = progress {
		child.finish();
	}

	Ok(bytes)
}
