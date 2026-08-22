use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::LauncherResult;
use crate::clusters::Cluster;
use crate::state::LauncherState;
use oneclient_common::domain::ContentType;
use oneclient_common::paths;
use oneclient_content::packages::store::manifest;
use oneclient_content::packages::store::{
    find_unreferenced_files, remove_unreferenced_files,
};
use oneclient_events::EventBus;

const LEGACY_TYPES: [ContentType; 2] = [ContentType::Mod, ContentType::DataPack];

// stable id for the storage scan's progress
pub const STORAGE_SCAN_PROGRESS: Uuid =
    Uuid::from_u128(0x5354_4F52_4147_4500_0000_0000_0000_0001);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageEntry {
    pub label: String,
    pub bytes: u64,
    pub path: PathBuf,
    pub files: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReclaimableEntry {
    pub bytes: u64,
    pub files: usize,
}

impl ReclaimableEntry {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageReport {
    pub total_bytes: u64,
    pub categories: Vec<StorageEntry>,
    pub clusters: Vec<StorageEntry>,
    pub unreferenced_cache: ReclaimableEntry,
    pub legacy_cluster_content: ReclaimableEntry,
}

#[tracing::instrument(skip(state))]
pub async fn storage_report(state: &LauncherState) -> LauncherResult<StorageReport> {
    #[cfg(debug_assertions)]
    if let Some(report) = fixture_report() {
        return Ok(report);
    }

    let launcher = paths::launcher_dir()?;

    let category_dirs = [
        ("Packages", paths::packages_cache_dir()?),
        ("Minecraft versions", paths::versions_dir()?),
        ("Libraries", paths::libraries_dir()?),
        ("Assets", paths::assets_dir()?),
        ("Natives", paths::natives_dir()?),
        ("Java runtimes", paths::java_dir()?),
        ("Shared game directory", paths::shared_minecraft_dir()?),
        ("Logs", paths::logs_dir()?),
        ("Image cache", paths::images_cache_dir()?),
        ("Bundles", paths::bundles_dir()?),
    ];

    let cluster_list = state.clusters.list().await?;

    let mut steps = ScanSteps {
        events: &state.services.events,
        done: 0,
        total: (category_dirs.len() + cluster_list.len() + 3) as u64,
    };

    let mut seen = SeenFiles::default();

    let mut categories = Vec::new();
    for (label, path) in category_dirs {
        steps.begin(label);
        categories.push(entry(label, path, &mut seen).await);
    }
    categories.sort_by_key(|entry| std::cmp::Reverse(entry.bytes));

    let mut clusters = Vec::new();
    for cluster in &cluster_list {
        steps.begin(&cluster.name);
        let Ok(dir) = cluster.dir() else {
            continue;
        };
        clusters.push(entry(&cluster.name, dir, &mut seen).await);
    }
    clusters.sort_by_key(|entry| std::cmp::Reverse(entry.bytes));

    steps.begin("unreferenced cache files");
    let unreferenced = find_unreferenced_files(&state.services.content()).await?;
    let unreferenced_cache = ReclaimableEntry {
        bytes: unreferenced.iter().map(|(_, size)| size).sum(),
        files: unreferenced.len(),
    };

    steps.begin("leftover cluster content");
    let legacy = legacy_cluster_content(&cluster_list).await;

    steps.begin("total size");
    let total_bytes = dir_size(launcher).await;

    Ok(StorageReport {
        total_bytes,
        categories,
        clusters,
        unreferenced_cache,
        legacy_cluster_content: legacy,
    })
}

struct ScanSteps<'a> {
    events: &'a EventBus,
    done: u64,
    total: u64,
}

impl ScanSteps<'_> {
    fn begin(&mut self, label: &str) {
        self.events.progress(
            STORAGE_SCAN_PROGRESS,
            format!("Measuring {label}"),
            self.done,
            self.total,
        );
        self.done += 1;
    }
}

impl Drop for ScanSteps<'_> {
    fn drop(&mut self) {
        self.events
            .progress(STORAGE_SCAN_PROGRESS, "Done", self.total, self.total);
    }
}

