use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use oneclient_common::domain::GameLoader;
use oneclient_db::models::ClusterRow;
use serde::{Deserialize, Serialize};

use crate::bundles::BundlesManager;
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use oneclient_events::EventBus;
use polyio::{ZipEntryCursor, sha1_bytes, sha1_file};

const ALWAYS_UPDATE_GLOBS: &[&str] = &["config/fabric_loader_dependencies.json"];

const OVERRIDES_PREFIX: &str = "overrides/";
const LOCK_REL_PATH: &str = ".oneclient/bundle_overrides.json";
const MERGED_DIR: &str = ".oneclient/merged_overrides";
const MERGED_BASE: &str = "base.json";
const MERGED_TARGET: &str = "path.txt";
const MERGED_PARTS: &str = "parts";

fn cluster_root(cluster: &ClusterRow) -> ContentResult<PathBuf> {
    Ok(oneclient_common::paths::clusters_dir()?.join(&cluster.folder_name))
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

    async fn save(&self, root: &Path) -> ContentResult<()> {
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

#[tracing::instrument(level = "debug", skip(cluster, events))]
pub async fn sync_bundle_overrides(
    archive_path: &Path,
    bundle_name: &str,
    cluster: &ClusterRow,
    events: Option<&EventBus>,
) -> ContentResult<OverrideSyncReport> {
    let root = cluster_root(cluster)?;
    sync_bundle_overrides_at(archive_path, bundle_name, &root, events).await
}

#[tracing::instrument(level = "debug", skip(events))]
async fn sync_bundle_overrides_at(
    archive_path: &Path,
    bundle_name: &str,
    root: &Path,
    events: Option<&EventBus>,
) -> ContentResult<OverrideSyncReport> {
    sync_layered_overrides(
        archive_path,
        bundle_name,
        root,
        &[OVERRIDES_PREFIX],
        &|_| true,
        true,
        events,
    )
    .await
}

pub async fn bundle_override_paths(
    bundles: &BundlesManager,
    ctx: &ContentCtx,
    mc_version: &str,
    loader: GameLoader,
) -> ContentResult<HashSet<String>> {
    let mut paths = HashSet::new();
    for archive in bundles.archives_for(ctx, mc_version, loader).await? {
        let layers = OverrideLayers::open(&archive.bundle.path, &[OVERRIDES_PREFIX]).await?;
        paths.extend(layers.entries().iter().map(|(rel, _)| rel.clone()));
    }
    Ok(paths)
}

pub(crate) struct OverrideLayers {
    cursor: ZipEntryCursor,
    entries: Vec<(String, String)>,
}

impl OverrideLayers {
    pub(crate) async fn open(archive_path: &Path, prefixes: &[&str]) -> ContentResult<Self> {
        let layer_of = |name: &str| prefixes.iter().position(|prefix| name.starts_with(prefix));
        let cursor = ZipEntryCursor::open(archive_path, |name| layer_of(name).is_some()).await?;

        let mut winners: HashMap<String, (usize, String)> = HashMap::new();
        for name in cursor.names() {
            let Some(layer) = layer_of(name) else {
                continue;
            };
            let rel = &name[prefixes[layer].len()..];
            if rel.is_empty() {
                continue;
            }
            let replaces = winners
                .get(rel)
                .is_none_or(|(current, _)| layer >= *current);
            if replaces {
                winners.insert(rel.to_string(), (layer, name.clone()));
            }
        }

        let mut entries: Vec<(String, String)> = winners
            .into_iter()
            .map(|(rel, (_, name))| (rel, name))
            .collect();
        entries.sort();

        Ok(Self { cursor, entries })
    }

    pub(crate) fn entries(&self) -> &[(String, String)] {
        &self.entries
    }

    pub(crate) async fn read(&mut self, entry: &str) -> Option<Vec<u8>> {
        match self.cursor.read(entry).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                tracing::warn!(entry, error = %err, "skipping an override that could not be read");
                None
            }
        }
    }
}

#[tracing::instrument(level = "debug", skip(keep, events))]
pub(crate) async fn sync_layered_overrides(
    archive_path: &Path,
    bundle_name: &str,
    root: &Path,
    prefixes: &[&str],
    keep: &(dyn Fn(&str) -> bool + Sync),
    adopt_existing: bool,
    events: Option<&EventBus>,
) -> ContentResult<OverrideSyncReport> {
    let mut layers = OverrideLayers::open(archive_path, prefixes).await?;
    let entries: Vec<(String, String)> = layers
        .entries()
        .iter()
        .filter(|(rel, _)| keep(rel))
        .cloned()
        .collect();

    let mut lock = OverrideLock::load(root).await;
    let previous = lock.bundles.remove(bundle_name).unwrap_or_default();
    let mut next: HashMap<String, String> = HashMap::new();
    let mut report = OverrideSyncReport::default();
    let mut merged_parts: HashSet<String> = HashSet::new();

    for (rel, entry) in entries {
        let rel = rel.as_str();
        let merges = !adopt_existing && matches_always_update(rel);
        let Some(bytes) = layers.read(&entry).await else {
            if merges {
                merged_parts.insert(rel.to_string());
            } else if let Some(base) = previous.get(rel) {
                next.insert(rel.to_string(), base.clone());
            }
            continue;
        };

        let new_sha1 = sha1_bytes(&bytes);
        let dest = root.join(polyio::sanitize_path(rel));
        let disk_sha1 = current_sha1(&dest).await;
        let base = previous.get(rel).cloned();

        if merges {
            let ours = disk_sha1.is_none()
                || base
                    .as_deref()
                    .is_some_and(|claimed| disk_sha1.as_deref() == Some(claimed))
                || own_part_matches(root, rel, bundle_name, disk_sha1.as_deref()).await;
            let collides = !ours
                || has_other_copies(root, rel, bundle_name).await
                || claimed_elsewhere(&lock, rel);
            if collides {
                convert_claims_to_parts(&mut lock, root, rel, bundle_name).await;
                add_merge_part(root, rel, bundle_name, &bytes).await;
                merged_parts.insert(rel.to_string());
                continue;
            }

            discard_own_part(root, rel, bundle_name).await;
            if write_override(&dest, &bytes, rel).await {
                if disk_sha1.as_deref() != Some(new_sha1.as_str()) {
                    report.written.push(rel.to_string());
                }
                next.insert(rel.to_string(), new_sha1);
            } else if let Some(claimed) = base {
                next.insert(rel.to_string(), claimed);
            }
            continue;
        }

        if !adopt_existing
            && base.is_none()
            && let Some(disk) = &disk_sha1
        {
            if *disk != new_sha1 {
                report.conflicts.push(rel.to_string());
            }
            continue;
        }

        if matches_always_update(rel) {
            convert_claims_to_parts(&mut lock, root, rel, bundle_name).await;
            let wrote = write_merge_base(root, rel, &bytes).await;
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
            None => {
                if write_override(&dest, &bytes, rel).await {
                    report.written.push(rel.to_string());
                    next.insert(rel.to_string(), new_sha1);
                }
            }
            Some(disk) if disk == new_sha1 => {
                next.insert(rel.to_string(), new_sha1);
            }
            Some(disk) if base.as_deref() == Some(disk.as_str()) => {
                if write_override(&dest, &bytes, rel).await {
                    report.written.push(rel.to_string());
                    next.insert(rel.to_string(), new_sha1);
                } else {
                    next.insert(rel.to_string(), disk);
                }
            }
            Some(_) => {
                if let Some(b) = base {
                    if b != new_sha1 {
                        report.conflicts.push(rel.to_string());
                    }
                    next.insert(rel.to_string(), b);
                }
            }
        }
    }

    for (rel, base_sha1) in &previous {
        if next.contains_key(rel) {
            continue;
        }
        if matches_always_update(rel) {
            if merged_parts.contains(rel) {
                continue;
            }
            if adopt_existing && drop_merge_base(root, rel).await {
                report.deleted.push(rel.clone());
                continue;
            }
        }
        let dest = root.join(polyio::sanitize_path(rel));

        if current_sha1(&dest).await.as_deref() == Some(base_sha1.as_str())
            && polyio::remove_file(&dest).await.is_ok()
        {
            report.deleted.push(rel.clone());
        }
    }

    if !adopt_existing {
        prune_merge_parts(root, bundle_name, &merged_parts).await;
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
        notify_conflicts(events, bundle_name, &report.conflicts);
    }

    Ok(report)
}

