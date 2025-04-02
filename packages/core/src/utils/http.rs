#![allow(clippy::implicit_hasher)]

use std::{collections::HashMap, path::Path, sync::Arc};

use bytes::Bytes;
use reqwest::Method;
use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio_stream::StreamExt;

use crate::{api::ingress::send_ingress, constants, error::LauncherResult, store::{ingress::IngressId, State}};

use super::{crypto::{CryptoError, HashAlgorithm}, io};

#[derive(Debug)]
pub struct FetchSemaphore(pub Arc<Semaphore>);

impl FetchSemaphore {
	#[must_use]
	pub fn new(limit: usize) -> Self {
		Self(Arc::new(Semaphore::new(limit)))
	}
}

pub fn create_client() -> LauncherResult<reqwest::Client> {
	Ok(reqwest::ClientBuilder::new()
		.tcp_keepalive(Some(std::time::Duration::from_secs(15)))
		.use_rustls_tls()
		.default_headers({
			let mut headers = reqwest::header::HeaderMap::new();
			let header = reqwest::header::HeaderValue::from_str(&format!(
				"{}/{} (https://polyfrost.org)",
				constants::NAME,
				constants::VERSION,
			))
			.expect("failed to build reqwest headers!");
			headers.insert(reqwest::header::USER_AGENT, header);
			headers
		})
		.build()?)
}

#[tracing::instrument(skip(body, headers, ingress))]
pub async fn fetch_advanced(
	method: Method,
	url: &str,
	body: Option<serde_json::Value>,
	hash: Option<(HashAlgorithm, &str)>,
	headers: Option<HashMap<&str, &str>>,
	ingress: Option<(&IngressId, f64)>,
) -> LauncherResult<Bytes> {
	let state = State::get().await?;
	let _permit = state.fetch_semaphore.0.acquire().await?;
	let client = &state.client;

	for attempt in 0..constants::FETCH_ATTEMPTS {
		let mut req = client.request(method.clone(), url);

		if let Some(body) = body.clone() {
			req = req.json(&body);
		}

		if let Some(headers) = &headers {
			for (key, value) in headers {
				req = req.header(*key, *value);
			}
		}

		let res = req.send().await;

		match res {
			Err(_) if attempt <= 3 => {},
			Err(err) => return Err(err.into()),
			Ok(res) => {
				let bytes = if let Some((ingress_id, total)) = ingress {
					let length = res.content_length();
					if let Some(total_size) = length {
						let mut stream = res.bytes_stream();
						let mut bytes = Vec::new();

						while let Some(item) = stream.next().await {
							let chunk = item.or(Err(anyhow::anyhow!("no value for fetch bytes")))?;
							bytes.append(&mut chunk.to_vec());

							send_ingress(
								ingress_id,
								(chunk.len() as f64 / total_size as f64) * total,
							)
							.await?;
						}

						Ok(Bytes::from(bytes))
					} else {
						res.bytes().await
					}
				} else {
					res.bytes().await
				};

				if let Ok(bytes) = bytes {
					if let Some((ref algorithm, expected_hash)) = hash {
						let expected_hash = expected_hash.to_string();
						let calculated_hash = algorithm.hash(&bytes);

						if *calculated_hash != expected_hash {
							if attempt <= 3 {
								continue;
							}

							return Err(CryptoError::InvalidHash {
								algorithm: algorithm.clone(),
								expected: expected_hash,
								actual: calculated_hash
							}.into());
						}
					}

					tracing::debug!("finished downloading {url}");
					return Ok(bytes);
				} else if let Err(err) = bytes {
					return Err(err.into());
				}
			},
		}
	}

	unreachable!()
}

pub async fn fetch(method: Method, url: &str) -> LauncherResult<Bytes> {
	fetch_advanced(method, url, None, None, None, None).await
}

pub async fn fetch_json<T: for<'de> Deserialize<'de>>(
	method: Method,
	url: &str,
	body: Option<serde_json::Value>,
	headers: Option<HashMap<&str, &str>>,
) -> LauncherResult<T> {
	fetch_json_advanced(method, url, body, None, headers, None).await
}

pub async fn fetch_json_advanced<T: for<'de> Deserialize<'de>>(
	method: Method,
	url: &str,
	body: Option<serde_json::Value>,
	hash: Option<(HashAlgorithm, &str)>,
	headers: Option<HashMap<&str, &str>>,
	ingress: Option<(&IngressId, f64)>,
) -> LauncherResult<T> {
	let bytes = fetch_advanced(method, url, body, hash, headers, ingress).await?;
	Ok(serde_json::from_slice(&bytes)?)
}

pub async fn download_advanced(
	method: Method,
	url: &str,
	path: impl AsRef<Path>,
	body: Option<serde_json::Value>,
	hash: Option<(HashAlgorithm, &str)>,
	headers: Option<HashMap<&str, &str>>,
	ingress: Option<(&IngressId, f64)>,
) -> LauncherResult<()> {
	let bytes = fetch_advanced(method, url, body, hash, headers, ingress.map(|(id, total)| (id, total / 2.0))).await?;
	let path = path.as_ref();

	if let Some((id, total)) = ingress {
		send_ingress(id, total / 4.0).await?;
	}

	if let Some(parent) = path.parent() {
		io::create_dir_all(parent).await?;
	}

	io::write(path, &bytes).await?;

	if let Some((id, total)) = ingress {
		send_ingress(id, total / 4.0).await?;
	}

	Ok(())
}

pub async fn download(
	method: Method,
	url: &str,
	path: impl AsRef<Path>,
	hash: Option<(HashAlgorithm, &str)>
) -> LauncherResult<()> {
	download_advanced(method, url, path, None, hash, None, None).await
}
