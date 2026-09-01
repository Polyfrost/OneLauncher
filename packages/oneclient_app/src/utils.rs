use std::cmp::Ordering;

use chrono::{Datelike, NaiveDate};
use oneclient_core::clusters::Cluster;
use oneclient_common::parse_mc_version;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

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

/// Prevents user from choosing his max amount of ram preset (e.g Someone has 16GB of RAM, 
/// so the max preset is 16GB - 2GB = 14GB)
const MEMORY_HEADROOM_GB: u32 = 2;
const MEMORY_PRESETS_GB: [u32; 10] = [2, 4, 6, 8, 12, 16, 24, 32, 48, 64];

pub fn total_ram_mb() -> u32 {
    static TOTAL_RAM_MB: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

    *TOTAL_RAM_MB.get_or_init(|| {
        let system = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        (system.total_memory() / 1024 / 1024).min(u32::MAX as u64) as u32
    })
}

pub fn memory_presets_mb() -> Vec<u32> {
    presets_for_total_gb((total_ram_mb() as f32 / 1024.).round() as u32)
}

fn presets_for_total_gb(total_gb: u32) -> Vec<u32> {
    let usable_gb = total_gb.saturating_sub(MEMORY_HEADROOM_GB);
    if usable_gb == 0 {
        return vec![1024];
    }

    let mut presets: Vec<u32> = MEMORY_PRESETS_GB
        .iter()
        .copied()
        .filter(|gb| *gb <= usable_gb)
        .map(|gb| gb * 1024)
        .collect();

    let usable_mb = usable_gb * 1024;
    if presets.last() != Some(&usable_mb) {
        presets.push(usable_mb);
    }

    presets
}

/// `8192` -> `8 GB` `1536` -> `1.5 GB`
pub fn format_memory_gb(mb: u32) -> String {
    if mb.is_multiple_of(1024) {
        format!("{} GB", mb / 1024)
    } else {
        format!("{:.1} GB", mb as f32 / 1024.)
    }
}

/// `7384` -> `2h 3m` `540` -> `9m` `0` -> `0m`
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

/// `3723` -> `1h 2m` `83` -> `1m 23s` `45` -> `45s`
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

/// `1500` -> `1.5K` `2_400_000` -> `2.4M`
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

/// 24-hour clock label `9` -> `09:00`
pub fn format_hour(hour: usize) -> String {
    format!("{hour:02}:00")
}

pub fn parse_day(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// `2026-07-05` -> `Jul 5`
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
        // Most recently played first
        (Some(a), Some(b)) => b.cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        // Never played latest version first (major then minor)
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
    static IS_WAYLAND: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_stop_two_gigabytes_short_of_the_machine() {
        assert_eq!(
            presets_for_total_gb(16),
            vec![2048, 4096, 6144, 8192, 12288, 14336]
        );
        assert_eq!(
            presets_for_total_gb(32),
            vec![2048, 4096, 6144, 8192, 12288, 16384, 24576, 30720]
        );
    }

    #[test]
    fn a_ceiling_landing_on_a_round_step_is_not_repeated() {
        assert_eq!(presets_for_total_gb(8), vec![2048, 4096, 6144]);
    }

    #[test]
    fn a_tiny_machine_still_gets_one_preset() {
        assert_eq!(presets_for_total_gb(4), vec![2048]);
        assert_eq!(presets_for_total_gb(2), vec![1024]);
    }

    #[test]
    fn memory_labels_drop_the_decimal_when_whole() {
        assert_eq!(format_memory_gb(8192), "8 GB");
        assert_eq!(format_memory_gb(1536), "1.5 GB");
    }
}