pub(crate) async fn lock_entries(root: &Path, key: &str) -> HashMap<String, String> {
    OverrideLock::load(root)
        .await
        .bundles
        .remove(key)
        .unwrap_or_default()
}

pub(crate) async fn sync_file_lock(
    root: &Path,
    key: &str,
    written: HashMap<String, String>,
    listed: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut lock = OverrideLock::load(root).await;
    let previous = lock.bundles.remove(key).unwrap_or_default();
    let mut next = written;
    let mut deleted = Vec::new();

    for (rel, base_sha1) in previous {
        if next.contains_key(&rel) {
            continue;
        }
        if listed.contains(&rel) {
            next.insert(rel, base_sha1);
            continue;
        }

        let dest = root.join(polyio::sanitize_path(&rel));
        if current_sha1(&dest).await.as_deref() == Some(base_sha1.as_str())
            && polyio::remove_file(&dest).await.is_ok()
        {
            deleted.push(rel);
        }
    }

    lock.bundles.insert(key.to_string(), next);
    if let Err(err) = lock.save(root).await {
        tracing::warn!(error = %err, "failed to persist the file lock");
    }

    deleted
}

fn merge_key(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn merged_dir(root: &Path, rel: &str) -> PathBuf {
    root.join(MERGED_DIR).join(merge_key(rel))
}

fn merge_part_path(root: &Path, rel: &str, bundle_name: &str) -> PathBuf {
    merged_dir(root, rel)
        .join(MERGED_PARTS)
        .join(format!("{}.json", merge_key(bundle_name)))
}

fn merge_missing(target: &mut serde_json::Value, extra: &serde_json::Value) {
    let (serde_json::Value::Object(target), serde_json::Value::Object(extra)) = (target, extra)
    else {
        return;
    };
    for (key, value) in extra {
        match target.get_mut(key) {
            Some(existing) => merge_missing(existing, value),
            None => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

async fn merge_parts(dir: &Path) -> Vec<Vec<u8>> {
    let mut paths = Vec::new();
    if let Ok(mut entries) = polyio::read_dir(dir.join(MERGED_PARTS)).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            paths.push(entry.path());
        }
    }
    paths.sort();

    let mut parts = Vec::new();
    for path in paths {
        if let Ok(bytes) = polyio::read(&path).await {
            parts.push(bytes);
        }
    }
    parts
}

async fn compose_merged(root: &Path, rel: &str) -> Option<Vec<u8>> {
    let dir = merged_dir(root, rel);
    let base = polyio::read(dir.join(MERGED_BASE)).await.ok();
    let parts = merge_parts(&dir).await;
    compose_bytes(rel, base, parts)
}

fn compose_bytes(rel: &str, base: Option<Vec<u8>>, mut parts: Vec<Vec<u8>>) -> Option<Vec<u8>> {
    if parts.is_empty() {
        return base;
    }
    if base.is_none() && parts.len() == 1 {
        return parts.pop();
    }

    let mut merged = base
        .as_deref()
        .and_then(|bytes| parse_json(bytes).ok())
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    for part in &parts {
        match parse_json(part) {
            Ok(value) => merge_missing(&mut merged, &value),
            Err(err) => tracing::warn!(path = rel, error = %err, "skipping an unreadable merged override"),
        }
    }
    if rel.ends_with(FABRIC_DEPENDENCIES_FILE) {
        merged = fabric_dependency_overrides(&merged);
    }
    version_first_json(&merged)
}

const FABRIC_DEPENDENCIES_FILE: &str = "fabric_loader_dependencies.json";
const FABRIC_DEPENDENCY_KINDS: [&str; 5] = ["depends", "recommends", "suggests", "conflicts", "breaks"];

fn parse_json(bytes: &[u8]) -> serde_json::Result<serde_json::Value> {
    serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes))
}

