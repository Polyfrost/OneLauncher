use std::path::{Path, PathBuf};

use oneclient_content::ContentError;

use super::ExternalInstance;
use super::prism;
use crate::LauncherResult;

const INSTANCE_CONFIG: &str = "instance.cfg";
const PACK_CONFIG: &str = "mmc-pack.json";

#[derive(Debug, Clone)]
pub struct InstanceArchive {
    pub instance: ExternalInstance,
    pub workdir: PathBuf,
}

fn instance_prefix(names: &[String]) -> Option<String> {
    names
        .iter()
        .map(|name| name.replace('\\', "/"))
        .filter_map(|name| {
            let prefix = name.strip_suffix(INSTANCE_CONFIG)?.to_string();
            let depth = prefix.matches('/').count();
            (prefix.is_empty() || prefix.ends_with('/')).then_some((depth, prefix))
        })
        .filter(|(depth, _)| *depth <= 1)
        .min_by_key(|(depth, _)| *depth)
        .map(|(_, prefix)| prefix)
        .filter(|prefix| {
            names
                .iter()
                .any(|name| name.replace('\\', "/") == format!("{prefix}{PACK_CONFIG}"))
        })
}

#[tracing::instrument(level = "debug")]
pub async fn is_instance_archive(path: &Path) -> bool {
    match polyio::zip_entry_names(path).await {
        Ok(names) => instance_prefix(&names).is_some(),
        Err(_) => false,
    }
}

#[tracing::instrument]
pub async fn open_instance_archive(path: &Path) -> LauncherResult<InstanceArchive> {
    let names = polyio::zip_entry_names(path).await?;
    let Some(prefix) = instance_prefix(&names) else {
        return Err(not_an_instance());
    };

    let workdir = std::env::temp_dir().join(format!("oneclient-instance-{}", uuid::Uuid::new_v4()));
    let opened = async {
        polyio::extract_zip(path, &workdir).await?;
        let dir = workdir.join(polyio::sanitize_path(prefix.trim_end_matches('/')));
        prism::read_exported(&dir)
            .await?
            .ok_or_else(not_an_instance)
    }
    .await;

    match opened {
        Ok(instance) => Ok(InstanceArchive { instance, workdir }),
        Err(err) => {
            discard_instance_archive(&workdir).await;
            Err(err)
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
            instance_prefix(&names(&[
                "instance.cfg",
                "mmc-pack.json",
                ".minecraft/options.txt"
            ])),
            Some(String::new())
        );
        assert_eq!(
            instance_prefix(&names(&["Pack/instance.cfg", "Pack/mmc-pack.json"])),
            Some("Pack/".to_string())
        );
        assert_eq!(instance_prefix(&names(&["instance.cfg"])), None);
        assert_eq!(
            instance_prefix(&names(&["a/b/instance.cfg", "a/b/mmc-pack.json"])),
            None
        );
        assert_eq!(
            instance_prefix(&names(&["modrinth.index.json", "overrides/config/x.json"])),
            None
        );
    }
}
