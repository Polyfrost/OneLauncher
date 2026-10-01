use std::cmp::Ordering;
use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use oneclient_common::domain::GameLoader;
use oneclient_common::{ParsedMcVersion, VersionKey, format_mc_version, parse_mc_version};
use oneclient_core::BundleArchive;
use oneclient_core::clusters::Cluster;

use oneclient_common::MEMORY_HEADROOM_GB;
pub use oneclient_common::total_ram_mb;

pub type ClusterGroups = BTreeMap<ReleaseLine, Vec<Cluster>>;

/// The modern scheme puts a full release in the first two components (`26.1`) so each
/// minor is its own line. `1.21.x` is split the same way (`1.21.5`) since it spans many
/// content drops; other legacy `1.x` versions keep the whole major (`1.20`) as one line
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReleaseLine {
    pub major: u32,
    /// `Some` for the modern scheme and `1.21.x`, where the minor is part of the line
    pub minor: Option<u32>,
}

impl ReleaseLine {
    fn from_parsed(parsed: &ParsedMcVersion) -> Option<Self> {
        let minor = if parsed.major >= 26 {
            Some(parsed.minor?)
        } else if parsed.major == 21 {
            parsed.minor
        } else {
            None
        };
        Some(Self {
            major: parsed.major,
            minor,
        })
    }

    pub fn from_version(version: &str) -> Option<Self> {
        Self::from_parsed(&parse_mc_version(version)?)
    }

    pub fn for_cluster(cluster: &Cluster) -> Option<Self> {
        Self::from_version(&cluster.mc_version)
    }

    /// The generic name of the line `26.1` `1.21.5` `1.20`
    pub fn pretty_name(&self) -> String {
        match self.minor {
            Some(minor) => format_mc_version(self.major, minor, None),
            None => format!("1.{}", self.major),
        }
    }

    fn art_key(&self) -> Option<VersionKey> {
        self.minor.map(|minor| (minor, None))
    }
}

fn cluster_key(cluster: &Cluster) -> Option<VersionKey> {
    parse_mc_version(&cluster.mc_version).and_then(|parsed| parsed.key())
}

