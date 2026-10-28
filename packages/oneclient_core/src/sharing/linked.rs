use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use oneclient_content::bundles::remove_artifact_from_cluster;
use oneclient_content::modpacks::{self, SHARE_EXTENSION};
use oneclient_content::packages::PackageStore;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use serde::{Deserialize, Serialize};

use super::code::ShareCode;
use super::store::{RemoteShare, ShareContents, ShareRole, ShareStore, SharingError};
use super::{check_meta, verify_payload};
use crate::LauncherResult;
use crate::state::LauncherState;
use crate::sync::{ClearContent, apply_existing};

const MAX_NAMED: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedShare {
    pub cluster_id: i64,
    pub code: ShareCode,
    pub revision: u64,
    pub mc_version: String,
    #[serde(default)]
    pub owned: Vec<String>,
    #[serde(default)]
    pub settings: Vec<String>,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub setting_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub role: ShareRole,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShareChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub settings: usize,
    pub version: Option<(String, String)>,
}

impl ShareChanges {
    #[must_use]
    pub fn between(link: &LinkedShare, remote: &RemoteShare) -> Self {
        let mut changes = Self::default();
        if link.mc_version != remote.meta.mc_version {
            changes.version = Some((link.mc_version.clone(), remote.meta.mc_version.clone()));
        }
        let Some(contents) = &remote.contents else {
            return changes;
        };

        let incoming: BTreeMap<&str, &str> = contents
            .files
            .iter()
            .map(|file| (file.path.as_str(), file.hash.as_str()))
            .collect();
        for (path, hash) in &incoming {
            match link.files.get(*path) {
                None => changes.added.push(display_name(path)),
                Some(old) if old != hash => changes.changed.push(display_name(path)),
                Some(_) => {}
            }
        }
        for path in link.files.keys() {
            if !incoming.contains_key(path.as_str()) {
                changes.removed.push(display_name(path));
            }
        }

        let settings: BTreeMap<&str, &str> = contents
            .settings
            .iter()
            .map(|file| (file.path.as_str(), file.hash.as_str()))
            .collect();
        changes.settings = settings
            .iter()
            .filter(|(path, hash)| {
                link.setting_hashes.get(**path).map(String::as_str) != Some(**hash)
            })
            .count()
            + link
                .setting_hashes
                .keys()
                .filter(|path| !settings.contains_key(path.as_str()))
                .count();
        changes
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.settings == 0
            && self.version.is_none()
    }

    #[must_use]
    pub fn describe(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some((from, to)) = &self.version {
            lines.push(format!("Moves from Minecraft {from} to {to}."));
        }
        for (label, names) in [
            ("Adds", &self.added),
            ("Updates", &self.changed),
            ("Removes", &self.removed),
        ] {
            if !names.is_empty() {
                lines.push(format!("{label} {}.", named(names)));
            }
        }
        match self.settings {
            0 => {}
            1 => lines.push("Changes 1 settings file.".to_string()),
            n => lines.push(format!("Changes {n} settings files.")),
        }
        lines
    }
}

fn display_name(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".jar")
        .or_else(|| name.strip_suffix(".zip"))
        .unwrap_or(name)
        .to_string()
}

