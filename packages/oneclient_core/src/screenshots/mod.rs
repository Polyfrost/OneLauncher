use std::path::{Path, PathBuf};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::clusters::Cluster;
use crate::paths;
use crate::LauncherResult;

#[derive(Debug, Error)]
pub enum ScreenshotsError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("path is not inside a cluster directory: {0}")]
    InvalidPath(String),
    #[error("failed to move screenshot to trash: {0}")]
    Trash(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenshotInfo {
    pub name: String,
    pub size_bytes: u64,
    pub created: DateTime<Utc>,
    pub resolution: Option<(u32, u32)>,
    pub path: PathBuf,
}

fn is_image(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")
}

fn ensure_in_clusters(path: &Path) -> LauncherResult<PathBuf> {
    let canon = std::fs::canonicalize(path).map_err(ScreenshotsError::Io)?;
    if let Ok(root) = std::fs::canonicalize(paths::clusters_dir()?)
        && canon.starts_with(&root)
    {
        return Ok(canon);
    }
    Err(ScreenshotsError::InvalidPath(path.display().to_string()).into())
}

pub fn list_cluster_screenshots(cluster: &Cluster) -> LauncherResult<Vec<ScreenshotInfo>> {
    let dir = cluster.dir()?.join("screenshots");
    let mut out = Vec::new();

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if !is_image(name) {
            continue;
        }

        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };

        if !meta.is_file() {
            continue;
        }

        let created = meta
            .created()
            .or_else(|_| meta.modified())
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());

        let resolution = image::image_dimensions(&path).ok();

        out.push(ScreenshotInfo {
            name: name.to_string(),
            size_bytes: meta.len(),
            created,
            resolution,
            path,
        });
    }

    out.sort_by_key(|s| std::cmp::Reverse(s.created));
    Ok(out)
}

pub fn load_screenshot(path: &Path, max_edge: Option<u32>) -> LauncherResult<Bytes> {
    let path = ensure_in_clusters(path)?;
    let raw = std::fs::read(&path).map_err(ScreenshotsError::Io)?;
    let bytes = match max_edge {
        Some(edge) => thumbnail(&raw, edge).unwrap_or_else(|| Bytes::from(raw)),
        None => Bytes::from(raw),
    };
    Ok(bytes)
}

fn thumbnail(raw: &[u8], max_edge: u32) -> Option<Bytes> {
    let img = image::load_from_memory(raw).ok()?;
    if img.width().max(img.height()) <= max_edge {
        return None;
    }
    let resized = img.resize(max_edge, max_edge, image::imageops::FilterType::Triangle);

    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    resized.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
    Some(Bytes::from(out))
}

pub fn delete_screenshot(path: &Path) -> LauncherResult<()> {
    let path = ensure_in_clusters(path)?;
    
    // TODO: maybe async?
    match trash::delete(&path) {
        Ok(()) => Ok(()),
        Err(err) => Err(ScreenshotsError::Trash(err.to_string()).into()),
    }
}
