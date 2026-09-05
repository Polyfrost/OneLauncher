use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::LauncherResult;
use crate::state::LauncherState;
use oneclient_common::domain::ContentType;
use oneclient_common::paths;
use oneclient_content::packages::store::{
    find_unreferenced_files, remove_unreferenced_files,
};

const LEGACY_TYPES: [ContentType; 4] = [
    ContentType::Mod,
    ContentType::ResourcePack,
    ContentType::Shader,
    ContentType::DataPack,
];

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
    pub unused_natives: ReclaimableEntry,
}

#[tracing::instrument(skip(state))]
pub async fn storage_report(state: &LauncherState) -> LauncherResult<StorageReport> {
    #[cfg(debug_assertions)]
    if let Some(report) = fixture_report() {
        return Ok(report);
    }

    let launcher = paths::launcher_dir()?;

    let mut categories = vec![
        entry("Packages", paths::packages_cache_dir()?).await,
        entry("Minecraft versions", paths::versions_dir()?).await,
        entry("Libraries", paths::libraries_dir()?).await,
        entry("Assets", paths::assets_dir()?).await,
        entry("Natives", paths::natives_dir()?).await,
        entry("Java runtimes", paths::java_dir()?).await,
        entry("Shared game directory", paths::shared_minecraft_dir()?).await,
        entry("Logs", paths::logs_dir()?).await,
        entry("Image cache", paths::images_cache_dir()?).await,
        entry("Bundles", paths::bundles_dir()?).await,
    ];
    categories.sort_by_key(|entry| std::cmp::Reverse(entry.bytes));

    let mut clusters = Vec::new();
    for cluster in state.clusters.list().await? {
        let Ok(dir) = cluster.dir() else {
            continue;
        };
        clusters.push(entry(&cluster.name, dir).await);
    }
    clusters.sort_by_key(|entry| std::cmp::Reverse(entry.bytes));

    let unreferenced = find_unreferenced_files(&state.services.content()).await?;
    let unreferenced_cache = ReclaimableEntry {
        bytes: unreferenced.iter().map(|(_, size)| size).sum(),
        files: unreferenced.len(),
    };

    Ok(StorageReport {
        total_bytes: dir_size(launcher).await,
        categories,
        clusters,
        unreferenced_cache,
        legacy_cluster_content: legacy_cluster_content(state).await?,
        unused_natives: unused_natives(state).await,
    })
}

async fn natives_in_use(state: &LauncherState) -> Option<HashSet<String>> {
    let clusters = state.clusters.list().await.ok()?;
    let metadata = state.metadata.lock().await;

    let mut in_use = HashSet::new();
    for cluster in clusters {
        let mc_version = oneclient_common::version::normalize_mc_version_input(&cluster.mc_version);

        if cluster.mc_loader == oneclient_common::domain::GameLoader::Vanilla {
            in_use.insert(mc_version);
            continue;
        }

        let resolved = oneclient_mc::get_loader_version_cached(
            &metadata,
            &mc_version,
            cluster.mc_loader,
            cluster.mc_loader_version.as_deref(),
        );

        match resolved {
            Ok(Some(loader)) => {
                in_use.insert(format!("{mc_version}-{}", loader.id));
            }
            Ok(None) | Err(_) => {
                tracing::info!(
                    cluster = %cluster.name,
                    loader = %cluster.mc_loader,
                    "cannot resolve a loader version from the cached manifest; \
                     skipping the natives sweep"
                );
                return None;
            }
        }
    }

    Some(in_use)
}

async fn unused_natives_dirs(state: &LauncherState) -> Vec<PathBuf> {
    if !state.games.active_ids().is_empty() {
        return Vec::new();
    }

    let Ok(root) = paths::natives_dir() else {
        return Vec::new();
    };
    let Some(in_use) = natives_in_use(state).await else {
        return Vec::new();
    };

    let mut unused = Vec::new();
    let Ok(mut entries) = polyio::read_dir(&root).await else {
        return unused;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !in_use.contains(name) {
            unused.push(path);
        }
    }

    unused
}

async fn unused_natives(state: &LauncherState) -> ReclaimableEntry {
    let dirs = unused_natives_dirs(state).await;

    let mut found = ReclaimableEntry::default();
    for dir in &dirs {
        found.files += 1;
        found.bytes += dir_size(dir).await;
    }
    found
}

/// Content is materialized from the cache now so anything in a cluster's own
/// folder is an inert leftover from an older launcher space not correctness
async fn legacy_cluster_content(state: &LauncherState) -> LauncherResult<ReclaimableEntry> {
    let mut found = ReclaimableEntry::default();

    for cluster in state.clusters.list().await? {
        // A dedicated cluster's folder *is* its game directory so content there belongs
        if cluster.uses_dedicated_dir() {
            continue;
        }
        let Ok(root) = cluster.dir() else {
            continue;
        };

        for content_type in LEGACY_TYPES {
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

    Ok(found)
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

async fn entry(label: &str, path: PathBuf) -> StorageEntry {
    StorageEntry {
        label: label.to_string(),
        bytes: dir_size(&path).await,
        path,
        files: None,
    }
}

/// Links are not followed so a materialized game directory does not appear to
/// double the size of the package cache
pub async fn dir_size(root: impl AsRef<Path>) -> u64 {
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
            } else if let Ok(meta) = entry.metadata().await {
                total += meta.len();
            }
        }
    }

    total
}

/// Cleanup refuses to run while fixture numbers are shown the report bears no
/// relation to disk so the only thing it could delete is the user's real data
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

pub async fn clean_unused_natives(state: &LauncherState) -> LauncherResult<usize> {
    if showing_fixture() {
        tracing::info!("fixture storage report is active; skipping natives cleanup");
        return Ok(0);
    }

    let dirs = unused_natives_dirs(state).await;
    let mut removed = 0;
    for dir in dirs {
        match polyio::remove_dir_all(&dir).await {
            Ok(()) => {
                tracing::info!(path = %dir.display(), "removed unused natives");
                removed += 1;
            }
            Err(err) => tracing::warn!(path = %dir.display(), "could not remove natives: {err}"),
        }
    }

    Ok(removed)
}

pub async fn clean_legacy_cluster_content(state: &LauncherState) -> LauncherResult<usize> {
    if showing_fixture() {
        tracing::info!("fixture storage report is active; skipping leftover cleanup");
        return Ok(0);
    }

    let report = crate::clusters::unlink_legacy_cluster_content(state).await?;
    Ok(report.removed)
}

pub async fn clean_cluster_leftovers(state: &LauncherState) -> LauncherResult<usize> {
    let content = clean_legacy_cluster_content(state).await?;
    let natives = clean_unused_natives(state).await?;
    Ok(content + natives)
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
            unused_natives: ReclaimableEntry::default(),
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
            unused_natives: ReclaimableEntry::default(),
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
            unused_natives: ReclaimableEntry {
                bytes: 4_200_000,
                files: 3,
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

        polyio::write(dir.join("a.bin"), vec![0u8; 1000]).await.unwrap();
        polyio::write(dir.join("nested").join("b.bin"), vec![0u8; 500])
            .await
            .unwrap();

        assert_eq!(dir_size(dir).await, 1500);

        polyio::symlink_file(dir.join("a.bin"), dir.join("link.bin"))
            .await
            .unwrap();
        assert_eq!(
            dir_size(dir).await,
            1500,
            "a link is not another copy of its target"
        );

        std::fs::remove_dir_all(root.path()).ok();
    }
}