async fn legacy_cluster_content(clusters: &[Cluster]) -> ReclaimableEntry {
    let mut found = ReclaimableEntry::default();

    for cluster in clusters {
        if cluster.uses_dedicated_dir() {
            continue;
        }
        let Ok(root) = cluster.dir() else {
            continue;
        };

        let mods_live_here = manifest::mods_live_in_cluster(&root).await;

        for content_type in LEGACY_TYPES {
            if content_type == ContentType::Mod && mods_live_here {
                continue;
            }

            let dir = root.join(content_type.folder_name());
            let Ok(mut entries) = polyio::read_dir(&dir).await else {
                continue;
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let Ok(file_type) = entry.file_type().await else {
                    continue;
                };
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };

                if file_type.is_symlink() {
                    found.files += 1;
                    continue;
                }

                if file_type.is_file() && is_content_file(content_type, name) {
                    found.files += 1;
                    found.bytes += polyio::stat(&path).await.map(|m| m.len()).unwrap_or(0);
                }
            }
        }
    }

    found
}

fn is_content_file(content_type: ContentType, name: &str) -> bool {
    let lower = name.to_lowercase();
    let lower = lower.trim_end_matches(".disabled");

    match content_type {
        ContentType::Mod => lower.ends_with(".jar"),
        ContentType::ResourcePack | ContentType::Shader | ContentType::DataPack => {
            lower.ends_with(".zip")
        }
        _ => false,
    }
}

async fn entry(label: &str, path: PathBuf, seen: &mut SeenFiles) -> StorageEntry {
    StorageEntry {
        label: label.to_string(),
        bytes: dir_size_seen(&path, seen).await,
        path,
        files: None,
    }
}

pub async fn dir_size(root: impl AsRef<Path>) -> u64 {
    dir_size_seen(root, &mut SeenFiles::default()).await
}

async fn dir_size_seen(root: impl AsRef<Path>, seen: &mut SeenFiles) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.as_ref().to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(mut entries) = polyio::read_dir(&dir).await else {
            continue;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };

            if file_type.is_symlink() {
                continue;
            }

            if file_type.is_dir() {
                stack.push(entry.path());
            } else if let Ok(meta) = entry.metadata().await
                && seen.first_sighting(&entry.path()).await
            {
                total += meta.len();
            }
        }
    }

    total
}

#[derive(Default)]
struct SeenFiles(HashSet<(u64, u64)>);

impl SeenFiles {
    async fn first_sighting(&mut self, path: &Path) -> bool {
        match file_identity(path).await {
            Some(id) => self.0.insert(id),
            None => true,
        }
    }
}

async fn file_identity(path: &Path) -> Option<(u64, u64)> {
    if !can_be_materialized(path) {
        return None;
    }

    polyio::file_id(path).await.ok()
}

fn can_be_materialized(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    let lower = name.to_lowercase();
    let lower = lower.trim_end_matches(".disabled");

    lower.ends_with(".jar") || lower.ends_with(".zip")
}

fn showing_fixture() -> bool {
    #[cfg(debug_assertions)]
    {
        std::env::var("ONECLIENT_FAKE_STORAGE").is_ok()
    }

    #[cfg(not(debug_assertions))]
    false
}

pub async fn clean_unreferenced_cache(state: &LauncherState) -> LauncherResult<u64> {
    if showing_fixture() {
        tracing::info!("fixture storage report is active; skipping cache cleanup");
        return Ok(0);
    }

    let report = remove_unreferenced_files(&state.services.content()).await?;
    Ok(report.bytes_freed)
}

pub async fn clean_legacy_cluster_content(state: &LauncherState) -> LauncherResult<usize> {
    if showing_fixture() {
        tracing::info!("fixture storage report is active; skipping leftover cleanup");
        return Ok(0);
    }

    let report = crate::clusters::unlink_legacy_cluster_content(state).await?;
    Ok(report.removed)
}

