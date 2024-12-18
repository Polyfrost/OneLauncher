//! **HTTP Utilities**
//!
//! Async extensions and wrappers around [`reqwest`] functions.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use reqwest::Method;
use serde::de::DeserializeOwned;
use tokio::io::AsyncWriteExt;
use tokio::sync::{RwLock, Semaphore};

use crate::constants::FETCH_ATTEMPTS;
use crate::proxy::send::send_ingress;
use crate::proxy::IngressId;

use onelauncher_utils::io;
use onelauncher_utils::io::IOError;

use super::crypto;

/// A [`Semaphore`] used for all I/O operations.
#[derive(Debug)]
pub struct IoSemaphore(pub RwLock<Semaphore>);
/// A [`Semaphore`] used for all HTTP operations.
#[derive(Debug)]
pub struct FetchSemaphore(pub RwLock<Semaphore>);

lazy_static::lazy_static! {
	/// A public reqwest client with configured headers.
	pub static ref REQWEST_CLIENT: reqwest::Client = {
		let mut headers = reqwest::header::HeaderMap::new();
		let header = reqwest::header::HeaderValue::from_str(&format!(
			"{}/{} (https://polyfrost.org)",
			crate::constants::NAME,
			crate::constants::VERSION,
		)).expect("failed to build reqwest headers!");
		headers.insert(reqwest::header::USER_AGENT, header);
		reqwest::Client::builder()
			.tcp_keepalive(Some(std::time::Duration::from_secs(15)))
			.default_headers(headers)
			.build()
			.expect("failed to build reqwest client!")
	};
}

/// Basic HTTP fetch interface.
#[tracing::instrument(skip(semaphore))]
pub async fn fetch(
	url: &str,
	sha1: Option<&str>,
	semaphore: &FetchSemaphore,
) -> crate::Result<Bytes> {
	fetch_advanced(Method::GET, url, sha1, None, None, None, semaphore).await
}

/// Basic JSON fetch interface.
#[tracing::instrument(skip(json_body, semaphore))]
pub async fn fetch_json<T>(
	method: Method,
	url: &str,
	sha1: Option<&str>,
	json_body: Option<serde_json::Value>,
	semaphore: &FetchSemaphore,
) -> crate::Result<T>
where
	T: DeserializeOwned,
{
	let result = fetch_advanced(method, url, sha1, json_body, None, None, semaphore).await?;

	let value = serde_json::from_slice(&result)?;
	Ok(value)
}

/// Advanced HTTP fetch interface with json, sha1 validation, and ingress support.
#[tracing::instrument(skip(json_body, semaphore))]
#[onelauncher_macros::memory]
pub async fn fetch_advanced(
	method: Method,
	url: &str,
	sha1: Option<&str>,
	json_body: Option<serde_json::Value>,
	headers: Option<HashMap<&str, &str>>,
	ingress: Option<(&IngressId, f64)>,
	semaphore: &FetchSemaphore,
) -> crate::Result<Bytes> {
	let io_semaphore = semaphore.0.read().await;
	let _permit = io_semaphore.acquire().await?;

	for attempt in 1..=(FETCH_ATTEMPTS + 1) {
		let mut req = REQWEST_CLIENT.request(method.clone(), url);

		if let Some(body) = json_body.clone() {
			req = req.json(&body);
		}

		if let Some(headers) = &headers {
			for (key, value) in headers {
				req = req.header(*key, *value);
			}
		}

		let result = req.send().await;

		match result {
			Ok(x) => {
				let x = if !x.url().to_string().starts_with("https://resources") {
					tracing::info!(x)//.error_for_status()?;
				} else {
					x
				};

				let bytes = if let Some((feed, total)) = &ingress {
					let length = x.content_length();
					if let Some(total_size) = length {
						use futures::StreamExt;
						let mut stream = x.bytes_stream();
						let mut bytes = Vec::new();
						while let Some(item) = stream.next().await {
							let chunk =
								item.or(Err(anyhow::anyhow!("no value for fetch bytes")))?;
							bytes.append(&mut chunk.to_vec());
							send_ingress(
								feed,
								(chunk.len() as f64 / total_size as f64) * total,
								None,
							)
							.await?;
						}

						Ok(Bytes::from(bytes))
					} else {
						x.bytes().await
					}
				} else {
					x.bytes().await
				};

				if let Ok(bytes) = bytes {
					if let Some(sha1) = sha1 {
						let hash = crypto::sha1(&bytes);
						if &*hash != sha1 {
							if attempt <= 3 {
								continue;
							}
							return Err(anyhow::anyhow!(
								"hash {0} does not match {1}",
								sha1.to_string(),
								hash
							)
							.into());
						}
					}

					tracing::trace!("finished downloading {url}");
					return Ok(bytes);
				} else if attempt <= 3 {
					continue;
				} else if let Err(err) = bytes {
					return Err(err.into());
				}
			}
			Err(_) if attempt <= 3 => continue,
			Err(err) => {
				return Err(err.into());
			}
		}
	}

	unreachable!()
}

