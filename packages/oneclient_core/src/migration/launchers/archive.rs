use std::path::{Path, PathBuf};

use oneclient_content::ContentError;

use super::ExternalInstance;
use super::prism;
use crate::LauncherResult;

const INSTANCE_CONFIG: &str = "instance.cfg";
const WORKDIR_PREFIX: &str = "oneclient-instance-";
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(60 * 60);
const SPACE_MARGIN: u64 = 512 * 1024 * 1024;
const PACK_CONFIG: &str = "mmc-pack.json";

static UNPACKING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn normalized(name: &str) -> String {
    name.replace('\\', "/")
}

fn belongs_to(prefixes: &[String], name: &str) -> bool {
    let name = normalized(name);
    prefixes
        .iter()
        .any(|prefix| name.starts_with(prefix.as_str()))
}

async fn refuse_unsupported(path: &Path, prefixes: &[String]) -> LauncherResult<()> {
    let configs = polyio::read_zip_file_entries(path, |name| {
        let name = normalized(name);
        prefixes.iter().any(|prefix| {
            name == format!("{prefix}{PACK_CONFIG}")
                || (name.starts_with(&format!("{prefix}patches/")) && name.ends_with(".json"))
        })
    })
    .await?;

    let mut problems: Vec<String> = Vec::new();
    for prefix in prefixes {
        let pack_name = format!("{prefix}{PACK_CONFIG}");
        let patch_dir = format!("{prefix}patches/");
        let Some((_, pack)) = configs
            .iter()
            .find(|(name, _)| normalized(name) == pack_name)
        else {
            return Ok(());
        };
        let patches: Vec<&[u8]> = configs
            .iter()
            .filter(|(name, _)| normalized(name).starts_with(&patch_dir))
            .map(|(_, bytes)| bytes.as_slice())
            .collect();
        match prism::archived_loader_problem(pack, &patches) {
            Some(problem) => problems.push(problem),
            None => return Ok(()),
        }
    }

    problems.sort();
    problems.dedup();
    Err(ContentError::InvalidData {
        reason: format!(
            "This .zip only holds instances that use {}, which OneClient can't run.",
            problems.join(" or ")
        ),
    }
    .into())
}

#[derive(Debug, Clone)]
pub struct InstanceArchive {
    pub instances: Vec<ExternalInstance>,
    pub workdir: PathBuf,
}

fn instance_prefixes(names: &[String]) -> Vec<String> {
    let names: Vec<String> = names.iter().map(|name| name.replace('\\', "/")).collect();
    let mut found: Vec<(usize, String)> = names
        .iter()
        .filter_map(|name| {
            let prefix = name.strip_suffix(INSTANCE_CONFIG)?.to_string();
            let depth = prefix.matches('/').count();
            ((prefix.is_empty() || prefix.ends_with('/')) && depth <= 1).then_some((depth, prefix))
        })
        .filter(|(_, prefix)| names.contains(&format!("{prefix}{PACK_CONFIG}")))
        .collect();
    let shallowest = found.iter().map(|(depth, _)| *depth).min();
    found.retain(|(depth, _)| Some(*depth) == shallowest);
    let mut prefixes: Vec<String> = found.into_iter().map(|(_, prefix)| prefix).collect();
    prefixes.sort();
    prefixes.dedup();
    prefixes
}

#[tracing::instrument(level = "debug")]
pub async fn is_instance_archive(path: &Path) -> bool {
    match polyio::zip_entry_names(path).await {
        Ok(names) => !instance_prefixes(&names).is_empty(),
        Err(_) => false,
    }
}

fn space_problem(unpacked: u64, staging: &Path) -> Option<String> {
    let need = unpacked.saturating_mul(2).saturating_add(SPACE_MARGIN);
    let available = crate::settings::data_dir::available_space(staging)?;
    (need > available).then(|| {
        format!(
            "There isn't enough free space to import this. It needs about {} MB on the drive with {}.",
            need / (1024 * 1024) + 1,
            staging.display()
        )
    })
}

