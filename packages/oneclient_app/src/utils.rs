use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use chrono::{Datelike, NaiveDate};
use oneclient_core::clusters::Cluster;
use oneclient_core::packages::domain::GameLoader;
use oneclient_core::{VersionKey, format_mc_version, parse_mc_version};

pub type ClusterGroups = BTreeMap<u32, Vec<Cluster>>;

fn cluster_key(cluster: &Cluster) -> Option<VersionKey> {
    parse_mc_version(&cluster.mc_version).and_then(|parsed| parsed.key())
}

pub fn group_clusters_by_major(clusters: &[Cluster]) -> ClusterGroups {
    let mut groups: ClusterGroups = BTreeMap::new();

    for cluster in clusters {
        let Some(parsed) = parse_mc_version(&cluster.mc_version) else {
            continue;
        };
        groups
            .entry(parsed.major)
            .or_default()
            .push(cluster.clone());
    }

    for list in groups.values_mut() {
        list.sort_by_key(|b| std::cmp::Reverse(cluster_key(b)));
    }

    groups
}

pub fn major_pretty_name(major: u32) -> String {
    if major >= 26 {
        format!("{major}.x")
    } else {
        format!("1.{major}")
    }
}

pub fn loader_tags(clusters: &[Cluster]) -> Vec<String> {
    let mut tags = Vec::new();
    for cluster in clusters {
        if cluster.mc_loader.is_modded() {
            let label = cluster.mc_loader.to_string();
            if !tags.iter().any(|t| t == &label) {
                tags.push(label);
            }
        }
    }
    tags.sort();
    tags
}

pub fn version_keys(clusters: &[Cluster]) -> Vec<VersionKey> {
    let mut keys = Vec::new();
    for cluster in clusters {
        if let Some(key) = cluster_key(cluster)
            && !keys.contains(&key)
        {
            keys.push(key);
        }
    }
    keys.sort();
    keys
}

pub fn loaders_for_major(clusters: &[Cluster]) -> Vec<GameLoader> {
    let mut loaders = Vec::new();
    for cluster in clusters {
        if cluster.mc_loader.is_modded() && !loaders.contains(&cluster.mc_loader) {
            loaders.push(cluster.mc_loader);
        }
    }
    loaders.sort_by_key(|loader| loader.to_string());
    loaders
}

pub fn resolve_cluster(
    clusters: &[Cluster],
    key: Option<VersionKey>,
    loader: Option<GameLoader>,
) -> Option<Cluster> {
    clusters
        .iter()
        .find(|cluster| {
            let version_ok = key.is_none_or(|key| cluster_key(cluster) == Some(key));
            let loader_ok = loader.is_none_or(|loader| cluster.mc_loader == loader);
            version_ok && loader_ok
        })
        .cloned()
}

pub fn default_major(groups: &ClusterGroups, active: Option<Cluster>) -> Option<u32> {
    if let Some(cluster) = active
        && let Some(parsed) = parse_mc_version(&cluster.mc_version)
    {
        return Some(parsed.major);
    }
    groups.keys().next().copied()
}

pub fn default_version_key(
    clusters: &[Cluster],
    preferred: Option<VersionKey>,
) -> Option<VersionKey> {
    let keys = version_keys(clusters);
    if keys.is_empty() {
        return None;
    }
    if let Some(key) = preferred
        && keys.contains(&key)
    {
        return Some(key);
    }
    keys.last().copied()
}

pub fn default_loader(clusters: &[Cluster], preferred: Option<GameLoader>) -> Option<GameLoader> {
    let loaders = loaders_for_major(clusters);
    if loaders.is_empty() {
        return clusters.first().map(|c| c.mc_loader);
    }
    if let Some(loader) = preferred
        && loaders.contains(&loader)
    {
        return Some(loader);
    }
    Some(loaders[0])
}

pub fn version_label(major: u32, (minor, patch): VersionKey) -> String {
    format_mc_version(major, minor, patch)
}

/// Human-readable byte size (B / KB / MB).
pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.;
    const MB: f64 = KB * 1024.;
    let b = bytes as f64;
    if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

pub fn format_res((w, h): (u32, u32)) -> String {
    format!("{w}×{h}")
}

/// `7384` -> `2h 3m`, `540` -> `9m`, `0` -> `0m`.
pub fn format_duration_hm(secs: i64) -> String {
    if secs <= 0 {
        return "0m".to_string();
    }
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

/// `3723` -> `1h 2m`, `83` -> `1m 23s`, `45` -> `45s`.
pub fn format_duration_hms(secs: i64) -> String {
    if secs <= 0 {
        return "0s".to_string();
    }
    if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

/// Compact large counts: `1500` -> `1.5K`, `2_400_000` -> `2.4M`.
pub fn abbreviate_number(n: u64) -> String {
    let f = n as f64;
    if f >= 1_000_000.0 {
        format!("{:.1}M", f / 1_000_000.0)
    } else if f >= 1_000.0 {
        format!("{:.1}K", f / 1_000.0)
    } else {
        n.to_string()
    }
}

/// English plural suffix: `""` for 1, `"s"` otherwise.
pub fn plural(n: i64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// 24-hour clock label: `9` -> `09:00`.
pub fn format_hour(hour: usize) -> String {
    format!("{hour:02}:00")
}

pub fn parse_day(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// `2026-07-05` -> `Jul 5`.
pub fn format_day(date: NaiveDate) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{} {}", MONTHS[date.month0() as usize], date.day())
}

pub fn sort_clusters_for_home(mut clusters: Vec<Cluster>) -> Vec<Cluster> {
    clusters.sort_by(compare_last_played);
    clusters
}

fn compare_last_played(a: &Cluster, b: &Cluster) -> Ordering {
    match (a.last_played, b.last_played) {
        // Most recently played first.
        (Some(a), Some(b)) => b.cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        // Never played: latest version first (major, then minor).
        (None, None) => version_sort_key(b).cmp(&version_sort_key(a)),
    }
}

fn version_sort_key(cluster: &Cluster) -> (u32, u32, u32) {
    parse_mc_version(&cluster.mc_version)
        .map(|v| (v.major, v.minor.unwrap_or(0), v.patch.unwrap_or(0)))
        .unwrap_or((0, 0, 0))
}

#[cfg(not(target_os = "linux"))]
pub fn is_wayland() -> bool {
    false
}

#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    static IS_WAYLAND: OnceLock<bool> = OnceLock::new();

    *IS_WAYLAND.get_or_init(|| {
        if cfg!(target_os = "linux") {
            std::env::var("XDG_SESSION_TYPE")
                .map(|v| v == "wayland")
                .unwrap_or(false)
        } else {
            false
        }
    })
}