fn fabric_dependency_overrides(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::{Map, Value};

    let mut overrides = Map::new();
    if let Some(Value::Object(mods)) = value.get("overrides") {
        for (mod_id, container) in mods {
            let Value::Object(container) = container else {
                continue;
            };
            let mut kept = Map::new();
            for (key, deps) in container {
                let kind = key
                    .strip_prefix('+')
                    .or_else(|| key.strip_prefix('-'))
                    .unwrap_or(key);
                let Value::Object(deps) = deps else {
                    continue;
                };
                if !FABRIC_DEPENDENCY_KINDS.contains(&kind) {
                    continue;
                }
                let valid: Map<String, Value> = deps
                    .iter()
                    .filter(|(_, range)| match range {
                        Value::String(_) => true,
                        Value::Array(ranges) => ranges.iter().all(Value::is_string),
                        _ => false,
                    })
                    .map(|(dep, range)| (dep.clone(), range.clone()))
                    .collect();
                kept.insert(key.clone(), Value::Object(valid));
            }
            overrides.insert(mod_id.clone(), Value::Object(kept));
        }
    }

    let mut root = Map::new();
    root.insert("version".to_string(), Value::from(1));
    root.insert("overrides".to_string(), Value::Object(overrides));
    Value::Object(root)
}

fn version_first_json(value: &serde_json::Value) -> Option<Vec<u8>> {
    let serde_json::Value::Object(map) = value else {
        return serde_json::to_vec_pretty(value).ok();
    };
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort_by_key(|key| key.as_str() != "version");

    let mut out = String::from("{\n");
    for (index, key) in keys.iter().enumerate() {
        let rendered = serde_json::to_string_pretty(&map[key.as_str()]).ok()?;
        out.push_str("  ");
        out.push_str(&serde_json::to_string(key).ok()?);
        out.push_str(": ");
        out.push_str(&rendered.replace('\n', "\n  "));
        out.push_str(if index + 1 < keys.len() { ",\n" } else { "\n" });
    }
    out.push('}');
    Some(out.into_bytes())
}

async fn apply_merged(root: &Path, rel: &str) -> bool {
    let dest = root.join(polyio::sanitize_path(rel));
    match compose_merged(root, rel).await {
        Some(bytes) => write_override(&dest, &bytes, rel).await,
        None => {
            if polyio::try_exists(&dest).await.unwrap_or(false) {
                polyio::remove_file(&dest).await.is_ok()
            } else {
                true
            }
        }
    }
}

async fn remember_merge_target(dir: &Path, rel: &str) {
    if polyio::create_dir_all(dir.join(MERGED_PARTS)).await.is_ok() {
        polyio::write(dir.join(MERGED_TARGET), rel).await.ok();
    }
}

async fn write_merge_base(root: &Path, rel: &str, bytes: &[u8]) -> bool {
    let dir = merged_dir(root, rel);
    remember_merge_target(&dir, rel).await;
    if let Err(err) = polyio::write(dir.join(MERGED_BASE), bytes).await {
        tracing::warn!(path = rel, error = %err, "could not keep the merge base");
        return write_override(&root.join(polyio::sanitize_path(rel)), bytes, rel).await;
    }
    apply_merged(root, rel).await
}

async fn drop_merge_base(root: &Path, rel: &str) -> bool {
    let base = merged_dir(root, rel).join(MERGED_BASE);
    if !polyio::try_exists(&base).await.unwrap_or(false) {
        return false;
    }
    polyio::remove_file(&base).await.ok();
    apply_merged(root, rel).await
}

async fn own_part_matches(
    root: &Path,
    rel: &str,
    bundle_name: &str,
    disk_sha1: Option<&str>,
) -> bool {
    let Some(disk_sha1) = disk_sha1 else {
        return false;
    };
    polyio::read(merge_part_path(root, rel, bundle_name))
        .await
        .is_ok_and(|bytes| sha1_bytes(&bytes) == disk_sha1)
}

async fn has_other_copies(root: &Path, rel: &str, bundle_name: &str) -> bool {
    let dir = merged_dir(root, rel);
    if polyio::try_exists(dir.join(MERGED_BASE))
        .await
        .unwrap_or(false)
    {
        return true;
    }
    let own = merge_part_path(root, rel, bundle_name);
    let Ok(mut entries) = polyio::read_dir(dir.join(MERGED_PARTS)).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.path() != own {
            return true;
        }
    }
    false
}

fn claimed_elsewhere(lock: &OverrideLock, rel: &str) -> bool {
    lock.bundles.values().any(|entries| entries.contains_key(rel))
}

async fn discard_own_part(root: &Path, rel: &str, bundle_name: &str) {
    let part = merge_part_path(root, rel, bundle_name);
    if polyio::try_exists(&part).await.unwrap_or(false) {
        polyio::remove_file(&part).await.ok();
    }
}

async fn convert_claims_to_parts(
    lock: &mut OverrideLock,
    root: &Path,
    rel: &str,
    bundle_name: &str,
) {
    let claimants: Vec<(String, String)> = lock
        .bundles
        .iter()
        .filter(|(key, _)| key.as_str() != bundle_name && crate::modpacks::is_imported_bundle(key))
        .filter_map(|(key, entries)| Some((key.clone(), entries.get(rel)?.clone())))
        .collect();
    if claimants.is_empty() {
        return;
    }

    let dest = root.join(polyio::sanitize_path(rel));
    let disk = polyio::read(&dest).await.ok();
    let disk_sha1 = disk.as_deref().map(sha1_bytes);
    let dir = merged_dir(root, rel);
    remember_merge_target(&dir, rel).await;

    for (key, claimed) in claimants {
        if let Some(entries) = lock.bundles.get_mut(&key) {
            entries.remove(rel);
        }
        if let Some(bytes) = &disk
            && disk_sha1.as_deref() == Some(claimed.as_str())
        {
            polyio::write(merge_part_path(root, rel, &key), bytes).await.ok();
        }
    }
}

async fn add_merge_part(root: &Path, rel: &str, bundle_name: &str, bytes: &[u8]) {
    let dir = merged_dir(root, rel);
    let base = dir.join(MERGED_BASE);
    let dest = root.join(polyio::sanitize_path(rel));
    if !polyio::try_exists(&base).await.unwrap_or(false) && merge_parts(&dir).await.is_empty() {
        remember_merge_target(&dir, rel).await;
        if let Ok(existing) = polyio::read(&dest).await {
            polyio::write(&base, existing).await.ok();
        }
    }

    remember_merge_target(&dir, rel).await;
    if let Err(err) = polyio::write(merge_part_path(root, rel, bundle_name), bytes).await {
        tracing::warn!(path = rel, bundle = bundle_name, error = %err, "could not keep a merged override");
        return;
    }
    apply_merged(root, rel).await;
}

