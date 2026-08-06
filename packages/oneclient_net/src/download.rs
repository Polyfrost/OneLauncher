//! Downloading with content verification.
//!
//! Streams and hashes in one pass, so nothing has to re-read a finished file
//! just to verify it.

use std::path::Path;

use oneclient_events::{EventBus, GroupedProgressChild, TaskPhase};
use polyio::{Checksum, ChecksumStream, normalize_hash};
use reqwest::Method;

use crate::error::RequestError;
use crate::response::{ResponseExt, ResponseNotifyOptions, ResponseOptions};
use crate::service::RequestClient;

/// How many times one file is fetched before it counts as failed.
///
/// [`RequestClient::send`] already retries what fails before the response
/// headers land; this covers what fails after, which on an unstable connection
/// is most of it. Without a retry here a single dropped chunk permanently fails
/// that one file and the install is quietly left incomplete.
const MAX_DOWNLOAD_ATTEMPTS: u32 = 3;

/// Whether fetching the same URL again has a realistic chance of working.
///
/// A transport error, a hash mismatch, or a short body means the bytes arrived
/// wrong, which is exactly what a retry fixes. Everything else — a 404, a bad
/// URL, a full disk — will fail the same way every time, and retrying only
/// multiplies the delay before the user sees the real error.
fn worth_retrying(err: &RequestError) -> bool {
    matches!(
        err,
        RequestError::ReqwestError(_)
            | RequestError::HashMismatch { .. }
            | RequestError::IncompleteBody { .. }
    )
}

/// Grows with each attempt, so a link that is briefly saturated gets a chance to
/// recover instead of being hit again immediately.
fn retry_delay(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_millis(300 * u64::from(attempt))
}

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

/// Downloads `url` to `dest`, verifying it from the bytes as they stream past.
///
/// Hashing happens on the bytes in flight, so verification costs no extra disk
/// read and a good file is never hashed twice. `dest` is only created once the
/// hash matches — a failed or corrupt attempt leaves nothing behind for a later
/// `exists()` check to trip over. Transport failures and mismatches are retried
/// up to [`MAX_DOWNLOAD_ATTEMPTS`] times before the error surfaces.
///
/// `notify` is optional: pass [`ResponseNotifyOptions::grouped`] when the caller
/// is running a grouped session, [`ResponseNotifyOptions::standalone`] for a
/// one-off with its own notification, or `None` to download silently.
#[tracing::instrument(level = "debug", skip(client, events, notify), fields(%url))]
pub async fn download_verified(
	client: &RequestClient,
	events: &EventBus,
	url: &str,
	dest: &Path,
	expected: Option<&Checksum>,
	expected_size: u64,
	notify: Option<ResponseNotifyOptions>,
) -> Result<(), RequestError> {
	// Callers that fan out over thousands of files (assets) pre-create the tree,
	// so this is usually a stat that avoids a blocking-pool round trip per file.
	if let Some(parent) = dest.parent()
		&& !parent.is_dir()
	{
		polyio::create_dir_all(parent).await?;
	}

	// A standalone notification needs a fixed id up front, or each retry opens a
	// new entry in the UI instead of updating the one already there.
	let notify = notify.map(ResponseNotifyOptions::pinned);

	let mut attempt = 1;
	loop {
		match download_attempt(
			client,
			events,
			url,
			dest,
			expected,
			expected_size,
			notify.as_ref(),
		)
		.await
		{
			Ok(()) => break,
			Err(err) if attempt < MAX_DOWNLOAD_ATTEMPTS && worth_retrying(&err) => {
				tracing::warn!(
					attempt,
					%url,
					"download failed, retrying: {err}"
				);
				tokio::time::sleep(retry_delay(attempt)).await;
				attempt += 1;
			}
			Err(err) => return Err(err),
		}
	}

	if let Some(child) = notify.as_ref().and_then(ResponseNotifyOptions::child) {
		child.finish();
	}

	Ok(())
}

/// One pass of [`download_verified`]: fetch, hash in flight, publish on match.
///
/// Split out so a retry re-enters with a fresh request and a fresh hasher. The
/// progress child is shared across attempts and reports absolute byte counts, so
/// an attempt that restarts simply rewinds its bar rather than double-counting.
#[allow(clippy::too_many_arguments)]
async fn download_attempt(
	client: &RequestClient,
	events: &EventBus,
	url: &str,
	dest: &Path,
	expected: Option<&Checksum>,
	expected_size: u64,
	notify: Option<&ResponseNotifyOptions>,
) -> Result<(), RequestError> {
	let child = notify.and_then(ResponseNotifyOptions::child);
	if let Some(child) = child {
		child.set_phase(TaskPhase::Downloading);
	}

	let request = reqwest::Request::new(Method::GET, url.parse()?);
	let response = client.send(request).await?;

	// Without this, an error body is written to disk under the requested file's name.
	let status = response.status();
	if !status.is_success() {
		let bytes = response.bytes().await?;
		return Err(RequestError::HttpStatus {
			status: status.as_u16(),
			url: url.to_string(),
			snippet: crate::error::body_snippet(&bytes),
		});
	}

	// Prefer the manifest size; Content-Length is absent for compressed or chunked bodies.
	let declared = if expected_size > 0 {
		Some(expected_size)
	} else {
		response.content_length()
	};
	let total = declared.unwrap_or(0).max(1);

	let options = ResponseOptions {
		notify: notify.cloned(),
	};
	let stream = response.stream(options, events).await?;

	let mut hasher = expected.map(|expected| ChecksumStream::new(expected.algorithm));
	let mut received = 0u64;
	{
		let stream = futures_lite::StreamExt::map(stream, |item| {
			if let Ok(chunk) = &item {
				received += chunk.len() as u64;
				if let Some(hasher) = hasher.as_mut() {
					hasher.update(chunk);
				}
			}
			item
		});
		let stream = std::pin::pin!(stream);
		// Writes to a scratch sibling and renames on success, so `dest` is either
		// absent or complete — never a truncated file a later run would skip.
		polyio::write_stream(dest, stream, Some(total)).await?;
	}

	if let (Some(expected), Some(hasher)) = (expected, hasher) {
		if let Some(child) = child {
			child.set_phase(TaskPhase::Verifying);
		}

		let actual = hasher.finish();
		if !expected.matches(&actual) {
			// The bytes were wrong, so the file that was just published is wrong
			// too. Remove it rather than leave it for the next `exists()` check.
			let _ = polyio::remove_file(dest).await;
			return Err(RequestError::HashMismatch {
				source_desc: dest.display().to_string(),
				expected: format!("{} {}", expected.algorithm.name(), expected.hex),
				actual,
			});
		}
	} else if let Some(declared) = declared
		&& received != declared
	{
		// With no hash, the byte count is the only evidence the transfer finished.
		let _ = polyio::remove_file(dest).await;
		return Err(RequestError::IncompleteBody {
			source_desc: dest.display().to_string(),
			expected: declared,
			actual: received,
		});
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
