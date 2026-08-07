use std::path::{Path, PathBuf};

use reqwest::{Method, StatusCode, header};

use crate::error::RequestError;
use crate::service::RequestClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtagPolicy {
	CommitNow,
	/// Hands the ETag back on [`Fetched::etag`]
	/// Use when work downstream of the body can still fail so an interrupted run
	/// retries instead of getting 304
	Defer,
}

#[derive(Debug, Clone)]
pub struct Fetched {
	pub bytes: Vec<u8>,
	/// `false` when the server answered 304 or the request failed and the
	/// cached copy was served instead
	pub changed: bool,
	/// Present only for a fresh body under [`EtagPolicy::Defer`]
	pub etag: Option<String>,
}

impl Fetched {
	pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, RequestError> {
		serde_json::from_slice(&self.bytes).map_err(|source| RequestError::DeserializeError {
			source,
			type_name: std::any::type_name::<T>().to_string(),
			url: String::new(),
			status: 200,
			snippet: crate::error::body_snippet(&self.bytes),
		})
	}

	/// Lossy a bad byte becomes a replacement character rather than an error
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

/// Callers using [`EtagPolicy::Defer`] call this once the work depending on the
/// body has succeeded
pub async fn commit_etag(cache_path: &Path, etag: &str) {
	if let Err(err) = polyio::write_atomic(etag_path(cache_path), etag.as_bytes()).await {
		tracing::warn!("failed to write etag sidecar: {err}");
	}
}

/// Returns `Ok(None)` only when the request failed *and* there is no cached
/// copy
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
