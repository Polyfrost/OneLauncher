#![allow(clippy::implicit_hasher)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};

use bytes::Bytes;
use reqwest::Method;
use serde::de::DeserializeOwned;
use tokio::sync::Semaphore;
use tokio_stream::StreamExt;

use crate::api::ingress::IngressSendExt;
use crate::error::LauncherResult;
use crate::store::Core;
use crate::store::ingress::{SubIngress, SubIngressExt};
use crate::store::semaphore::SemaphoreStore;

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

pub(crate) static REQWEST_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
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

#[tracing::instrument(level = "debug", skip(ingress))]
#[onelauncher_macro::pin]
pub async fn fetch_advanced(
	method: Method,
	url: &str,
	body: Option<serde_json::Value>,
	hash: Option<(HashAlgorithm, &str)>,
	headers: Option<HashMap<&str, &str>>,
	ingress: Option<&SubIngress<'_>>,
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
					tracing::error!(
						"error occurred whilst fetching on attempt {attempt}: {err} retrying request..."
					);
					continue;
				}

				return Err(err.into());
			}
			Ok(res) => break res,
		}
	};

	// Drop the fetch permit as we are no longer fetching
	drop(permit);

	let bytes = if let Some(ingress) = ingress {
		let length = res.content_length();
		if let Some(total_size) = length {
			let mut stream = res.bytes_stream();
			let mut bytes = Vec::new();

			while let Some(item) = stream.next().await {
				let chunk = item.or(Err(anyhow::anyhow!("no value for fetch bytes")))?;
				bytes.append(&mut chunk.to_vec());

				ingress
					.send_ingress((chunk.len() as f64 / total_size as f64) * ingress.total)
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
		match algorithm.hash(&bytes).await {
			Ok(calculated_hash) => {
				if *calculated_hash != expected_hash {
					return Err(CryptoError::InvalidHash {
						algorithm: algorithm.clone(),
						expected: expected_hash,
						actual: calculated_hash,
					}
					.into());
				}
			}
			Err(err) => {
				tracing::error!("failed to calculate hash for {url}: {err}");
				return Err(err.into());
			}
		}
	}

	tracing::debug!("finished fetching {url}");
	Ok(bytes)
}

pub async fn fetch(method: Method, url: &str) -> LauncherResult<Bytes> {
	fetch_advanced(method, url, None, None, None, None).await
}

pub async fn fetch_json<T: DeserializeOwned>(
	method: Method,
	url: &str,
	body: Option<serde_json::Value>,
	ingress: Option<&SubIngress<'_>>,
) -> LauncherResult<T> {
	fetch_json_advanced(method, url, body, None, None, ingress).await
}

pub async fn fetch_json_advanced<T: DeserializeOwned>(
	method: Method,
	url: &str,
	body: Option<serde_json::Value>,
	headers: Option<HashMap<&str, &str>>,
	hash: Option<(HashAlgorithm, &str)>,
	ingress: Option<&SubIngress<'_>>,
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
	ingress: Option<&SubIngress<'_>>,
) -> LauncherResult<Bytes> {
	const TASKS: f64 = 3.0;
	let ingress_step = ingress
		.ingress_total()
		.map(|total| total / TASKS)
		.unwrap_or_default();

	let bytes = fetch_advanced(
		method,
		url,
		body,
		hash,
		headers,
		ingress.ingress_sub(|total| total / TASKS).as_ref(),
	)
	.await?;
	let path = path.as_ref();

	ingress.send_ingress(ingress_step).await?;

	if let Some(parent) = path.parent() {
		io::create_dir_all(parent).await?;
	}

	io::write(path, &bytes).await?;

	ingress.send_ingress(ingress_step).await?;

	Ok(bytes)
}

pub async fn download(
	method: Method,
	url: &str,
	path: impl AsRef<Path>,
	hash: Option<(HashAlgorithm, &str)>,
	ingress: Option<&SubIngress<'_>>,
) -> LauncherResult<Bytes> {
	download_advanced(method, url, path, None, None, hash, ingress).await
}
