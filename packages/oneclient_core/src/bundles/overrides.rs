use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oneclient_db::models::ClusterRow;
use serde::{Deserialize, Serialize};

use crate::LauncherResult;
use crate::crypto::{sha1_bytes, sha1_file};
use crate::notification::NotificationService;

const ALWAYS_UPDATE_GLOBS: &[&str] = &["config/fabric_loader_dependencies.json"];

const OVERRIDES_PREFIX: &str = "overrides/";
const LOCK_REL_PATH: &str = ".oneclient/bundle_overrides.json";

fn cluster_root(cluster: &ClusterRow) -> LauncherResult<PathBuf> {
    Ok(crate::paths::clusters_dir()?.join(&cluster.folder_name))
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct OverrideLock {
    #[serde(default)]
    bundles: HashMap<String, HashMap<String, String>>,
}

impl OverrideLock {
    async fn load(root: &Path) -> Self {
        let path = root.join(LOCK_REL_PATH);
        match polyio::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                tracing::warn!(error = %err, "corrupt bundle override lock; starting fresh");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    async fn save(&self, root: &Path) -> LauncherResult<()> {
        let path = root.join(LOCK_REL_PATH);
        if let Some(parent) = path.parent() {
            polyio::create_dir_all(parent).await.ok();
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        polyio::write(&path, bytes).await?;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct OverrideSyncReport {
    pub written: Vec<String>,
    pub conflicts: Vec<String>,
    pub deleted: Vec<String>,
}

#[tracing::instrument(level = "debug", skip(cluster, notifier))]
pub async fn sync_bundle_overrides(
    archive_path: &Path,
    bundle_name: &str,
    cluster: &ClusterRow,
    notifier: Option<&NotificationService>,
) -> LauncherResult<OverrideSyncReport> {
    let root = cluster_root(cluster)?;
    sync_bundle_overrides_at(archive_path, bundle_name, &root, notifier).await
}

#[tracing::instrument(level = "debug", skip(notifier))]
async fn sync_bundle_overrides_at(
    archive_path: &Path,
    bundle_name: &str,
    root: &Path,
    notifier: Option<&NotificationService>,
) -> LauncherResult<OverrideSyncReport> {
    let entries =
        polyio::read_zip_file_entries(archive_path, |name| name.starts_with(OVERRIDES_PREFIX))
            .await?;

    let mut lock = OverrideLock::load(root).await;
    let previous = lock.bundles.remove(bundle_name).unwrap_or_default();
    let mut next: HashMap<String, String> = HashMap::new();
    let mut report = OverrideSyncReport::default();

    for (name, bytes) in entries {
        let rel = name.trim_start_matches(OVERRIDES_PREFIX);
        if rel.is_empty() {
            continue;
        }

        let new_sha1 = sha1_bytes(&bytes);
        let dest = root.join(polyio::sanitize_path(rel));
        let disk_sha1 = current_sha1(&dest).await;
        let base = previous.get(rel).cloned();

        if matches_always_update(rel) {
            let wrote = write_override(&dest, &bytes, rel).await;
            if wrote {
                if disk_sha1.as_deref() != Some(new_sha1.as_str()) {
                    report.written.push(rel.to_string());
                }
                next.insert(rel.to_string(), new_sha1);
            } else if let Some(b) = base {
                next.insert(rel.to_string(), b);
            }
            continue;
        }

        match disk_sha1 {
            // Nothing on disk -> copy from bundle
            None => {
                if write_override(&dest, &bytes, rel).await {
                    report.written.push(rel.to_string());
                    next.insert(rel.to_string(), new_sha1);
                }
            }
            // Already matches the bundle -> so just record it in the lock
            Some(disk) if disk == new_sha1 => {
                next.insert(rel.to_string(), new_sha1);
            }
            // Untouched -< safe to update
            Some(disk) if base.as_deref() == Some(disk.as_str()) => {
                if write_override(&dest, &bytes, rel).await {
                    report.written.push(rel.to_string());
                    next.insert(rel.to_string(), new_sha1);
                } else {
                    next.insert(rel.to_string(), disk);
                }
            }
            // Differs from both the bundle and the base -> conflict
            Some(_) => {
                if let Some(b) = base {
                    if b != new_sha1 {
                        report.conflicts.push(rel.to_string());
                    }
                    next.insert(rel.to_string(), b);
                }
                // untracked and untouched
            }
        }
    }

    // delete files the bundle no longer ships, but only if they were unmodified locally
    for (rel, base_sha1) in &previous {
        if next.contains_key(rel) {
            continue;
        }
        let dest = root.join(polyio::sanitize_path(rel));

        if current_sha1(&dest).await.as_deref() == Some(base_sha1.as_str())
            && polyio::remove_file(&dest).await.is_ok()
        {
            report.deleted.push(rel.clone());
        }
    }

    lock.bundles.insert(bundle_name.to_string(), next);
    if let Err(err) = lock.save(root).await {
        tracing::warn!(error = %err, "failed to persist bundle override lock");
    }

    tracing::debug!(
        bundle = bundle_name,
        written = report.written.len(),
        conflicts = report.conflicts.len(),
        deleted = report.deleted.len(),
        "synced bundle overrides"
    );

    if !report.conflicts.is_empty() {
        notify_conflicts(notifier, bundle_name, &report.conflicts);
    }

    Ok(report)
}

async fn write_override(dest: &Path, bytes: &[u8], rel: &str) -> bool {
    if let Some(parent) = dest.parent() {
        polyio::create_dir_all(parent).await.ok();
    }
    match polyio::write(dest, bytes).await {
        Ok(()) => true,
        Err(err) => {
            tracing::warn!(path = rel, error = %err, "failed to write bundle override");
            false
        }
    }
}

async fn current_sha1(path: &Path) -> Option<String> {
    if polyio::try_exists(path).await.unwrap_or(false) {
        sha1_file(path).await.ok()
    } else {
        None
    }
}

fn notify_conflicts(
    notifier: Option<&NotificationService>,
    bundle_name: &str,
    conflicts: &[String],
) {
    let Some(notifier) = notifier else {
        return;
    };

    const MAX_LISTED: usize = 5;
    let listed = conflicts
        .iter()
        .take(MAX_LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let extra = conflicts.len().saturating_sub(MAX_LISTED);
    let suffix = if extra > 0 {
        format!(" (+{extra} more)")
    } else {
        String::new()
    };

    notifier.send_info(
        "Kept your config edits",
        &format!(
            "{bundle_name}: {} config file(s) you edited were left untouched by the update: {listed}{suffix}",
            conflicts.len()
        ),
    );
}

fn matches_always_update(rel: &str) -> bool {
    ALWAYS_UPDATE_GLOBS
        .iter()
        .any(|pattern| glob_match(pattern, rel))
}

fn glob_match(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let seg: Vec<&str> = path.split('/').collect();
    glob_segments(&pat, &seg)
}

fn glob_segments(pat: &[&str], path: &[&str]) -> bool {
    match pat.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|i| glob_segments(rest, &path[i..])),
        Some((first, rest)) => {
            if let Some((head, tail)) = path.split_first() {
                segment_match(first, head) && glob_segments(rest, tail)
            } else {
                false
            }
        }
    }
}

fn segment_match(pattern: &str, segment: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == segment;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0usize;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 {
            if !segment[pos..].starts_with(part) {
                return false;
            }
            pos += part.len();
        } else if index == parts.len() - 1 {
            if !segment[pos..].ends_with(part) {
                return false;
            }
        } else if let Some(found) = segment[pos..].find(part) {
            pos += found + part.len();
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    #[test]
    fn glob_exact_and_wildcards() {
        assert!(glob_match(
            "config/fabric_loader_dependencies.json",
            "config/fabric_loader_dependencies.json"
        ));
        assert!(!glob_match(
            "config/fabric_loader_dependencies.json",
            "config/other.json"
        ));
        assert!(glob_match("config/*.json", "config/anything.json"));
        assert!(!glob_match("config/*.json", "config/nested/anything.json"));
        assert!(glob_match("config/**", "config/nested/deep/file.toml"));
        assert!(glob_match("**/options.txt", "a/b/options.txt"));
        assert!(glob_match("**/options.txt", "options.txt"));
        assert!(glob_match("config/mod-*.cfg", "config/mod-foo.cfg"));
        assert!(!glob_match("config/mod-*.cfg", "config/other-foo.cfg"));
    }

    fn tmp_root(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);

        let dir =
            std::env::temp_dir().join(format!("oneclient_ovr_{}_{tag}_{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    async fn write_bundle(path: &Path, entries: &[(&str, &[u8])]) {
        let file = tokio::fs::File::create(path).await.unwrap();
        let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
        for (name, data) in entries {
            let builder = async_zip::ZipEntryBuilder::new(
                (*name).to_string().into(),
                async_zip::Compression::Stored,
            );
            writer.write_entry_whole(builder, data).await.unwrap();
        }
        writer.close().await.unwrap();
    }

    async fn read(path: &Path) -> Option<Vec<u8>> {
        polyio::read(path).await.ok()
    }

    async fn sync(zip: &Path, root: &Path) -> OverrideSyncReport {
        sync_bundle_overrides_at(zip, "test-bundle", root, None)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn fresh_install_writes_all() {
        let root = tmp_root("fresh");
        let zip = root.join("v1.mrpack");
        write_bundle(
            &zip,
            &[
                ("overrides/config/a.toml", b"alpha"),
                ("overrides/config/nested/b.cfg", b"beta"),
                ("overrides/", b""),
            ],
        )
        .await;

        let report = sync(&zip, &root).await;
        assert_eq!(report.written.len(), 2);
        assert!(report.conflicts.is_empty());
        assert_eq!(read(&root.join("config/a.toml")).await.unwrap(), b"alpha");
        assert_eq!(
            read(&root.join("config/nested/b.cfg")).await.unwrap(),
            b"beta"
        );
    }

    #[tokio::test]
    async fn rerun_same_version_is_noop() {
        let root = tmp_root("noop");
        let zip = root.join("v1.mrpack");
        write_bundle(&zip, &[("overrides/config/a.toml", b"alpha")]).await;
        sync(&zip, &root).await;

        let report = sync(&zip, &root).await;
        assert!(report.written.is_empty());
        assert!(report.conflicts.is_empty());
        assert!(report.deleted.is_empty());
    }

    #[tokio::test]
    async fn untouched_file_updates() {
        let root = tmp_root("update");
        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[("overrides/config/a.toml", b"alpha")]).await;
        sync(&v1, &root).await;

        let v2 = root.join("v2.mrpack");
        write_bundle(&v2, &[("overrides/config/a.toml", b"alpha-v2")]).await;
        let report = sync(&v2, &root).await;

        assert_eq!(report.written, vec!["config/a.toml".to_string()]);
        assert!(report.conflicts.is_empty());
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"alpha-v2"
        );
    }

    #[tokio::test]
    async fn user_edit_with_bundle_unchanged_is_kept_silently() {
        let root = tmp_root("edit_noconflict");
        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[("overrides/config/a.toml", b"alpha")]).await;
        sync(&v1, &root).await;

        polyio::write(root.join("config/a.toml"), b"user-edit")
            .await
            .unwrap();

        let report = sync(&v1, &root).await;
        assert!(report.written.is_empty());
        assert!(report.conflicts.is_empty());
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"user-edit"
        );
    }

    #[tokio::test]
    async fn user_edit_and_bundle_changed_is_conflict_kept() {
        let root = tmp_root("conflict");
        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[("overrides/config/a.toml", b"alpha")]).await;
        sync(&v1, &root).await;

        polyio::write(root.join("config/a.toml"), b"user-edit")
            .await
            .unwrap();

        let v2 = root.join("v2.mrpack");
        write_bundle(&v2, &[("overrides/config/a.toml", b"alpha-v2")]).await;
        let report = sync(&v2, &root).await;

        assert_eq!(report.conflicts, vec!["config/a.toml".to_string()]);
        assert!(report.written.is_empty());

        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"user-edit"
        );

        let report2 = sync(&v2, &root).await;
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"user-edit"
        );
        assert_eq!(report2.conflicts, vec!["config/a.toml".to_string()]);
    }

    #[tokio::test]
    async fn always_update_overwrites_user_edit() {
        let root = tmp_root("always");
        let path = "config/fabric_loader_dependencies.json";
        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[(&format!("overrides/{path}"), b"deps-v1")]).await;
        sync(&v1, &root).await;

        polyio::write(root.join(path), b"runtime-mutated")
            .await
            .unwrap();

        let v2 = root.join("v2.mrpack");
        write_bundle(&v2, &[(&format!("overrides/{path}"), b"deps-v2")]).await;
        let report = sync(&v2, &root).await;

        assert!(report.conflicts.is_empty());
        assert_eq!(read(&root.join(path)).await.unwrap(), b"deps-v2");
    }

    #[tokio::test]
    async fn removed_file_deleted_only_when_unmodified() {
        let root = tmp_root("removed");
        let v1 = root.join("v1.mrpack");
        write_bundle(
            &v1,
            &[
                ("overrides/config/gone.toml", b"g"),
                ("overrides/config/kept.toml", b"k"),
            ],
        )
        .await;
        sync(&v1, &root).await;

        // User modified kept.toml; gone.toml untouched.
        polyio::write(root.join("config/kept.toml"), b"edited")
            .await
            .unwrap();

        let v2 = root.join("v2.mrpack");
        write_bundle(&v2, &[("overrides/config/a.toml", b"a")]).await;
        let report = sync(&v2, &root).await;

        assert_eq!(report.deleted, vec!["config/gone.toml".to_string()]);
        assert!(read(&root.join("config/gone.toml")).await.is_none());

        assert_eq!(
            read(&root.join("config/kept.toml")).await.unwrap(),
            b"edited"
        );
    }

    #[tokio::test]
    async fn bootstrap_pre_existing_edit_is_never_overwritten() {
        let root = tmp_root("bootstrap");

        polyio::create_dir_all(root.join("config"))
            .await
            .unwrap();
        polyio::write(root.join("config/a.toml"), b"pre-existing")
            .await
            .unwrap();

        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[("overrides/config/a.toml", b"bundle-version")]).await;
        let report = sync(&v1, &root).await;

        assert!(report.written.is_empty());
        assert!(report.conflicts.is_empty());
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"pre-existing"
        );

        let report2 = sync(&v1, &root).await;
        assert!(report2.written.is_empty());
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"pre-existing"
        );
    }
}