fn named(names: &[String]) -> String {
    let shown: Vec<&str> = names.iter().take(MAX_NAMED).map(String::as_str).collect();
    match names.len().saturating_sub(MAX_NAMED) {
        0 => shown.join(", "),
        extra => format!("{} and {extra} more", shown.join(", ")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareUpdate {
    Current,
    Available {
        link: Box<LinkedShare>,
        remote: Box<RemoteShare>,
        changes: ShareChanges,
    },
    Ended {
        link: LinkedShare,
    },
}

#[derive(Debug, Clone)]
pub struct LinkedLedger {
    path: PathBuf,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl LinkedLedger {
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::default(),
        }
    }

    async fn load(&self) -> Result<Vec<LinkedShare>, SharingError> {
        if !polyio::try_exists(&self.path).await? {
            return Ok(Vec::new());
        }
        let bytes = polyio::read(&self.path).await?;
        match serde_json::from_slice(&bytes) {
            Ok(entries) => Ok(entries),
            Err(err) => {
                tracing::warn!(path = %self.path.display(), error = %err, "the linked share list is unreadable; starting over");
                Ok(Vec::new())
            }
        }
    }

    async fn save(&self, entries: &[LinkedShare]) -> Result<(), SharingError> {
        if let Some(parent) = self.path.parent() {
            polyio::create_dir_all(parent).await?;
        }
        let staging = polyio::temp_sibling(&self.path);
        polyio::write(&staging, serde_json::to_vec_pretty(entries)?).await?;
        if let Err(err) = polyio::rename(&staging, &self.path).await {
            polyio::remove_file(&staging).await.ok();
            return Err(err.into());
        }
        Ok(())
    }

    #[must_use]
    pub fn owned_shares(&self) -> super::ShareLedger {
        super::ShareLedger::at(self.path.with_file_name(super::PUBLISHED_FILE))
    }

    pub async fn find(&self, cluster_id: i64) -> Result<Option<LinkedShare>, SharingError> {
        let _guard = self.lock.lock().await;
        Ok(self
            .load()
            .await?
            .into_iter()
            .find(|entry| entry.cluster_id == cluster_id))
    }

    pub async fn upsert(&self, link: LinkedShare) -> Result<(), SharingError> {
        let _guard = self.lock.lock().await;
        let mut entries = self.load().await?;
        entries.retain(|entry| entry.cluster_id != link.cluster_id);
        entries.push(link);
        self.save(&entries).await
    }

    pub async fn remove(&self, cluster_id: i64) -> Result<bool, SharingError> {
        let _guard = self.lock.lock().await;
        let mut entries = self.load().await?;
        let before = entries.len();
        entries.retain(|entry| entry.cluster_id != cluster_id);
        if entries.len() == before {
            return Ok(false);
        }
        self.save(&entries).await?;
        Ok(true)
    }
}

pub fn default_linked_ledger() -> LauncherResult<LinkedLedger> {
    Ok(LinkedLedger::at(
        super::sharing_dir()?.join(super::LINKED_FILE),
    ))
}

async fn owned_artifacts(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    excluded: &HashSet<String>,
) -> LauncherResult<Vec<String>> {
    let content = state.services.content();
    let mut owned = Vec::new();
    for link in PackageStore::list_linked_artifacts(cluster_id, &content).await? {
        if excluded.contains(&link.hash)
            || bundle_dao::get_bundle_tracked(&state.services.db, cluster_id, &link.hash)
                .await?
                .is_some()
        {
            continue;
        }
        owned.push(link.hash);
    }
    owned.sort();
    owned.dedup();
    Ok(owned)
}

fn contents_maps(
    contents: Option<&ShareContents>,
) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let Some(contents) = contents else {
        return Default::default();
    };
    let map = |files: &[super::store::ContentEntry]| {
        files
            .iter()
            .map(|file| (file.path.clone(), file.hash.clone()))
            .collect()
    };
    (map(&contents.files), map(&contents.settings))
}

async fn record_link(
    state: &Arc<LauncherState>,
    ledger: &LinkedLedger,
    cluster_id: i64,
    code: &ShareCode,
    remote: &RemoteShare,
    payload: &Path,
    user_added: &HashSet<String>,
) -> LauncherResult<LinkedShare> {
    let settings = modpacks::read_cluster_share(payload)
        .await?
        .map(|share| share.settings)
        .unwrap_or_default();
    let (files, setting_hashes) = contents_maps(remote.contents.as_ref());
    let link = LinkedShare {
        cluster_id,
        code: code.clone(),
        revision: remote.revision,
        mc_version: remote.meta.mc_version.clone(),
        owned: owned_artifacts(state, cluster_id, user_added).await?,
        settings,
        files,
        setting_hashes,
        updated_at: Some(Utc::now()),
        role: remote.role,
    };
    ledger.upsert(link.clone()).await?;
    Ok(link)
}

#[tracing::instrument(skip(state, ledger, remote, payload))]
pub async fn link_import(
    state: &Arc<LauncherState>,
    ledger: &LinkedLedger,
    cluster_id: i64,
    code: &ShareCode,
    remote: &RemoteShare,
    payload: &Path,
) -> LauncherResult<LinkedShare> {
    let link = record_link(
        state,
        ledger,
        cluster_id,
        code,
        remote,
        payload,
        &HashSet::new(),
    )
    .await?;
    tracing::info!(cluster_id, code = %code, revision = link.revision, "linked an imported instance to its share code");
    Ok(link)
}

#[tracing::instrument(skip(store, ledger))]
pub async fn check_update(
    store: &dyn ShareStore,
    ledger: &LinkedLedger,
    cluster_id: i64,
) -> LauncherResult<Option<ShareUpdate>> {
    let Some(link) = ledger.find(cluster_id).await? else {
        return Ok(None);
    };
    match store.describe(&link.code).await {
        Ok(remote) if remote.revision > link.revision => {
            let changes = ShareChanges::between(&link, &remote);
            Ok(Some(ShareUpdate::Available {
                link: Box::new(link),
                remote: Box::new(remote),
                changes,
            }))
        }
        Ok(_) => Ok(Some(ShareUpdate::Current)),
        Err(SharingError::Gone(_) | SharingError::NotFound(_)) => {
            Ok(Some(ShareUpdate::Ended { link }))
        }
        Err(err) => Err(err.into()),
    }
}

#[tracing::instrument(skip(state, store, ledger))]
pub async fn apply_update(
    state: &Arc<LauncherState>,
    store: &dyn ShareStore,
    ledger: &LinkedLedger,
    cluster_id: i64,
) -> LauncherResult<i64> {
    let link = ledger.find(cluster_id).await?.ok_or_else(|| {
        SharingError::Unavailable("this instance isn't linked to a share code".into())
    })?;

    let staging = polyio::tempdir().await?;
    let payload = staging
        .dir_path()
        .join(format!("{}.{SHARE_EXTENSION}", link.code.as_str()));
    let remote = store.fetch(&link.code, &payload).await?;
    check_meta(&remote.meta)?;
    verify_payload(&payload, &remote.meta).await?;

    let owned: HashSet<String> = link.owned.iter().cloned().collect();
    let content = state.services.content();
    let before: HashSet<String> = PackageStore::list_linked_artifacts(cluster_id, &content)
        .await?
        .into_iter()
        .map(|artifact| artifact.hash)
        .collect();
    let user_added: HashSet<String> = before.difference(&owned).cloned().collect();

    for hash in before.intersection(&owned) {
        if bundle_dao::get_bundle_tracked(&state.services.db, cluster_id, hash)
            .await?
            .is_some()
        {
            continue;
        }
        remove_artifact_from_cluster(cluster_id, hash, false, &content).await?;
    }
    remove_stale_settings(state, cluster_id, &link.settings).await?;

    let applied = Box::pin(apply_existing(
        state,
        cluster_id,
        &payload,
        ClearContent::Nothing,
    ))
    .await?;
    let excluded = if applied == cluster_id {
        user_added
    } else {
        ledger.remove(cluster_id).await?;
        HashSet::new()
    };
    if applied == cluster_id {
        super::refresh_push_state(&ledger.owned_shares(), store.name(), cluster_id, &payload).await;
    }
    record_link(
        state, ledger, applied, &link.code, &remote, &payload, &excluded,
    )
    .await?;
    tracing::info!(cluster_id = applied, code = %link.code, revision = remote.revision, "updated an instance from its share code");
    Ok(applied)
}

async fn remove_stale_settings(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    settings: &[String],
) -> LauncherResult<()> {
    let cluster = state.clusters.get(cluster_id).await?;
    if settings.is_empty() || (!cluster.is_isolated() && !cluster.uses_dedicated_dir()) {
        return Ok(());
    }
    let root = cluster.game_dir()?;
    for path in settings {
        if !modpacks::is_setting_path(path) {
            continue;
        }
        let dest = root.join(polyio::sanitize_path(path));
        if polyio::try_exists(&dest).await? {
            polyio::remove_file(&dest).await?;
        }
    }
    Ok(())
}

pub async fn unlink(ledger: &LinkedLedger, cluster_id: i64) -> LauncherResult<bool> {
    Ok(ledger.remove(cluster_id).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sharing::store::{ContentEntry, ShareMeta};
    use oneclient_common::domain::GameLoader;
    use oneclient_db::models::ClusterKind;

    fn entry(path: &str, hash: &str) -> ContentEntry {
        ContentEntry {
            path: path.to_string(),
            size: 1,
            hash: hash.to_string(),
            source: Some("modrinth".to_string()),
        }
    }

    fn remote(revision: u64, files: Vec<ContentEntry>, settings: Vec<ContentEntry>) -> RemoteShare {
        RemoteShare {
            meta: ShareMeta {
                format: 1,
                name: "Pack".to_string(),
                mc_version: "1.21.1".to_string(),
                mc_loader: GameLoader::Fabric,
                kind: ClusterKind::Modded,
                size: 1,
                sha512: String::new(),
                created_at: None,
            },
            revision,
            contents: Some(ShareContents {
                files,
                settings,
                bundles: Vec::new(),
                truncated: false,
            }),
            expires_at: None,
            role: ShareRole::Viewer,
        }
    }

    #[test]
    fn changes_name_what_the_owner_did() {
        let link = LinkedShare {
            cluster_id: 1,
            code: ShareCode::generate(),
            revision: 1,
            mc_version: "1.21.1".to_string(),
            owned: Vec::new(),
            settings: Vec::new(),
            files: BTreeMap::from([
                ("mods/sodium.jar".to_string(), "a".to_string()),
                ("mods/iris.jar".to_string(), "b".to_string()),
            ]),
            setting_hashes: BTreeMap::from([("options.txt".to_string(), "x".to_string())]),
            updated_at: None,
            role: ShareRole::Viewer,
        };
        let changes = ShareChanges::between(
            &link,
            &remote(
                2,
                vec![
                    entry("mods/sodium.jar", "a2"),
                    entry("mods/lithium.jar", "c"),
                ],
                vec![entry("options.txt", "y")],
            ),
        );
        assert_eq!(changes.added, vec!["lithium"]);
        assert_eq!(changes.changed, vec!["sodium"]);
        assert_eq!(changes.removed, vec!["iris"]);
        assert_eq!(changes.settings, 1);
        assert_eq!(
            changes.describe(),
            vec![
                "Adds lithium.",
                "Updates sodium.",
                "Removes iris.",
                "Changes 1 settings file."
            ]
        );
    }
}
