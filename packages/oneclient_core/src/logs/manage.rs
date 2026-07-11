use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::LauncherResult;
use crate::clusters::Cluster;
use crate::paths;

use super::parse::parse_level;
use super::{LogFileInfo, LogKind, LogLevel, LogLine, LogsError, ReadOptions};

pub(super) fn ensure_allowed(path: &Path) -> LauncherResult<PathBuf> {
    let canon = std::fs::canonicalize(path).map_err(LogsError::Io)?;

    for root in [paths::logs_dir()?, paths::clusters_dir()?] {
        if let Ok(root) = std::fs::canonicalize(&root)
            && canon.starts_with(&root)
        {
            return Ok(canon);
        }
    }

    Err(LogsError::InvalidName(path.display().to_string()).into())
}

fn push_file(path: PathBuf, name: String, kind: LogKind, out: &mut Vec<LogFileInfo>) {
    let Ok(meta) = std::fs::metadata(&path) else {
        return;
    };
    if !meta.is_file() {
        return;
    }
    let modified = meta
        .modified()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now());

    out.push(LogFileInfo {
        name,
        kind,
        size_bytes: meta.len(),
        modified,
        path,
    });
}

fn collect_dir(
    dir: &Path,
    kind: LogKind,
    accept: impl Fn(&str) -> bool,
    out: &mut Vec<LogFileInfo>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !accept(name) {
            continue;
        }
        push_file(path.clone(), name.to_string(), kind, out);
    }
}

pub fn list_cluster_logs(cluster: &Cluster) -> LauncherResult<Vec<LogFileInfo>> {
    let mut out = Vec::new();

    let dir = cluster.dir()?;

    collect_dir(
        &dir.join("logs"),
        LogKind::Minecraft,
        |name| name.ends_with(".log") || name.ends_with(".log.gz"),
        &mut out,
    );

    collect_dir(
        &dir.join("crash-reports"),
        LogKind::CrashReport,
        |name| name.ends_with(".txt") || name.ends_with(".log"),
        &mut out,
    );

    push_file(
        cluster_output_log(cluster)?,
        "Launcher output".to_string(),
        LogKind::Game {
            cluster_id: cluster.id,
        },
        &mut out,
    );

    out.sort_by_key(|info| std::cmp::Reverse(info.modified));
    Ok(out)
}

pub fn cluster_output_log(cluster: &Cluster) -> LauncherResult<PathBuf> {
    Ok(cluster.dir()?.join("cluster-output.log"))
}

pub(super) fn lines_from(content: &str, opts: &ReadOptions) -> Vec<LogLine> {
    let mut prev = LogLevel::Unknown;
    let mut lines: Vec<LogLine> = Vec::new();
    for (idx, raw) in content.lines().enumerate() {
        let level = match parse_level(raw) {
            Some(level) => {
                prev = level;
                level
            }
            None => prev,
        };
        lines.push(LogLine {
            number: idx + 1,
            level,
            text: raw.to_string(),
        });
    }

    if let Some(filter) = opts.level_filter {
        lines.retain(|l| l.level == filter);
    }
    if let Some(query) = opts.search.as_ref().filter(|q| !q.is_empty()) {
        let query = query.to_lowercase();
        lines.retain(|l| l.text.to_lowercase().contains(&query));
    }
    if let Some(max) = opts.max_lines
        && lines.len() > max
    {
        lines.drain(0..lines.len() - max);
    }

    lines
}

pub(super) async fn read_file_string(path: &Path) -> LauncherResult<String> {
    let is_gz = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("gz"));

    if is_gz {
        Ok(polyio::read_gz_to_string(path).await?)
    } else {
        Ok(polyio::read_to_string(path).await?)
    }
}

pub async fn read_log_at(path: &Path, opts: &ReadOptions) -> LauncherResult<Vec<LogLine>> {
    let path = ensure_allowed(path)?;
    let content = read_file_string(&path).await?;
    Ok(lines_from(&content, opts))
}

pub async fn delete_log_at(path: &Path) -> LauncherResult<()> {
    let path = ensure_allowed(path)?;

    Ok(polyio::remove_file(&path).await?)
}
