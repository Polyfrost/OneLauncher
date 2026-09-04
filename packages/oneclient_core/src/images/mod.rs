use std::collections::HashMap;
use std::path::PathBuf;

use bytes::Bytes;
use reqwest::Method;
use tokio::sync::Mutex;

use polyio::sha1_bytes;
use oneclient_net::RequestError;
use oneclient_common::paths;
use oneclient_net::RequestClient;
use crate::{LauncherError, LauncherResult};

pub const DEFAULT_IMAGE_EDGE: u32 = 1600;

/// Image urls come from untrusted remote descriptions, so cap what is fetched and decoded.
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct ImageCacheStore {
    memory: Mutex<HashMap<String, Bytes>>,
}

impl ImageCacheStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[tracing::instrument(level = "debug", skip(self, net))]
    pub async fn get(
        &self,
        net: &RequestClient,
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

        // icon unpacked for a jar is already on disk
        let raw = match paths::local_image_path(url) {
            Some(source) => Bytes::from(polyio::read(&source).await?),
            None => download(net, url).await?,
        };
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

/// Icons come out of user supplied jars so the decoder gets a ceiling rather
/// than whatever the file's header claims
const MAX_DECODE_EDGE: u32 = 8192;
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;

fn downscale(bytes: &[u8], max_edge: u32) -> Option<Bytes> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DECODE_EDGE);
    limits.max_image_height = Some(MAX_DECODE_EDGE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);

    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes));
    reader.limits(limits);

    let img = reader
        .with_guessed_format()
        .ok()?
        .decode()
        .inspect_err(|err| tracing::warn!("refused to decode an image: {err}"))
        .ok()?;
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

#[tracing::instrument(level = "debug", skip(net))]
async fn download(net: &RequestClient, url: &str) -> LauncherResult<Bytes> {
    let parsed: reqwest::Url = url.parse().map_err(RequestError::from)?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(refused(url, "unsupported scheme"));
    }
    if !is_public_host(&parsed) {
        return Err(refused(url, "host is not public"));
    }

    let request = reqwest::Request::new(Method::GET, parsed);
    let mut res = net.send(request).await?;

    if res
        .content_length()
        .is_some_and(|len| len > MAX_IMAGE_BYTES as u64)
    {
        return Err(refused(url, "declared body is too large"));
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(RequestError::from)? {
        if bytes.len() + chunk.len() > MAX_IMAGE_BYTES {
            return Err(refused(url, "body is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok(Bytes::from(bytes))
}

fn refused(url: &str, reason: &str) -> LauncherError {
    LauncherError::StdIoError(std::io::Error::other(format!(
        "refused to fetch image {url}: {reason}"
    )))
}

fn is_public_host(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return is_public_ip(ip);
    }

    let host = host.to_ascii_lowercase();
    host != "localhost" && !host.ends_with(".localhost")
}

fn is_public_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 0x40)
        }
        std::net::IpAddr::V6(ip) => match ip.to_ipv4_mapped() {
            Some(ip) => is_public_ip(ip.into()),
            None => {
                !(ip.is_loopback()
                    || ip.is_unspecified()
                    || (ip.segments()[0] & 0xfe00) == 0xfc00
                    || (ip.segments()[0] & 0xffc0) == 0xfe80)
            }
        },
    }
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
