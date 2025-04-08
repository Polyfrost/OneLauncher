#![allow(clippy::implicit_hasher)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};

use bytes::Bytes;
use reqwest::Method;
use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio_stream::StreamExt;

use crate::api::ingress::send_ingress;
use crate::error::LauncherResult;
use crate::store::ingress::IngressRef;
use crate::store::semaphore::SemaphoreStore;
use crate::store::Core;

use super::crypto::{CryptoError, HashAlgorithm};
use super::io;

#[derive(Debug)]
pub struct FetchSemaphore(pub Arc<Semaphore>);

impl FetchSemaphore {
	#[must_use]
	pub fn new(limit: usize) -> Self {
		Self(Arc::new(Semaphore::new(limit)))
	}
}

static REQWEST_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
	reqwest::ClientBuilder::new()
		.tcp_keepalive(Some(std::time::Duration::from_secs(15)))
		.use_rustls_tls()
		.default_headers({
			let mut headers = reqwest::header::HeaderMap::new();
			let header = reqwest::header::HeaderValue::from_str(&format!(
				"{}/{} ({})",
				Core::get().launcher_name,
				Core::get().launcher_version,
				Core::get().launcher_website,
			))
			.expect("failed to build reqwest headers!");
			headers.insert(reqwest::header::USER_AGENT, header);
			headers
		})
		.build()
		.expect("failed to build reqwest client!")
});

#[tracing::instrument(level = "debug", skip(body, headers, ingress_ref))]
#[onelauncher_macro::pin]
pub async fn fetch_advanced(
	method: Method,
	url: &str,
	body: Option<serde_json::Value>,
	hash: Option<(HashAlgorithm, &str)>,
	headers: Option<HashMap<&str, &str>>,
	ingress_ref: Option<&IngressRef<'_>>,
) -> LauncherResult<Bytes> {
	let semaphore = SemaphoreStore::fetch().await;
	let permit = semaphore.acquire().await;

	let client = &REQWEST_CLIENT;

	let mut attempt = 0;
	let res = loop {
		attempt += 1;
		let mut req = client.request(method.clone(), url);

		if let Some(body) = body.clone() {
			req = req.json(&body);
		}

		if let Some(headers) = &headers {
			for (key, value) in headers {
				req = req.header(*key, *value);
			}
		}

		match req.send().await {
			Err(err) => {
				if attempt <= Core::get().fetch_attempts {
					tracing::error!("error occurred whilst fetching on attempt {attempt}: {err} retrying request...");
					continue;
				}

				return Err(err.into())
			},
			Ok(res) => break res
		}
	};

	// Drop the fetch permit as we are no longer fetching
	drop(permit);

	let bytes = if let Some(ingress_ref) = ingress_ref {
		let length = res.content_length();
		if let Some(total_size) = length {
			let mut stream = res.bytes_stream();
			let mut bytes = Vec::new();

			while let Some(item) = stream.next().await {
				let chunk =
					item.or(Err(anyhow::anyhow!("no value for fetch bytes")))?;
				bytes.append(&mut chunk.to_vec());

				send_ingress(
					ingress_ref,
					(chunk.len() as f64 / total_size as f64) * ingress_ref.increment_by,
				)
				.await?;
			}

			Ok(Bytes::from(bytes))
		} else {
			res.bytes().await
		}
	} else {
		res.bytes().await
	}?;

	if let Some((ref algorithm, expected_hash)) = hash {
		let expected_hash = expected_hash.to_string();
		let calculated_hash = algorithm.hash(&bytes);

		if *calculated_hash != expected_hash {
			return Err(CryptoError::InvalidHash {
				algorithm: algorithm.clone(),
				expected: expected_hash,
				actual: calculated_hash,
			}
			.into());
		}
	}

	tracing::debug!("finished downloading {url}");
	Ok(bytes)
}

pub async fn fetch(method: Method, url: &str) -> LauncherResult<Bytes> {
	fetch_advanced(method, url, None, None, None, None).await
}

pub async fn fetch_json<T: for<'de> Deserialize<'de>>(
	method: Method,
	url: &str,
	body: Option<serde_json::Value>,
	ingress_ref: Option<&IngressRef<'_>>,
) -> LauncherResult<T> {
	fetch_json_advanced(method, url, body, None, None, ingress_ref).await
}

pub async fn fetch_json_advanced<T: for<'de> Deserialize<'de>>(
	method: Method,
	url: &str,
	body: Option<serde_json::Value>,
	headers: Option<HashMap<&str, &str>>,
	hash: Option<(HashAlgorithm, &str)>,
	ingress: Option<&IngressRef<'_>>,
) -> LauncherResult<T> {
	let bytes = fetch_advanced(method, url, body, hash, headers, ingress).await?;
	Ok(serde_json::from_slice(&bytes)?)
}

pub async fn download_advanced(
	method: Method,
	url: &str,
	path: impl AsRef<Path>,
	body: Option<serde_json::Value>,
	headers: Option<HashMap<&str, &str>>,
	hash: Option<(HashAlgorithm, &str)>,
	ingress: Option<&IngressRef<'_>>,
) -> LauncherResult<Bytes> {
	let bytes = fetch_advanced(
		method,
		url,
		body,
		hash,
		headers,
		ingress.map(|i| i.with_increment(i.increment_by / 2.0)).as_ref(),
	)
	.await?;
	let path = path.as_ref();

	if let Some(ingress_ref) = ingress {
		send_ingress(ingress_ref, ingress_ref.increment_by / 4.0).await?;
	}

	if let Some(parent) = path.parent() {
		io::create_dir_all(parent).await?;
	}

	io::write(path, &bytes).await?;

	if let Some(ingress_ref) = ingress {
		send_ingress(ingress_ref, ingress_ref.increment_by / 4.0).await?;
	}

	Ok(bytes)
}

pub async fn download(
	method: Method,
	url: &str,
	path: impl AsRef<Path>,
	hash: Option<(HashAlgorithm, &str)>,
	ingress_ref: Option<&IngressRef<'_>>
) -> LauncherResult<Bytes> {
	download_advanced(method, url, path, None, None, hash, ingress_ref).await
}
