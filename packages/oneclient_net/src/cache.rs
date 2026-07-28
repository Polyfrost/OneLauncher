//! ETag-conditional GET backed by an on-disk cache.
//!
//! Four call sites had a byte-identical copy of the same three helpers
//! (`insert_if_none_match`, `etag_from_response`, `read_sidecar_etag`) plus the
//! same arm-for-arm 304 / 2xx / error / other match around them. They differed
//! only in the cache filename, the log noun, and whether a total failure was an
//! error or an empty result.

use std::path::{Path, PathBuf};

use reqwest::{Method, StatusCode, header};

use crate::error::RequestError;
use crate::service::RequestClient;

/// Whether the ETag sidecar is written as soon as a fresh body arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtagPolicy {
	/// Write it immediately. Right when the body alone is the whole payload.
	CommitNow,
	/// Leave it alone and hand it back on [`Fetched::etag`]. Use when work
	/// downstream of the body can still fail: the bundle catalog only commits
	/// its ETag once every bundle in it has actually downloaded, so an
	/// interrupted sync retries instead of reporting 304 and skipping them.
	Defer,
}

/// The outcome of a conditional fetch.
#[derive(Debug, Clone)]
pub struct Fetched {
	pub bytes: Vec<u8>,
	/// `false` when the server answered 304, or the request failed and the
	/// cached copy was served instead. Callers use this to skip work that only
	/// matters when the document actually changed.
	pub changed: bool,
	/// The server's ETag, present only for a fresh body under
	/// [`EtagPolicy::Defer`].
	pub etag: Option<String>,
}

impl Fetched {
	/// Deserialize the body as JSON.
	pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, RequestError> {
		serde_json::from_slice(&self.bytes).map_err(|source| RequestError::DeserializeError {
			source,
			type_name: std::any::type_name::<T>().to_string(),
			url: String::new(),
			status: 200,
			snippet: crate::error::body_snippet(&self.bytes),
		})
	}

	/// Body as UTF-8, lossily. Used for documents that are text by contract
	/// (changelogs), where refusing to render because of one bad byte is worse
	/// than showing a replacement character.
	#[must_use]
	pub fn text(&self) -> String {
		String::from_utf8_lossy(&self.bytes).into_owned()
	}
}

fn etag_path(cache_path: &Path) -> PathBuf {
	let mut path = cache_path.as_os_str().to_os_string();
	path.push(".etag");
	PathBuf::from(path)
}

async fn read_sidecar_etag(cache_path: &Path) -> Option<String> {
	polyio::read(etag_path(cache_path))
		.await
		.ok()
		.and_then(|bytes| String::from_utf8(bytes).ok())
		.filter(|etag| !etag.is_empty())
}

/// Writes the ETag sidecar for `cache_path`. Callers using [`EtagPolicy::Defer`]
/// call this once the work depending on the body has succeeded.
pub async fn commit_etag(cache_path: &Path, etag: &str) {
	if let Err(err) = polyio::write_atomic(etag_path(cache_path), etag.as_bytes()).await {
		tracing::warn!("failed to write etag sidecar: {err}");
	}
}

/// `GET url`, using `cache_path` as both the conditional-request cache and the
/// offline fallback.
///
/// Returns `Ok(None)` only when the request failed *and* there is no cached
/// copy. Callers decide whether that is fatal.
#[tracing::instrument(level = "debug", skip(client), fields(%url))]
pub async fn fetch_cached(
	client: &RequestClient,
	url: &str,
	cache_path: &Path,
	policy: EtagPolicy,
) -> Result<Option<Fetched>, RequestError> {
	let stored_etag = read_sidecar_etag(cache_path).await;
	let mut request = reqwest::Request::new(Method::GET, url.parse()?);

	if let Some(etag) = &stored_etag
		&& let Ok(value) = header::HeaderValue::from_str(etag)
	{
		request.headers_mut().insert(header::IF_NONE_MATCH, value);
	}

	let cached = || async {
		match polyio::read(cache_path).await {
			Ok(bytes) => Ok(Some(Fetched {
				bytes,
				changed: false,
				etag: None,
			})),
			Err(err) => {
				tracing::debug!("no usable cache at {}: {err}", cache_path.display());
				Ok(None)
			}
		}
	};

	match client.send(request).await {
		Ok(res) if res.status() == StatusCode::NOT_MODIFIED => {
			tracing::debug!("cache hit (304)");
			cached().await
		}
		Ok(res) if res.status().is_success() => {
			let server_etag = res
				.headers()
				.get(header::ETAG)
				.and_then(|value| value.to_str().ok())
				.map(str::to_string);

			let bytes = res.bytes().await?.to_vec();
			polyio::write_atomic(cache_path, &bytes).await?;

			let etag = match (&server_etag, policy) {
				(Some(etag), EtagPolicy::CommitNow) => {
					commit_etag(cache_path, etag).await;
					None
				}
				(_, EtagPolicy::Defer) => server_etag,
				(None, EtagPolicy::CommitNow) => None,
			};

			Ok(Some(Fetched {
				bytes,
				changed: true,
				etag,
			}))
		}
		Ok(res) => {
			tracing::warn!(status = %res.status(), "unexpected response; using cache if present");
			cached().await
		}
		Err(err) => {
			tracing::debug!("fetch failed, using cache if present: {err}");
			cached().await
		}
	}
}
