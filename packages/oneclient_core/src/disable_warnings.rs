use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::{LauncherError, LauncherResult};
use oneclient_common::paths;
use oneclient_net::RequestClient;
use oneclient_net::{EtagPolicy, fetch_cached};

const MESSAGES_DIR: &str = "/oneclient/bundles/disable-warnings/";

#[derive(Debug, Default, Deserialize)]
struct Manifest {
    #[serde(default)]
    mods: HashMap<String, Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisableWarnings {
    mods: HashMap<String, String>,
}

impl DisableWarnings {
    pub fn body_for(&self, project_id: &str) -> Option<&str> {
        self.mods.get(project_id).map(String::as_str)
    }
}

fn message_file_name(path: &str) -> Option<&str> {
    let name = path.strip_prefix(MESSAGES_DIR)?;
    let valid = name.ends_with(".md")
        && name.len() > ".md".len()
        && !name.contains("..")
        && !name.contains(['/', '\\', '?', '#']);
    valid.then_some(name)
}

async fn fetch_message(net: &RequestClient, path: &str) -> Option<String> {
    let Some(file_name) = message_file_name(path) else {
        tracing::warn!(path, "ignoring disable warning outside {MESSAGES_DIR}");
        return None;
    };

    let url = format!("{}{path}", net.config().meta_url_base);
    let cache_path = match paths::caches_dir() {
        Ok(dir) => dir.join("disable-warnings").join(file_name),
        Err(err) => {
            tracing::warn!(%err, "cannot resolve the cache dir for disable warnings");
            return None;
        }
    };

    match fetch_cached(net, &url, &cache_path, EtagPolicy::CommitNow).await {
        Ok(Some(fetched)) => Some(fetched.text()).filter(|body| !body.trim().is_empty()),
        Ok(None) => {
            tracing::warn!(path, "disable warning is unavailable and not cached");
            None
        }
        Err(err) => {
            tracing::warn!(path, %err, "failed to fetch disable warning");
            None
        }
    }
}

fn resolve(manifest: Manifest, bodies: &HashMap<String, String>) -> DisableWarnings {
    let mods = manifest
        .mods
        .into_iter()
        .filter_map(|(project_id, entry)| {
            bodies
                .get(&entry.message)
                .map(|body| (project_id, body.clone()))
        })
        .collect();

    DisableWarnings { mods }
}

#[tracing::instrument(level = "debug", skip(net))]
pub async fn fetch_disable_warnings(net: &RequestClient) -> LauncherResult<DisableWarnings> {
    let url = format!(
        "{}/oneclient/bundles/disable-warnings.json",
        net.config().meta_url_base
    );
    let cache_path = paths::caches_dir()?.join("disable-warnings.json");

    let fetched = fetch_cached(net, &url, &cache_path, EtagPolicy::CommitNow)
        .await?
        .ok_or_else(|| LauncherError::InvalidSettingsProfile {
            reason: "disable warnings are unavailable and not cached".to_string(),
        })?;
    let manifest: Manifest = fetched.json()?;

    let paths: HashSet<&str> = manifest
        .mods
        .values()
        .map(|entry| entry.message.as_str())
        .collect();

    let mut bodies = HashMap::new();
    for path in paths {
        if let Some(body) = fetch_message(net, path).await {
            bodies.insert(path.to_string(), body);
        }
    }

    Ok(resolve(manifest, &bodies))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Manifest {
        serde_json::from_str(raw).unwrap()
    }

    fn bodies(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(path, body)| (path.to_string(), body.to_string()))
            .collect()
    }

    #[test]
    fn only_listed_mods_get_a_warning() {
        let manifest = parse(
            r#"{"mods":{"Iw9mZi4a":{"message":"/oneclient/bundles/disable-warnings/polyplus.md"}}}"#,
        );
        let warnings = resolve(
            manifest,
            &bodies(&[("/oneclient/bundles/disable-warnings/polyplus.md", "Poly+")]),
        );

        assert_eq!(warnings.body_for("Iw9mZi4a"), Some("Poly+"));
        assert_eq!(warnings.body_for("AANobbMI"), None);
    }

    #[test]
    fn unfetched_message_means_no_warning() {
        let manifest = parse(
            r#"{"mods":{"Iw9mZi4a":{"message":"/oneclient/bundles/disable-warnings/polyplus.md"}}}"#,
        );
        let warnings = resolve(manifest, &HashMap::new());

        assert_eq!(warnings.body_for("Iw9mZi4a"), None);
    }

    #[test]
    fn empty_manifest_means_no_warnings() {
        let warnings = resolve(parse("{}"), &HashMap::new());

        assert_eq!(warnings.body_for("Iw9mZi4a"), None);
    }

    #[test]
    fn message_paths_are_confined_to_the_warnings_dir() {
        assert_eq!(
            message_file_name("/oneclient/bundles/disable-warnings/polyplus.md"),
            Some("polyplus.md")
        );
        assert_eq!(
            message_file_name("/oneclient/bundles/disable-warnings/../x.md"),
            None
        );
        assert_eq!(
            message_file_name("/oneclient/bundles/disable-warnings/a/b.md"),
            None
        );
        assert_eq!(message_file_name("/oneclient/tos.json"), None);
        assert_eq!(
            message_file_name("/oneclient/bundles/disable-warnings/.md"),
            None
        );
    }
}