pub(crate) async fn move_lock_entries(root: &Path, from: &str, to: &str) {
    let mut lock = OverrideLock::load(root).await;
    let Some(moved) = lock.bundles.remove(from) else {
        return;
    };
    let target = lock.bundles.entry(to.to_string()).or_default();
    for (rel, sha1) in moved {
        target.entry(rel).or_insert(sha1);
    }
    if let Err(err) = lock.save(root).await {
        tracing::warn!(error = %err, "failed to persist the handed over file lock");
    }
}

pub(crate) async fn move_merge_parts(root: &Path, from: &str, to: &str) {
    let Ok(mut entries) = polyio::read_dir(root.join(MERGED_DIR)).await else {
        return;
    };
    let mut dirs = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        dirs.push(entry.path());
    }

    for dir in dirs {
        let Ok(rel) = polyio::read(dir.join(MERGED_TARGET)).await else {
            continue;
        };
        let rel = String::from_utf8_lossy(&rel).trim().to_string();
        let from_part = merge_part_path(root, &rel, from);
        let Ok(bytes) = polyio::read(&from_part).await else {
            continue;
        };
        let to_part = merge_part_path(root, &rel, to);
        if !polyio::try_exists(&to_part).await.unwrap_or(false) {
            polyio::write(&to_part, bytes).await.ok();
        }
        polyio::remove_file(&from_part).await.ok();
        apply_merged(root, &rel).await;
    }
}

pub(crate) async fn prune_merge_parts(root: &Path, bundle_name: &str, keep: &HashSet<String>) {
    let Ok(mut entries) = polyio::read_dir(root.join(MERGED_DIR)).await else {
        return;
    };
    let mut dirs = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        dirs.push(entry.path());
    }

    for dir in dirs {
        let Ok(rel) = polyio::read(dir.join(MERGED_TARGET)).await else {
            continue;
        };
        let rel = String::from_utf8_lossy(&rel).trim().to_string();
        if rel.is_empty() || keep.contains(&rel) {
            continue;
        }
        let part = merge_part_path(root, &rel, bundle_name);
        if polyio::try_exists(&part).await.unwrap_or(false) && polyio::remove_file(&part).await.is_ok()
        {
            apply_merged(root, &rel).await;
        }
    }
}