#[tracing::instrument]
pub async fn open_instance_archive(path: &Path) -> LauncherResult<InstanceArchive> {
    let names = polyio::zip_entry_names(path).await?;
    let prefixes = instance_prefixes(&names);
    if prefixes.is_empty() {
        return Err(not_an_instance());
    }
    refuse_unsupported(path, &prefixes).await?;

    let _unpacking = UNPACKING.lock().await;
    let unpacked = polyio::zip_uncompressed_size(path, |name| belongs_to(&prefixes, name)).await?;
    let staging = oneclient_common::paths::import_staging_dir()?;
    polyio::create_dir_all(&staging).await?;
    if let Some(reason) = space_problem(unpacked, &staging) {
        return Err(ContentError::InvalidData { reason }.into());
    }

    let workdir = staging.join(format!("{WORKDIR_PREFIX}{}", uuid::Uuid::new_v4()));
    let opened = async {
        let wanted = prefixes.clone();
        polyio::extract_zip_limited(path, &workdir, unpacked, move |name| {
            belongs_to(&wanted, name)
        })
        .await?;
        let mut instances = Vec::new();
        for prefix in &prefixes {
            let dir = workdir.join(polyio::sanitize_path(prefix.trim_end_matches('/')));
            match prism::read_exported(&dir).await {
                Ok(Some(mut instance)) => {
                    instance.settings.drop_commands();
                    instance.played_secs = 0;
                    instances.push(instance);
                }
                Ok(None) => {
                    tracing::info!(prefix, "skipping an instance in the archive that can't be read");
                }
                Err(err) => {
                    tracing::warn!(prefix, error = %err, "skipping an unreadable instance in the archive");
                }
            }
        }
        if instances.is_empty() {
            return Err(not_an_instance());
        }
        LauncherResult::Ok(instances)
    }
    .await;

    match opened {
        Ok(instances) => Ok(InstanceArchive { instances, workdir }),
        Err(err) => {
            discard_instance_archive(&workdir).await;
            Err(err)
        }
    }
}

pub async fn sweep_instance_archives() {
    let Ok(staging) = oneclient_common::paths::import_staging_dir() else {
        return;
    };
    let Ok(mut entries) = polyio::read_dir(staging).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let stale = entry
            .file_name()
            .to_string_lossy()
            .starts_with(WORKDIR_PREFIX)
            && entry
                .metadata()
                .await
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age > STALE_AFTER);
        if stale {
            tracing::info!(dir = %entry.path().display(), "removing an instance left unpacked by an earlier run");
            discard_instance_archive(&entry.path()).await;
        }
    }
}

pub async fn discard_instance_archive(workdir: &Path) {
    if let Err(err) = polyio::remove_dir_all(workdir).await {
        tracing::debug!(dir = %workdir.display(), error = %err, "could not remove an unpacked instance");
    }
}

fn not_an_instance() -> crate::LauncherError {
    ContentError::InvalidData {
        reason: "This .zip isn't an instance exported from Prism Launcher or MultiMC.".to_string(),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn exports_are_recognised_at_the_top_or_one_folder_down() {
        assert_eq!(
            instance_prefixes(&names(&[
                "instance.cfg",
                "mmc-pack.json",
                ".minecraft/options.txt"
            ])),
            vec![String::new()]
        );
        assert_eq!(
            instance_prefixes(&names(&["Pack/instance.cfg", "Pack/mmc-pack.json"])),
            vec!["Pack/".to_string()]
        );
        assert_eq!(
            instance_prefixes(&names(&[
                "B/instance.cfg",
                "B/mmc-pack.json",
                "A/instance.cfg",
                "A/mmc-pack.json",
                "C/instance.cfg"
            ])),
            vec!["A/".to_string(), "B/".to_string()]
        );
        assert!(instance_prefixes(&names(&["instance.cfg"])).is_empty());
        assert!(instance_prefixes(&names(&["a/b/instance.cfg", "a/b/mmc-pack.json"])).is_empty());
        assert!(
            instance_prefixes(&names(&["modrinth.index.json", "overrides/config/x.json"]))
                .is_empty()
        );
    }
}