fn sole_version_key(clusters: &[Cluster]) -> Option<VersionKey> {
    match version_keys(clusters).as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

/// A line with a single version is shown as that full version (`26.1.2`) else `1.21`
pub fn line_title(line: ReleaseLine, clusters: &[Cluster]) -> String {
    match sole_version_key(clusters) {
        Some(key) => version_label(line.major, key),
        None => line.pretty_name(),
    }
}

pub fn line_art_key(line: ReleaseLine, clusters: &[Cluster]) -> Option<VersionKey> {
    sole_version_key(clusters).or_else(|| line.art_key())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GridSelection {
    Line(ReleaseLine),
    Instance(i64),
}

pub fn split_clusters(clusters: &[Cluster]) -> (ClusterGroups, Vec<Cluster>) {
    let (instances, oneclient): (Vec<&Cluster>, Vec<&Cluster>) =
        clusters.iter().partition(|cluster| cluster.is_isolated());

    (
        group_clusters_by_release(oneclient),
        sort_clusters_for_home(instances.into_iter().cloned().collect()),
    )
}

pub fn group_clusters_by_release<'a>(
    clusters: impl IntoIterator<Item = &'a Cluster>,
) -> ClusterGroups {
    let mut groups: ClusterGroups = BTreeMap::new();

    for cluster in clusters {
        let Some(line) = ReleaseLine::for_cluster(cluster) else {
            continue;
        };
        groups.entry(line).or_default().push(cluster.clone());
    }

    for list in groups.values_mut() {
        list.sort_by_key(|b| std::cmp::Reverse(cluster_key(b)));
    }

    groups
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

pub fn loaders_for_line(clusters: &[Cluster]) -> Vec<GameLoader> {
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

/// The line the page opens on the active cluster's line else the newest
pub fn default_line(groups: &ClusterGroups, active: Option<Cluster>) -> Option<ReleaseLine> {
    if let Some(cluster) = active
        && let Some(line) = ReleaseLine::for_cluster(&cluster)
    {
        return Some(line);
    }
    groups.keys().next_back().copied()
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
    let loaders = loaders_for_line(clusters);
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
const MEMORY_PRESETS_GB: [u32; 11] = [2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64];

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

/// `45` -> `45s` `540` -> `9m` `7200` -> `2h` `7384` -> `2h 3m` `180000` -> `2d 2h`
pub fn format_duration(secs: i64) -> String {
    if secs <= 0 {
        return "0m".to_string();
    }

    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3600;
    let minutes = (secs % 3600) / 60;

    match (days, hours, minutes) {
        (0, 0, 0) => format!("{secs}s"),
        (0, 0, m) => format!("{m}m"),
        (0, h, 0) => format!("{h}h"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, 0, _) => format!("{d}d"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

/// `3723` -> `1h 2m` `83` -> `1m 23s` `45` -> `45s`
pub fn format_durations(secs: i64) -> String {
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

pub fn bundle_display_name(archive: &BundleArchive) -> String {
    let category = archive.manifest.category.trim();
    if category.is_empty() {
        archive.manifest.name.clone()
    } else {
        category.to_string()
    }
}

pub fn default_cluster(clusters: Vec<Cluster>, active: Option<i64>) -> Option<Cluster> {
    home_cluster(&clusters, active).cloned()
}

pub fn home_cluster(clusters: &[Cluster], active: Option<i64>) -> Option<&Cluster> {
    active
        .and_then(|id| clusters.iter().find(|cluster| cluster.id == id))
        // `min_by` keeps the first of equals, matching the stable sort in `sort_clusters_for_home`
        .or_else(|| clusters.iter().min_by(|a, b| compare_recent_activity(a, b)))
}

pub fn sort_clusters_for_home(mut clusters: Vec<Cluster>) -> Vec<Cluster> {
    clusters.sort_by(compare_recent_activity);
    clusters
}

fn recent_activity(cluster: &Cluster) -> Option<chrono::DateTime<chrono::Utc>> {
    let created = cluster.created_at.filter(|_| cluster.user_created);
    cluster.last_played.max(created)
}

fn compare_recent_activity(a: &Cluster, b: &Cluster) -> Ordering {
    match (recent_activity(a), recent_activity(b)) {
        // Most recently played first
        (Some(a), Some(b)) => b.cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        // Never played latest version first (major then minor)
        (None, None) => version_sort_key(&b.mc_version).cmp(&version_sort_key(&a.mc_version)),
    }
}

pub fn version_sort_key(version: &str) -> (u32, u32, u32) {
    parse_mc_version(version)
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
    use crate::view::onboarding::test_support::cluster;

    fn versioned(id: i64, mc_version: &str) -> Cluster {
        Cluster {
            mc_version: mc_version.to_string(),
            ..cluster(id)
        }
    }

    #[test]
    fn user_created_oneclient_clusters_join_their_release_line() {
        let provisioned = versioned(1, "26.1");
        let mine = Cluster {
            user_created: true,
            ..versioned(2, "26.1")
        };
        let vanilla = Cluster {
            user_created: true,
            kind: oneclient_db::models::ClusterKind::Vanilla,
            ..versioned(3, "26.1")
        };

        let (groups, instances) = split_clusters(&[provisioned, mine, vanilla]);
        let line = ReleaseLine::from_version("26.1").unwrap();
        let grouped: Vec<i64> = groups[&line].iter().map(|c| c.id).collect();
        assert_eq!(grouped, vec![1, 2]);
        assert_eq!(instances.iter().map(|c| c.id).collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn a_newly_created_instance_comes_first_on_home() {
        let now = chrono::Utc::now();
        let played = Cluster {
            last_played: Some(now - chrono::Duration::hours(2)),
            ..versioned(1, "1.21.1")
        };
        let created = Cluster {
            user_created: true,
            created_at: Some(now),
            ..versioned(2, "1.20.1")
        };
        let provisioned = Cluster {
            user_created: false,
            created_at: Some(now),
            ..versioned(3, "26.3")
        };

        let order: Vec<i64> = sort_clusters_for_home(vec![played, provisioned, created])
            .into_iter()
            .map(|cluster| cluster.id)
            .collect();
        assert_eq!(order, vec![2, 1, 3]);
    }

    #[test]
    fn playing_an_older_instance_moves_it_back_in_front() {
        let now = chrono::Utc::now();
        let created = Cluster {
            user_created: true,
            created_at: Some(now - chrono::Duration::hours(1)),
            ..versioned(1, "1.20.1")
        };
        let played = Cluster {
            last_played: Some(now),
            ..versioned(2, "1.21.1")
        };

        let order: Vec<i64> = sort_clusters_for_home(vec![created, played])
            .into_iter()
            .map(|cluster| cluster.id)
            .collect();
        assert_eq!(order, vec![2, 1]);
    }

    #[test]
    fn home_background_matches_the_first_recents_card() {
        let clusters = vec![
            versioned(1, "1.8.9"),
            versioned(2, "26.2"),
            versioned(3, "1.21.1"),
        ];

        let first_card = sort_clusters_for_home(clusters.clone())[0].id;
        assert_eq!(
            home_cluster(&clusters, None).map(|c| c.id),
            Some(first_card)
        );
        assert_eq!(home_cluster(&clusters, Some(3)).map(|c| c.id), Some(3));
    }

    fn line(major: u32, minor: Option<u32>) -> ReleaseLine {
        ReleaseLine { major, minor }
    }

    fn lines_of(clusters: &[Cluster]) -> Vec<ReleaseLine> {
        group_clusters_by_release(clusters)
            .keys()
            .copied()
            .collect()
    }

    #[test]
    fn modern_minors_are_separate_lines() {
        let clusters = [
            versioned(1, "26.1"),
            versioned(2, "26.2"),
            versioned(3, "26.2.1"),
        ];

        assert_eq!(
            lines_of(&clusters),
            vec![line(26, Some(1)), line(26, Some(2))]
        );
    }

    #[test]
    fn one_twenty_one_minors_are_separate_lines() {
        let clusters = [versioned(1, "1.21.5"), versioned(2, "1.21.11")];

        assert_eq!(
            lines_of(&clusters),
            vec![line(21, Some(5)), line(21, Some(11))]
        );
        assert_eq!(line_title(line(21, Some(5)), &clusters[..1]), "1.21.5");
    }

    #[test]
    fn other_legacy_minors_share_one_line() {
        let clusters = [versioned(1, "1.20.1"), versioned(2, "1.20.4")];

        assert_eq!(lines_of(&clusters), vec![line(20, None)]);
        assert_eq!(line_title(line(20, None), &clusters), "1.20");
        assert_eq!(line_art_key(line(20, None), &clusters), None);
    }

    #[test]
    fn a_lone_version_is_shown_in_full() {
        let clusters = [versioned(1, "26.1.2")];
        let line = line(26, Some(1));

        assert_eq!(line_title(line, &clusters), "26.1.2");
        assert_eq!(line_art_key(line, &clusters), Some((1, Some(2))));
    }

    #[test]
    fn several_loaders_of_one_version_still_count_as_one() {
        let clusters = [
            versioned(1, "26.1.2"),
            Cluster {
                mc_loader: GameLoader::Vanilla,
                ..versioned(2, "26.1.2")
            },
        ];

        assert_eq!(line_title(line(26, Some(1)), &clusters), "26.1.2");
    }

    #[test]
    fn several_versions_fall_back_to_the_generic_name() {
        let modern = [versioned(4, "26.1"), versioned(5, "26.1.2")];
        assert_eq!(line_title(line(26, Some(1)), &modern), "26.1");
        assert_eq!(line_art_key(line(26, Some(1)), &modern), Some((1, None)));
    }

    #[test]
    fn presets_stop_two_gigabytes_short_of_the_machine() {
        assert_eq!(
            presets_for_total_gb(16),
            vec![2048, 3072, 4096, 6144, 8192, 12288, 14336]
        );
        assert_eq!(
            presets_for_total_gb(32),
            vec![2048, 3072, 4096, 6144, 8192, 12288, 16384, 24576, 30720]
        );
    }

    #[test]
    fn a_ceiling_landing_on_a_round_step_is_not_repeated() {
        assert_eq!(presets_for_total_gb(8), vec![2048, 3072, 4096, 6144]);
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

    #[test]
    fn durations_never_show_a_zero_unit() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(540), "9m");
        assert_eq!(format_duration(7200), "2h");
        assert_eq!(format_duration(7384), "2h 3m");
        assert_eq!(format_duration(180_000), "2d 2h");
        assert_eq!(format_duration(172_800), "2d");
        assert_eq!(format_duration(0), "0m");
    }

    #[test]
    fn a_bare_legacy_version_keeps_its_line_name() {
        let clusters = [versioned(1, "1.21")];

        assert_eq!(line_title(line(21, None), &clusters), "1.21");
    }
}
