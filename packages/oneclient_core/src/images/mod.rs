use std::collections::HashMap;
use std::path::PathBuf;

use bytes::Bytes;
use reqwest::Method;
use tokio::sync::Mutex;
use tracing::instrument;

use crate::crypto::sha1_bytes;
use crate::http::RequestError;
use crate::paths;
use crate::state::LauncherServices;
use crate::LauncherResult;

pub const DEFAULT_IMAGE_EDGE: u32 = 1600;

#[derive(Debug, Default)]
pub struct ImageCacheStore {
    memory: Mutex<HashMap<String, Bytes>>,
}

impl ImageCacheStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[instrument(skip(self, services))]
    pub async fn get(
        &self,
        services: &LauncherServices,
        url: &str,
        max_edge: u32,
    ) -> LauncherResult<Bytes> {
        let mem_key = Self::mem_key(url, max_edge);
        if let Some(hit) = self.memory.lock().await.get(&mem_key).cloned() {
            return Ok(hit);
        }

        let path = Self::disk_path(url, max_edge)?;

        if let Ok(bytes) = polyio::read(&path).await {
            let bytes = Bytes::from(bytes);
            self.memory.lock().await.insert(mem_key, bytes.clone());
            return Ok(bytes);
        }

        let raw = download(services, url).await?;
        let bytes = downscale_if_oversized(raw, max_edge).await;

        if let Some(parent) = path.parent() {
            polyio::create_dir_all(parent).await?;
        }
        if let Err(err) = polyio::write(&path, &bytes).await {
            tracing::warn!("failed to persist cached image for {url}: {err}");
        }

        self.memory.lock().await.insert(mem_key, bytes.clone());

        Ok(bytes)
    }

    fn mem_key(url: &str, max_edge: u32) -> String {
        format!("{max_edge}|{url}")
    }

    fn disk_path(url: &str, max_edge: u32) -> LauncherResult<PathBuf> {
        let mut name = sha1_bytes(url.as_bytes());
        name.push('_');
        name.push_str(&max_edge.to_string());
        if let Some(ext) = extension_from_url(url) {
            name.push('.');
            name.push_str(&ext);
        }
        Ok(paths::images_cache_dir()?.join(name))
    }
}

static DECODE_LIMIT: std::sync::LazyLock<tokio::sync::Semaphore> =
    std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(2));

async fn downscale_if_oversized(bytes: Bytes, max_edge: u32) -> Bytes {
    let _permit = DECODE_LIMIT.acquire().await;
    let candidate = bytes.clone();
    match tokio::task::spawn_blocking(move || downscale(&candidate, max_edge)).await {
        Ok(Some(smaller)) => smaller,
        _ => bytes,
    }
}

fn downscale(bytes: &[u8], max_edge: u32) -> Option<Bytes> {
    let img = image::load_from_memory(bytes).ok()?;
    if img.width().max(img.height()) <= max_edge {
        return None;
    }

    let resized = img.resize(max_edge, max_edge, image::imageops::FilterType::Lanczos3);

    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    if resized.color().has_alpha() {
        resized.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
    } else {
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 85)
            .encode_image(&resized)
            .ok()?;
    }

    Some(Bytes::from(out))
}

async fn download(services: &LauncherServices, url: &str) -> LauncherResult<Bytes> {
    let parsed = url.parse().map_err(RequestError::from)?;
    let request = reqwest::Request::new(Method::GET, parsed);
    let res = services.requester.send(request).await?;
    let bytes = res.bytes().await.map_err(RequestError::from)?;
    Ok(bytes)
}

fn extension_from_url(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name.rsplit_once('.')?.1;
    if ext.is_empty() || ext.len() > 5 || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}