/// Set `ONECLIENT_FAKE_STORAGE` to `empty` `clean` or `full` to return a fixture
/// instead of scanning disk
/// Read on every refresh so no rebuild is needed
#[cfg(debug_assertions)]
fn fixture_report() -> Option<StorageReport> {
    let scenario = std::env::var("ONECLIENT_FAKE_STORAGE").ok()?;

    let fake = |label: &str, bytes: u64| StorageEntry {
        label: label.to_string(),
        bytes,
        path: PathBuf::from("/tmp/oneclient-fake").join(label.to_lowercase().replace(' ', "-")),
        files: None,
    };

    let report = match scenario.trim().to_lowercase().as_str() {
        "empty" => StorageReport {
            total_bytes: 0,
            categories: Vec::new(),
            clusters: Vec::new(),
            unreferenced_cache: ReclaimableEntry::default(),
            legacy_cluster_content: ReclaimableEntry::default(),
        },
        "clean" => StorageReport {
            total_bytes: 1_284_000_000,
            categories: vec![
                fake("Minecraft versions", 512_000_000),
                fake("Packages", 368_000_000),
                fake("Libraries", 240_000_000),
                fake("Assets", 96_000_000),
                fake("Java runtimes", 61_000_000),
                fake("Shared game directory", 5_400_000),
                fake("Logs", 1_200_000),
                fake("Image cache", 400_000),
            ],
            clusters: vec![
                fake("26.1.2 Fabric", 42_000_000),
                fake("26.2 Fabric", 18_500_000),
                fake("1.21.11 Fabric", 9_100_000),
            ],
            unreferenced_cache: ReclaimableEntry::default(),
            legacy_cluster_content: ReclaimableEntry::default(),
        },
        "full" => StorageReport {
            total_bytes: 4_930_000_000,
            categories: vec![
                fake("Packages", 2_100_000_000),
                fake("Minecraft versions", 1_400_000_000),
                fake("Libraries", 890_000_000),
                fake("Assets", 412_000_000),
                fake("Java runtimes", 118_000_000),
                fake("Natives", 6_800_000),
                fake("Shared game directory", 2_100_000),
                fake("Logs", 940_000),
                fake("Bundles", 120_000),
            ],
            clusters: vec![
                fake("26.1.2 Fabric", 310_000_000),
                fake("26.2 Fabric", 148_000_000),
                fake("1.21.11 Fabric", 96_000_000),
                fake("1.21.10 Fabric", 12_000_000),
                fake("1.21.1 Fabric", 0),
            ],
            unreferenced_cache: ReclaimableEntry {
                bytes: 743_000_000,
                files: 214,
            },
            legacy_cluster_content: ReclaimableEntry {
                bytes: 261_000_000,
                files: 439,
            },
        },
        other => {
            tracing::warn!(
                scenario = other,
                "unknown ONECLIENT_FAKE_STORAGE value; expected empty, clean or full"
            );
            return None;
        }
    };

    tracing::info!(scenario = %scenario, "serving fixture storage report");
    Some(report)
}

#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];

    if bytes < 1000 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;

    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }

    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_read_the_way_a_person_would_say_them() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(1_500), "1.5 KB");
        assert_eq!(format_bytes(628_000_000), "628 MB");
        assert_eq!(format_bytes(2_400_000_000), "2.4 GB");
    }

    #[test]
    fn disabled_content_still_counts_as_content() {
        assert!(is_content_file(ContentType::Mod, "sodium.jar"));
        assert!(is_content_file(ContentType::Mod, "sodium.jar.disabled"));
        assert!(is_content_file(ContentType::Shader, "BSL.zip"));
        assert!(!is_content_file(ContentType::Shader, "BSL.zip.txt"));
        assert!(!is_content_file(ContentType::Mod, "options.txt"));
    }

    #[tokio::test]
    async fn size_ignores_what_links_point_at() {
        let root = polyio::testing::ScratchDir::new("dir_size");
        let dir = root.path();
        polyio::create_dir_all(dir.join("nested")).await.unwrap();

        polyio::write(dir.join("a.jar"), vec![0u8; 1000]).await.unwrap();
        polyio::write(dir.join("nested").join("b.jar"), vec![0u8; 500])
            .await
            .unwrap();

        assert_eq!(dir_size(dir).await, 1500);

        polyio::symlink_file(dir.join("a.jar"), dir.join("link.jar"))
            .await
            .unwrap();
        assert_eq!(
            dir_size(dir).await,
            1500,
            "a link is not another copy of its target"
        );

        std::fs::remove_dir_all(root.path()).ok();
    }

    #[tokio::test]
    async fn a_file_in_two_folders_is_paid_for_once() {
        let root = polyio::testing::ScratchDir::new("shared_size");
        let dir = root.path();
        let store = dir.join("store");
        let cluster = dir.join("cluster");
        polyio::create_dir_all(&store).await.unwrap();
        polyio::create_dir_all(&cluster).await.unwrap();

        polyio::write(store.join("mod.jar"), vec![0u8; 1000]).await.unwrap();
        polyio::symlink_file(store.join("mod.jar"), cluster.join("mod.jar"))
            .await
            .unwrap();

        let mut seen = SeenFiles::default();
        assert_eq!(dir_size_seen(&store, &mut seen).await, 1000);
        assert_eq!(
            dir_size_seen(&cluster, &mut seen).await,
            0,
            "the cache already paid for it"
        );

        std::fs::remove_dir_all(root.path()).ok();
    }
}
