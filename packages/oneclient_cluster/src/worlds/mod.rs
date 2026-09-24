use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;
use thiserror::Error;

use crate::cluster::Cluster;
use crate::error::ClusterResult;

const SAVES_DIR: &str = "saves";
const DATAPACKS_DIR: &str = "datapacks";
const LEVEL_DAT: &str = "level.dat";
const WORLD_ICON: &str = "icon.png";
const PACK_META: &str = "pack.mcmeta";

#[derive(Debug, Error)]
pub enum WorldsError {
    #[error("not a plain file or folder name: {0}")]
    InvalidName(String),
    #[error("world '{0}' not found")]
    NotFound(String),
    #[error("failed to move to trash: {0}")]
    Trash(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldInfo {
    pub folder_name: String,
    pub path: PathBuf,
    pub icon: Option<PathBuf>,
    pub last_played: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataPackInfo {
    pub file_name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified: DateTime<Utc>,
    pub description: Option<String>,
}

fn plain_name(name: &str) -> ClusterResult<&str> {
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(name),
        _ => Err(WorldsError::InvalidName(name.to_string()).into()),
    }
}

fn world_dir(cluster: &Cluster, world: &str) -> ClusterResult<PathBuf> {
    let dir = cluster.game_dir()?.join(SAVES_DIR).join(plain_name(world)?);
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err(WorldsError::NotFound(world.to_string()).into())
    }
}

fn modified_or_now(meta: &std::fs::Metadata) -> DateTime<Utc> {
    meta.modified()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now())
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => dir_size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

fn move_to_trash(path: &Path) -> ClusterResult<()> {
    trash::delete(path).map_err(|err| WorldsError::Trash(err.to_string()).into())
}

#[tracing::instrument(level = "debug", skip(cluster), fields(cluster_id = cluster.id))]
pub fn list_cluster_worlds(cluster: &Cluster) -> ClusterResult<Vec<WorldInfo>> {
    let dir = cluster.game_dir()?.join(SAVES_DIR);
    let mut out = Vec::new();

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if !path.is_dir() {
            continue;
        }

        let Ok(level_meta) = std::fs::metadata(path.join(LEVEL_DAT)) else {
            continue;
        };

        let icon = path.join(WORLD_ICON);

        out.push(WorldInfo {
            folder_name: name.to_string(),
            icon: icon.is_file().then_some(icon),
            last_played: modified_or_now(&level_meta),
            path,
        });
    }

    out.sort_by_key(|w| std::cmp::Reverse(w.last_played));
    Ok(out)
}

#[tracing::instrument(level = "debug", skip(cluster), fields(cluster_id = cluster.id))]
pub fn world_size(cluster: &Cluster, world: &str) -> ClusterResult<u64> {
    Ok(dir_size(&world_dir(cluster, world)?))
}

#[tracing::instrument(level = "debug", skip(cluster), fields(cluster_id = cluster.id))]
pub fn delete_world(cluster: &Cluster, world: &str) -> ClusterResult<()> {
    move_to_trash(&world_dir(cluster, world)?)
}

#[tracing::instrument(level = "debug", skip(cluster), fields(cluster_id = cluster.id))]
pub async fn list_world_datapacks(
    cluster: &Cluster,
    world: &str,
) -> ClusterResult<Vec<DataPackInfo>> {
    let dir = world_dir(cluster, world)?.join(DATAPACKS_DIR);
    let mut found = tokio::task::spawn_blocking(move || scan_datapacks(&dir))
        .await
        .map_err(std::io::Error::other)?;

    for (path, info) in &mut found {
        if !info.is_dir {
            info.description = zip_description(path).await;
        }
    }

    let mut out: Vec<DataPackInfo> = found.into_iter().map(|(_, info)| info).collect();
    out.sort_by_key(|p| p.file_name.to_lowercase());
    Ok(out)
}

fn scan_datapacks(dir: &Path) -> Vec<(PathBuf, DataPackInfo)> {
    let mut out = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };

        let is_dir = meta.is_dir();
        let is_pack = if is_dir {
            path.join(PACK_META).is_file()
        } else {
            name.to_ascii_lowercase().ends_with(".zip")
        };
        if !is_pack {
            continue;
        }

        let info = DataPackInfo {
            file_name: name.to_string(),
            is_dir,
            size_bytes: if is_dir { dir_size(&path) } else { meta.len() },
            modified: modified_or_now(&meta),
            description: if is_dir {
                std::fs::read(path.join(PACK_META))
                    .ok()
                    .and_then(|raw| parse_description(&raw))
            } else {
                None
            },
        };
        out.push((path, info));
    }

    out
}

#[tracing::instrument(level = "debug", skip(cluster), fields(cluster_id = cluster.id))]
pub async fn add_world_datapacks(
    cluster: &Cluster,
    world: &str,
    files: &[PathBuf],
) -> ClusterResult<()> {
    let dir = world_dir(cluster, world)?.join(DATAPACKS_DIR);
    polyio::create_dir_all(&dir).await?;

    for file in files {
        let Some(name) = file.file_name() else {
            continue;
        };
        let target = dir.join(name);
        if is_same_file(file, &target) {
            continue;
        }
        if file.is_dir() {
            polyio::copy_dir(file, &target, &[]).await?;
        } else {
            polyio::copy(file, &target).await?;
        }
    }

    Ok(())
}

#[tracing::instrument(level = "debug", skip(cluster), fields(cluster_id = cluster.id))]
pub fn delete_world_datapack(cluster: &Cluster, world: &str, file_name: &str) -> ClusterResult<()> {
    let path = world_dir(cluster, world)?
        .join(DATAPACKS_DIR)
        .join(plain_name(file_name)?);
    move_to_trash(&path)
}

fn is_same_file(a: &Path, b: &Path) -> bool {
    match (polyio::canonicalize(a), polyio::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

async fn zip_description(path: &Path) -> Option<String> {
    let (_, raw) = polyio::read_zip_file_entries(path, |name| name == PACK_META)
        .await
        .ok()?
        .into_iter()
        .next()?;
    parse_description(&raw)
}

fn parse_description(raw: &[u8]) -> Option<String> {
    let raw = raw.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(raw);
    let meta: Value = serde_json::from_slice(raw).ok()?;
    let text = strip_formatting(flatten_text(meta.get("pack")?.get("description")?).trim());
    (!text.is_empty()).then_some(text)
}

fn flatten_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(flatten_text).collect(),
        Value::Object(map) => {
            let mut out = map.get("text").map(flatten_text).unwrap_or_default();
            if let Some(extra) = map.get("extra") {
                out.push_str(&flatten_text(extra));
            }
            out
        }
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
    }
}

fn strip_formatting(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}