async fn write_override(dest: &Path, bytes: &[u8], rel: &str) -> bool {
    if let Some(parent) = dest.parent() {
        polyio::create_dir_all(parent).await.ok();
    }
    match polyio::write_atomic(dest, bytes).await {
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

fn notify_conflicts(events: Option<&EventBus>, bundle_name: &str, conflicts: &[String]) {
    let Some(events) = events else {
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

    events.notify("Kept your config edits")
        .body(format!(
            "{bundle_name}: {} config file(s) you edited were left untouched by the update: {listed}{suffix}",
            conflicts.len()
        ))
        .send();
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

    use super::*;

    const FABRIC_DEPS: &str = "config/fabric_loader_dependencies.json";

    fn first_key(bytes: &[u8]) -> String {
        let text = std::str::from_utf8(bytes).unwrap();
        let start = text.find('"').unwrap() + 1;
        let end = start + text[start..].find('"').unwrap();
        text[start..end].to_string()
    }

    #[test]
    fn a_single_pack_copy_is_written_unchanged() {
        let part = br#"{"version":1,"overrides":{"a":{"-depends":{"b":"*"}}}}"#.to_vec();
        let out = compose_bytes(FABRIC_DEPS, None, vec![part.clone()]).unwrap();
        assert_eq!(out, part);
    }

    #[test]
    fn a_merged_file_keeps_version_as_the_first_key() {
        let base = br#"{"version":1,"overrides":{"owner":{"+breaks":{"x":"*"}}}}"#.to_vec();
        let part = br#"{"version":1,"overrides":{"pack":{"-depends":{"y":"IGNORED"}},"owner":{"+breaks":{"x":"<1"}}}}"#.to_vec();
        let out = compose_bytes(FABRIC_DEPS, Some(base), vec![part]).unwrap();

        assert_eq!(first_key(&out), "version");
        let merged: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(merged["version"], 1);
        assert_eq!(merged["overrides"]["owner"]["+breaks"]["x"], "*");
        assert_eq!(merged["overrides"]["pack"]["-depends"]["y"], "IGNORED");
    }

    #[test]
    fn two_packs_without_a_base_still_put_version_first() {
        let a = br#"{"version":1,"overrides":{"a":{"-depends":{"m":"*"}}}}"#.to_vec();
        let b = br#"{"overrides":{"b":{"+breaks":{"n":"*"}}},"version":1}"#.to_vec();
        let out = compose_bytes(FABRIC_DEPS, None, vec![a, b]).unwrap();

        assert_eq!(first_key(&out), "version");
        let merged: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(merged["overrides"]["a"]["-depends"]["m"], "*");
        assert_eq!(merged["overrides"]["b"]["+breaks"]["n"], "*");
    }

    #[test]
    fn no_parts_returns_the_owner_copy_untouched() {
        let base = br#"{"overrides":{},"version":1}"#.to_vec();
        assert_eq!(compose_bytes(FABRIC_DEPS, Some(base.clone()), Vec::new()), Some(base));
        assert_eq!(compose_bytes(FABRIC_DEPS, None, Vec::new()), None);
    }

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
    async fn a_later_layer_wins_over_an_earlier_one() {
        let root = polyio::testing::ScratchDir::new("layers");
        let zip = root.join("pack.mrpack");
        write_bundle(
            &zip,
            &[
                ("client-overrides/options.txt", b"client"),
                ("overrides/options.txt", b"common"),
                ("overrides/config/a.toml", b"alpha"),
                ("server-overrides/server.properties", b"server"),
            ],
        )
        .await;

        let report = sync_layered_overrides(
            &zip,
            "test-pack",
            root.path(),
            &["overrides/", "client-overrides/"],
            &|_| true,
            true,
            None,
        )
        .await
        .unwrap();

        assert_eq!(report.written.len(), 2);
        assert_eq!(read(&root.join("options.txt")).await.unwrap(), b"client");
        assert_eq!(read(&root.join("config/a.toml")).await.unwrap(), b"alpha");
        assert!(read(&root.join("server.properties")).await.is_none());
    }

    #[tokio::test]
    async fn entries_the_filter_rejects_are_left_alone() {
        let root = polyio::testing::ScratchDir::new("keep-filter");
        let zip = root.join("pack.mrpack");
        write_bundle(
            &zip,
            &[
                ("overrides/mods/bundled.jar", b"jar"),
                ("overrides/config/a.toml", b"alpha"),
            ],
        )
        .await;

        let report = sync_layered_overrides(
            &zip,
            "test-pack",
            root.path(),
            &["overrides/"],
            &|rel| !rel.starts_with("mods/"),
            true,
            None,
        )
        .await
        .unwrap();

        assert_eq!(report.written, vec!["config/a.toml".to_string()]);
        assert!(read(&root.join("mods/bundled.jar")).await.is_none());
    }

    #[tokio::test]
    async fn a_file_dropped_from_the_lock_is_deleted_only_when_untouched() {
        let root = polyio::testing::ScratchDir::new("file-lock");
        polyio::create_dir_all(root.join("config")).await.unwrap();
        polyio::write(root.join("config/kept.json"), b"mine")
            .await
            .unwrap();
        polyio::write(root.join("config/gone.json"), b"pack")
            .await
            .unwrap();
        polyio::write(root.join("config/edited.json"), b"edited")
            .await
            .unwrap();

        let first: HashMap<String, String> = [
            ("config/kept.json", b"mine".as_slice()),
            ("config/gone.json", b"pack".as_slice()),
            ("config/edited.json", b"pack".as_slice()),
        ]
        .into_iter()
        .map(|(rel, bytes)| (rel.to_string(), sha1_bytes(bytes)))
        .collect();
        sync_file_lock(root.path(), "files", first, &Default::default()).await;

        let listed = ["config/kept.json".to_string()].into_iter().collect();
        let deleted = sync_file_lock(root.path(), "files", HashMap::new(), &listed).await;

        assert_eq!(deleted, vec!["config/gone.json".to_string()]);
        assert!(read(&root.join("config/kept.json")).await.is_some());
        assert!(read(&root.join("config/edited.json")).await.is_some());
    }

    #[tokio::test]
    async fn a_large_override_is_not_skipped() {
        let root = polyio::testing::ScratchDir::new("large-override");
        let zip = root.join("pack.mrpack");
        let big = vec![7u8; 17 * 1024 * 1024];
        write_bundle(&zip, &[("overrides/resourcepacks/big.zip", big.as_slice())]).await;

        let report = sync(&zip, root.path()).await;

        assert_eq!(report.written, vec!["resourcepacks/big.zip".to_string()]);
        assert_eq!(
            read(&root.join("resourcepacks/big.zip"))
                .await
                .map(|bytes| bytes.len()),
            Some(big.len())
        );
    }

    #[tokio::test]
    async fn fresh_install_writes_all() {
        let root = polyio::testing::ScratchDir::new("fresh");
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

        let report = sync(&zip, root.path()).await;
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
        let root = polyio::testing::ScratchDir::new("noop");
        let zip = root.join("v1.mrpack");
        write_bundle(&zip, &[("overrides/config/a.toml", b"alpha")]).await;
        sync(&zip, root.path()).await;

        let report = sync(&zip, root.path()).await;
        assert!(report.written.is_empty());
        assert!(report.conflicts.is_empty());
        assert!(report.deleted.is_empty());
    }

    #[tokio::test]
    async fn untouched_file_updates() {
        let root = polyio::testing::ScratchDir::new("update");
        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[("overrides/config/a.toml", b"alpha")]).await;
        sync(&v1, root.path()).await;

        let v2 = root.join("v2.mrpack");
        write_bundle(&v2, &[("overrides/config/a.toml", b"alpha-v2")]).await;
        let report = sync(&v2, root.path()).await;

        assert_eq!(report.written, vec!["config/a.toml".to_string()]);
        assert!(report.conflicts.is_empty());
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"alpha-v2"
        );
    }

    #[tokio::test]
    async fn user_edit_with_bundle_unchanged_is_kept_silently() {
        let root = polyio::testing::ScratchDir::new("edit_noconflict");
        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[("overrides/config/a.toml", b"alpha")]).await;
        sync(&v1, root.path()).await;

        polyio::write(root.join("config/a.toml"), b"user-edit")
            .await
            .unwrap();

        let report = sync(&v1, root.path()).await;
        assert!(report.written.is_empty());
        assert!(report.conflicts.is_empty());
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"user-edit"
        );
    }

    #[tokio::test]
    async fn user_edit_and_bundle_changed_is_conflict_kept() {
        let root = polyio::testing::ScratchDir::new("conflict");
        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[("overrides/config/a.toml", b"alpha")]).await;
        sync(&v1, root.path()).await;

        polyio::write(root.join("config/a.toml"), b"user-edit")
            .await
            .unwrap();

        let v2 = root.join("v2.mrpack");
        write_bundle(&v2, &[("overrides/config/a.toml", b"alpha-v2")]).await;
        let report = sync(&v2, root.path()).await;

        assert_eq!(report.conflicts, vec!["config/a.toml".to_string()]);
        assert!(report.written.is_empty());

        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"user-edit"
        );

        let report2 = sync(&v2, root.path()).await;
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"user-edit"
        );
        assert_eq!(report2.conflicts, vec!["config/a.toml".to_string()]);
    }

    const PACK_KEY: &str = "modpack:modrinth:abc";
    const PACK_DEPS: &[u8] = br#"{"version":1,"overrides":{"pack-mod":{"-depends":{"x":"*"}}}}"#;
    const OWNER_DEPS: &[u8] =
        br#"{"version":1,"overrides":{"owner-mod":{"+breaks":{"y":"*"}}}}"#;

    async fn sync_pack(zip: &Path, root: &Path) -> OverrideSyncReport {
        sync_layered_overrides(zip, PACK_KEY, root, &[OVERRIDES_PREFIX], &|_| true, false, None)
            .await
            .unwrap()
    }

    async fn deps_bundle(root: &polyio::testing::ScratchDir, name: &str, bytes: &[u8]) -> PathBuf {
        let zip = root.join(name);
        write_bundle(&zip, &[(&format!("overrides/{FABRIC_DEPS}"), bytes)]).await;
        zip
    }

    fn assert_both_merged(bytes: &[u8]) {
        assert_eq!(first_key(bytes), "version");
        let merged: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(merged["overrides"]["pack-mod"]["-depends"]["x"], "*");
        assert_eq!(merged["overrides"]["owner-mod"]["+breaks"]["y"], "*");
    }

    #[tokio::test]
    async fn an_added_pack_alone_owns_its_file_unchanged() {
        let root = polyio::testing::ScratchDir::new("pack-alone");
        let pack = deps_bundle(&root, "pack.mrpack", PACK_DEPS).await;
        sync_pack(&pack, root.path()).await;

        assert_eq!(read(&root.join(FABRIC_DEPS)).await.unwrap(), PACK_DEPS);
        assert!(
            read(&merge_part_path(root.path(), FABRIC_DEPS, PACK_KEY))
                .await
                .is_none()
        );
        let lock = OverrideLock::load(root.path()).await;
        assert!(lock.bundles[PACK_KEY].contains_key(FABRIC_DEPS));
    }

    #[tokio::test]
    async fn an_added_pack_merges_into_an_owner_file() {
        let root = polyio::testing::ScratchDir::new("pack-after-owner");
        let owner = deps_bundle(&root, "owner.mrpack", OWNER_DEPS).await;
        sync(&owner, root.path()).await;
        let pack = deps_bundle(&root, "pack.mrpack", PACK_DEPS).await;
        sync_pack(&pack, root.path()).await;

        assert_both_merged(&read(&root.join(FABRIC_DEPS)).await.unwrap());
    }

    #[tokio::test]
    async fn an_owner_write_keeps_a_pack_owned_file() {
        let root = polyio::testing::ScratchDir::new("owner-after-pack");
        let pack = deps_bundle(&root, "pack.mrpack", PACK_DEPS).await;
        sync_pack(&pack, root.path()).await;
        let owner = deps_bundle(&root, "owner.mrpack", OWNER_DEPS).await;
        sync(&owner, root.path()).await;

        assert_both_merged(&read(&root.join(FABRIC_DEPS)).await.unwrap());
        let lock = OverrideLock::load(root.path()).await;
        assert!(!lock.bundles[PACK_KEY].contains_key(FABRIC_DEPS));
    }

    #[tokio::test]
    async fn a_leftover_part_switches_back_to_owning_the_file() {
        let root = polyio::testing::ScratchDir::new("leftover-part");
        add_merge_part(root.path(), FABRIC_DEPS, PACK_KEY, PACK_DEPS).await;
        assert_eq!(read(&root.join(FABRIC_DEPS)).await.unwrap(), PACK_DEPS);

        let pack = deps_bundle(&root, "pack.mrpack", PACK_DEPS).await;
        sync_pack(&pack, root.path()).await;

        assert_eq!(read(&root.join(FABRIC_DEPS)).await.unwrap(), PACK_DEPS);
        assert!(
            read(&merge_part_path(root.path(), FABRIC_DEPS, PACK_KEY))
                .await
                .is_none()
        );
        let lock = OverrideLock::load(root.path()).await;
        assert!(lock.bundles[PACK_KEY].contains_key(FABRIC_DEPS));
    }

    const ONECLIENT_PACK_DEPS: &str = "{\n    \"version\": 1,\n    \"overrides\": {\n        \"flashback\": {\n            \"+breaks\": {\n                \"skyblock-item-list\": \"<0.0.23\"\n            }\n        },\n        \"idontwannascrollagain\": {\n            \"-depends\": {\n                \"minecraft\": \"IGNORED\"\n            }\n        },\n        \"nbtac\": {\n            \"+breaks\": {\n                \"nbt_ac\": \"*\"\n            }\n        }\n    }\n}";
    const QOL_BUNDLE_DEPS: &str = "{\n    \"version\": 1,\n    \"overrides\": {\n        \"flashback\": {\n            \"+breaks\": {\n                \"skyblock-item-list\": \"<0.0.23\"\n            }\n        },\n        \"idontwannascrollagain\": {\n            \"-depends\": {\n                \"minecraft\": \"IGNORED\"\n            }\n        },\n        \"nbtac\": {\n            \"+breaks\": {\n                \"nbt_ac\": \"*\"\n            }\n        }\n    }\n}\n";
    const SKYBLOCK_BUNDLE_DEPS: &str = "\n{\n    \"version\": 1,\n    \"overrides\": {\n        \"flashback\": {\n            \"+breaks\": {\n                \"skyblock-item-list\": \"<0.0.23\"\n            }\n        }\n    }\n}\n\n";
    const DISTANT_HORIZONS_PACK_DEPS: &str = "{\r\n  \"version\": 1,\r\n  \"overrides\": {\r\n\r\n  }\r\n}\r\n";
    const OTHER_PACK_KEY: &str = "modpack:curseforge:123";
    const FABRIC_KINDS: [&str; 5] = ["depends", "recommends", "suggests", "conflicts", "breaks"];

    fn assert_fabric_accepts(bytes: &[u8]) {
        let text = std::str::from_utf8(bytes).expect("utf-8");
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let body = text.trim_start();
        assert!(body.starts_with('{'), "root must be an object: {text}");
        assert!(
            body[1..].trim_start().starts_with("\"version\""),
            "first key must be version: {text}"
        );
        let value: serde_json::Value = serde_json::from_str(text).expect("valid json");
        let root = value.as_object().expect("object root");
        assert_eq!(root["version"].as_f64(), Some(1.0), "{text}");
        for key in root.keys() {
            assert!(key == "version" || key == "overrides", "root key {key}: {text}");
        }
        let Some(overrides) = root.get("overrides") else {
            return;
        };
        for (mod_id, container) in overrides.as_object().expect("overrides object") {
            let container = container
                .as_object()
                .unwrap_or_else(|| panic!("{mod_id} must be an object"));
            for (key, deps) in container {
                let kind = key
                    .strip_prefix('+')
                    .or_else(|| key.strip_prefix('-'))
                    .unwrap_or(key);
                assert!(FABRIC_KINDS.contains(&kind), "{mod_id}: {key}");
                for (dep, range) in deps.as_object().expect("dependency object") {
                    let ok = range.is_string()
                        || range
                            .as_array()
                            .is_some_and(|ranges| ranges.iter().all(serde_json::Value::is_string));
                    assert!(ok, "{mod_id}.{key}.{dep}");
                }
            }
        }
    }

    fn override_mods(bytes: &[u8]) -> std::collections::BTreeSet<String> {
        let value = parse_json(bytes).expect("json");
        value["overrides"]
            .as_object()
            .map(|mods| mods.keys().cloned().collect())
            .unwrap_or_default()
    }

    async fn owner_sync(root: &polyio::testing::ScratchDir, bundle: &str, bytes: Option<&[u8]>) {
        let zip = root.join(format!("{bundle}.mrpack"));
        match bytes {
            Some(bytes) => {
                write_bundle(&zip, &[(&format!("overrides/{FABRIC_DEPS}"), bytes)]).await
            }
            None => write_bundle(&zip, &[("overrides/config/other.toml", b"other")]).await,
        }
        sync_bundle_overrides_at(&zip, bundle, root.path(), None)
            .await
            .unwrap();
    }

    async fn pack_sync(root: &polyio::testing::ScratchDir, key: &str, bytes: Option<&[u8]>) {
        let zip = root.join(format!("{}.mrpack", key.replace(':', "_")));
        match bytes {
            Some(bytes) => {
                write_bundle(&zip, &[(&format!("overrides/{FABRIC_DEPS}"), bytes)]).await
            }
            None => write_bundle(&zip, &[("overrides/config/other.toml", b"other")]).await,
        }
        sync_layered_overrides(&zip, key, root.path(), &[OVERRIDES_PREFIX], &|_| true, false, None)
            .await
            .unwrap();
    }

    async fn pack_removed(root: &polyio::testing::ScratchDir, key: &str) {
        let none = HashSet::new();
        sync_file_lock(root.path(), key, HashMap::new(), &none).await;
        prune_merge_parts(root.path(), key, &none).await;
    }

    async fn deps_on_disk(root: &polyio::testing::ScratchDir) -> Option<Vec<u8>> {
        read(&root.join(FABRIC_DEPS)).await
    }

    #[tokio::test]
    async fn scenario_real_oneclient_pack_alone_is_untouched() {
        let root = polyio::testing::ScratchDir::new("s-oneclient-alone");
        pack_sync(&root, PACK_KEY, Some(ONECLIENT_PACK_DEPS.as_bytes())).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_eq!(out, ONECLIENT_PACK_DEPS.as_bytes());
        assert_fabric_accepts(&out);
    }

    #[tokio::test]
    async fn scenario_crlf_pack_alone_is_untouched() {
        let root = polyio::testing::ScratchDir::new("s-crlf-alone");
        pack_sync(&root, PACK_KEY, Some(DISTANT_HORIZONS_PACK_DEPS.as_bytes())).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_eq!(out, DISTANT_HORIZONS_PACK_DEPS.as_bytes());
        assert_fabric_accepts(&out);
    }

    #[tokio::test]
    async fn scenario_bom_pack_alone_is_untouched() {
        let root = polyio::testing::ScratchDir::new("s-bom-alone");
        let with_bom = [b"\xef\xbb\xbf".as_slice(), PACK_DEPS].concat();
        pack_sync(&root, PACK_KEY, Some(&with_bom)).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_eq!(out, with_bom);
        assert_fabric_accepts(&out);
    }

    #[tokio::test]
    async fn scenario_bundle_then_pack() {
        let root = polyio::testing::ScratchDir::new("s-bundle-pack");
        owner_sync(&root, "qol", Some(QOL_BUNDLE_DEPS.as_bytes())).await;
        pack_sync(&root, PACK_KEY, Some(PACK_DEPS)).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_fabric_accepts(&out);
        let mods = override_mods(&out);
        assert!(mods.contains("pack-mod") && mods.contains("flashback") && mods.contains("nbtac"));
    }

    #[tokio::test]
    async fn scenario_pack_then_bundle() {
        let root = polyio::testing::ScratchDir::new("s-pack-bundle");
        pack_sync(&root, PACK_KEY, Some(PACK_DEPS)).await;
        owner_sync(&root, "qol", Some(QOL_BUNDLE_DEPS.as_bytes())).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_fabric_accepts(&out);
        let mods = override_mods(&out);
        assert!(mods.contains("pack-mod") && mods.contains("idontwannascrollagain"));
    }

    #[tokio::test]
    async fn scenario_two_bundles_then_the_real_pack() {
        let root = polyio::testing::ScratchDir::new("s-two-bundles");
        owner_sync(&root, "qol", Some(QOL_BUNDLE_DEPS.as_bytes())).await;
        owner_sync(&root, "skyblock", Some(SKYBLOCK_BUNDLE_DEPS.as_bytes())).await;
        assert_fabric_accepts(&deps_on_disk(&root).await.unwrap());
        pack_sync(&root, PACK_KEY, Some(ONECLIENT_PACK_DEPS.as_bytes())).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_fabric_accepts(&out);
        assert!(override_mods(&out).contains("nbtac"));
    }

    #[tokio::test]
    async fn scenario_two_packs_without_a_bundle() {
        let root = polyio::testing::ScratchDir::new("s-two-packs");
        pack_sync(&root, PACK_KEY, Some(ONECLIENT_PACK_DEPS.as_bytes())).await;
        pack_sync(&root, OTHER_PACK_KEY, Some(DISTANT_HORIZONS_PACK_DEPS.as_bytes())).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_fabric_accepts(&out);
        assert!(override_mods(&out).contains("flashback"));

        pack_removed(&root, OTHER_PACK_KEY).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_eq!(out, ONECLIENT_PACK_DEPS.as_bytes());
        pack_removed(&root, PACK_KEY).await;
        assert!(deps_on_disk(&root).await.is_none());
    }

    #[tokio::test]
    async fn scenario_bundle_and_two_packs_then_both_removed() {
        let root = polyio::testing::ScratchDir::new("s-bundle-two-packs");
        owner_sync(&root, "qol", Some(QOL_BUNDLE_DEPS.as_bytes())).await;
        pack_sync(&root, PACK_KEY, Some(PACK_DEPS)).await;
        pack_sync(&root, OTHER_PACK_KEY, Some(DISTANT_HORIZONS_PACK_DEPS.as_bytes())).await;
        assert_fabric_accepts(&deps_on_disk(&root).await.unwrap());

        pack_removed(&root, PACK_KEY).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_fabric_accepts(&out);
        assert!(!override_mods(&out).contains("pack-mod"));

        pack_removed(&root, OTHER_PACK_KEY).await;
        assert_eq!(deps_on_disk(&root).await.unwrap(), QOL_BUNDLE_DEPS.as_bytes());
    }

    #[tokio::test]
    async fn scenario_bundle_drops_the_file_while_a_pack_needs_it() {
        let root = polyio::testing::ScratchDir::new("s-bundle-drops");
        owner_sync(&root, "qol", Some(QOL_BUNDLE_DEPS.as_bytes())).await;
        pack_sync(&root, PACK_KEY, Some(PACK_DEPS)).await;
        owner_sync(&root, "qol", None).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_eq!(out, PACK_DEPS);
        assert_fabric_accepts(&out);
    }

    #[tokio::test]
    async fn scenario_pack_update_changes_and_then_drops_its_file() {
        let root = polyio::testing::ScratchDir::new("s-pack-update");
        pack_sync(&root, PACK_KEY, Some(PACK_DEPS)).await;
        pack_sync(&root, PACK_KEY, Some(ONECLIENT_PACK_DEPS.as_bytes())).await;
        assert_eq!(deps_on_disk(&root).await.unwrap(), ONECLIENT_PACK_DEPS.as_bytes());
        pack_sync(&root, PACK_KEY, None).await;
        assert!(deps_on_disk(&root).await.is_none());
    }

    #[tokio::test]
    async fn scenario_bom_on_both_sides_still_merges() {
        let root = polyio::testing::ScratchDir::new("s-bom-both");
        let bom = b"\xef\xbb\xbf".as_slice();
        owner_sync(&root, "qol", Some(&[bom, QOL_BUNDLE_DEPS.as_bytes()].concat())).await;
        pack_sync(&root, PACK_KEY, Some(&[bom, PACK_DEPS].concat())).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_fabric_accepts(&out);
        let mods = override_mods(&out);
        assert!(mods.contains("pack-mod") && mods.contains("flashback"));
    }

    #[tokio::test]
    async fn scenario_broken_inputs_never_produce_a_broken_merge() {
        let root = polyio::testing::ScratchDir::new("s-broken-inputs");
        owner_sync(&root, "qol", Some(b"{ not json")).await;
        let bad_pack = br#"{"overrides":{"good":{"+depends":{"a":"*"},"+requires":{"b":"*"},"breaks":{"c":5,"d":["1.0",2],"e":["<2"]}},"bad":"nope"},"extra":true}"#;
        pack_sync(&root, PACK_KEY, Some(bad_pack)).await;
        pack_sync(&root, OTHER_PACK_KEY, Some(PACK_DEPS)).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_fabric_accepts(&out);
        let merged = parse_json(&out).unwrap();
        assert_eq!(merged["overrides"]["good"]["+depends"]["a"], "*");
        assert_eq!(merged["overrides"]["good"]["breaks"]["e"][0], "<2");
        assert!(merged["overrides"]["good"].get("+requires").is_none());
        assert!(merged["overrides"].get("bad").is_none());
    }

    #[tokio::test]
    async fn scenario_user_edit_then_bundle_sync() {
        let root = polyio::testing::ScratchDir::new("s-user-edit");
        pack_sync(&root, PACK_KEY, Some(PACK_DEPS)).await;
        polyio::write(root.join(FABRIC_DEPS), br#"{"version":1,"overrides":{"mine":{"-depends":{"z":"*"}}}}"#)
            .await
            .unwrap();
        owner_sync(&root, "qol", Some(QOL_BUNDLE_DEPS.as_bytes())).await;
        assert_fabric_accepts(&deps_on_disk(&root).await.unwrap());
        pack_sync(&root, PACK_KEY, Some(PACK_DEPS)).await;
        let out = deps_on_disk(&root).await.unwrap();
        assert_fabric_accepts(&out);
        assert!(override_mods(&out).contains("pack-mod"));
    }

    #[tokio::test]
    async fn always_update_overwrites_user_edit() {
        let root = polyio::testing::ScratchDir::new("always");
        let path = "config/fabric_loader_dependencies.json";
        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[(&format!("overrides/{path}"), b"deps-v1")]).await;
        sync(&v1, root.path()).await;

        polyio::write(root.join(path), b"runtime-mutated")
            .await
            .unwrap();

        let v2 = root.join("v2.mrpack");
        write_bundle(&v2, &[(&format!("overrides/{path}"), b"deps-v2")]).await;
        let report = sync(&v2, root.path()).await;

        assert!(report.conflicts.is_empty());
        assert_eq!(read(&root.join(path)).await.unwrap(), b"deps-v2");
    }

    #[tokio::test]
    async fn removed_file_deleted_only_when_unmodified() {
        let root = polyio::testing::ScratchDir::new("removed");
        let v1 = root.join("v1.mrpack");
        write_bundle(
            &v1,
            &[
                ("overrides/config/gone.toml", b"g"),
                ("overrides/config/kept.toml", b"k"),
            ],
        )
        .await;
        sync(&v1, root.path()).await;

        polyio::write(root.join("config/kept.toml"), b"edited")
            .await
            .unwrap();

        let v2 = root.join("v2.mrpack");
        write_bundle(&v2, &[("overrides/config/a.toml", b"a")]).await;
        let report = sync(&v2, root.path()).await;

        assert_eq!(report.deleted, vec!["config/gone.toml".to_string()]);
        assert!(read(&root.join("config/gone.toml")).await.is_none());

        assert_eq!(
            read(&root.join("config/kept.toml")).await.unwrap(),
            b"edited"
        );
    }

    #[tokio::test]
    async fn bootstrap_pre_existing_edit_is_never_overwritten() {
        let root = polyio::testing::ScratchDir::new("bootstrap");

        polyio::create_dir_all(root.join("config")).await.unwrap();
        polyio::write(root.join("config/a.toml"), b"pre-existing")
            .await
            .unwrap();

        let v1 = root.join("v1.mrpack");
        write_bundle(&v1, &[("overrides/config/a.toml", b"bundle-version")]).await;
        let report = sync(&v1, root.path()).await;

        assert!(report.written.is_empty());
        assert!(report.conflicts.is_empty());
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"pre-existing"
        );

        let report2 = sync(&v1, root.path()).await;
        assert!(report2.written.is_empty());
        assert_eq!(
            read(&root.join("config/a.toml")).await.unwrap(),
            b"pre-existing"
        );
    }
}