/// A utility to fetch from multiple mirrored sources.
#[tracing::instrument(skip(semaphore))]
#[onelauncher_macros::memory]
pub async fn fetch_from_mirrors(
	mirrors: &[&str],
	sha1: Option<&str>,
	semaphore: &FetchSemaphore,
) -> crate::Result<Bytes> {
	if mirrors.is_empty() {
		return Err(anyhow::anyhow!("no mirrors provided!").into());
	}

	for (index, mirror) in mirrors.iter().enumerate() {
		let result = fetch(mirror, sha1, semaphore).await;
		if result.is_ok() || (result.is_err() && index == (mirrors.len() - 1)) {
			return result;
		}
	}

	unreachable!()
}

/// Checks if we are playing offline by contacting a reliable server.
#[tracing::instrument]
#[onelauncher_macros::memory]
pub async fn check_internet_connection(timeout: u64) -> bool {
	REQWEST_CLIENT
		.get("https://api.polyfrost.org/")
		.timeout(std::time::Duration::from_secs(timeout))
		.send()
		.await
		.is_ok()
}

/// Post JSON to a specified URL (helpful for Microsoft auth and Metadata)
#[tracing::instrument(skip(json_body, semaphore))]
#[onelauncher_macros::memory]
pub async fn post_json<T>(
	url: &str,
	json_body: serde_json::Value,
	semaphore: &FetchSemaphore,
) -> crate::Result<T>
where
	T: DeserializeOwned,
{
	let io_semaphore = semaphore.0.read().await;
	let _permit = io_semaphore.acquire().await?;

	let req = REQWEST_CLIENT.post(url).json(&json_body);

	let result = req.send().await?.error_for_status()?;
	let value = result.json().await?;

	Ok(value)
}

/// Read JSON from a specified [`Path`].
pub async fn read_json<T>(path: &Path, semaphore: &IoSemaphore) -> crate::Result<T>
where
	T: DeserializeOwned,
{
	let io_semaphore = semaphore.0.read().await;
	let _permit = io_semaphore.acquire().await?;

	let json = io::read(path).await?;
	let json = serde_json::from_slice::<T>(&json)?;

	Ok(json)
}

/// Write to a file at a specified [`Path`].
#[tracing::instrument(skip(bytes, semaphore))]
pub async fn write<'a>(path: &Path, bytes: &[u8], semaphore: &IoSemaphore) -> crate::Result<()> {
	let io_semaphore = semaphore.0.read().await;
	let _permit = io_semaphore.acquire().await?;

	if let Some(parent) = path.parent() {
		io::create_dir_all(parent).await?;
	}

	let mut file = tokio::fs::File::create(path)
		.await
		.map_err(|e| IOError::with_path(e, path))?;
	file.write_all(bytes)
		.await
		.map_err(|e| IOError::with_path(e, path))?;
	tracing::trace!("done writing to file {}", path.display());
	Ok(())
}

/// Copy a file from one [`AsRef<Path>`] to another.
pub async fn copy(
	src: impl AsRef<Path>,
	dest: impl AsRef<Path>,
	semaphore: &IoSemaphore,
) -> crate::Result<()> {
	let src = src.as_ref();
	let dest = dest.as_ref();

	let io_semaphore = semaphore.0.read().await;
	let _permit = io_semaphore.acquire().await?;

	if let Some(parent) = dest.parent() {
		io::create_dir_all(parent).await?;
	}

	io::copy(src, dest).await?;
	tracing::trace!(
		"done copying file from {} to {}",
		src.display(),
		dest.display()
	);
	Ok(())
}

#[tracing::instrument(skip(bytes, io_semaphore))]
pub async fn write_icon(
	icon_path: &str,
	cache_dir: &Path,
	bytes: Bytes,
	io_semaphore: &IoSemaphore,
) -> crate::Result<PathBuf> {
	let ext = Path::new(&icon_path).extension().and_then(OsStr::to_str);
	let hash = crypto::sha1(&bytes);
	let path = cache_dir.join("icons").join(if let Some(e) = ext {
		format!("{hash}.{e}")
	} else {
		hash
	});

	write(&path, &bytes, io_semaphore).await?;

	let path = io::canonicalize(path)?;
	Ok(path)
}
