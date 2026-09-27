use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use oneclient_common::paths;
use oneclient_net::{EtagPolicy, fetch_cached};

use super::{FileIdentity, ProviderId};
use crate::ctx::ContentCtx;
use crate::error::ContentResult;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BadModList {
    #[serde(rename = "bad-mods", default)]
    pub bad_mods: Vec<BadMod>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BadMod {
    pub hash: String,
    #[serde(default)]
    pub alternatives: Vec<BadModAlternative>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BadModAlternative {
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAlternative {
    pub provider: ProviderId,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub icon_url: Option<String>,
}

impl BadModList {
    pub fn find(&self, sha1: &str) -> Option<&BadMod> {
        let sha1 = polyio::normalize_hash(sha1);
        self.bad_mods
            .iter()
            .find(|entry| polyio::normalize_hash(&entry.hash) == sha1)
    }

    pub fn is_bad(&self, sha1: &str) -> bool {
        self.find(sha1).is_some()
    }
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn fetch_bad_mods(ctx: &ContentCtx) -> ContentResult<BadModList> {
    let url = format!("{}/oneclient/bad-mods.json", ctx.net.config().meta_url_base);
    let cache_path = paths::caches_dir()?.join("bad-mods.json");

    let Some(fetched) = fetch_cached(&ctx.net, &url, &cache_path, EtagPolicy::CommitNow).await?
    else {
        return Ok(BadModList::default());
    };

    Ok(fetched.json()?)
}

pub async fn load_bad_mods(ctx: &ContentCtx) -> BadModList {
    match fetch_bad_mods(ctx).await {
        Ok(list) => list,
        Err(err) => {
            tracing::warn!(%err, "bad mods list unavailable, installs are not screened");
            BadModList::default()
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn resolve_alternatives(entry: &BadMod, ctx: &ContentCtx) -> Vec<ResolvedAlternative> {
    let mut seen = HashSet::new();
    let identities: Vec<FileIdentity> = entry
        .alternatives
        .iter()
        .map(|alt| FileIdentity::from_sha1(&alt.hash))
        .filter(|identity| seen.insert(identity.sha1.clone()))
        .collect();

    if identities.is_empty() {
        return Vec::new();
    }

    let provider = match ctx.providers.get(ProviderId::Modrinth) {
        Ok(provider) => provider,
        Err(err) => {
            tracing::warn!(%err, "cannot resolve bad mod alternatives");
            return Vec::new();
        }
    };

    let versions = match provider.lookup_versions(&identities, ctx).await {
        Ok(versions) => versions,
        Err(err) => {
            tracing::warn!(%err, "failed to look up bad mod alternatives");
            return Vec::new();
        }
    };

    let project_ids: Vec<String> = versions
        .values()
        .map(|version| version.project_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let projects: HashMap<String, (String, Option<String>)> =
        match provider.get_projects(&project_ids, ctx).await {
            Ok(projects) => projects
                .into_iter()
                .map(|project| (project.id, (project.name, project.icon_url)))
                .collect(),
            Err(err) => {
                tracing::warn!(%err, "failed to fetch bad mod alternative projects");
                HashMap::new()
            }
        };

    identities
        .iter()
        .filter_map(|identity| {
            let Some(version) = versions.get(&identity.sha1) else {
                tracing::warn!(hash = %identity.sha1, "bad mod alternative not found on Modrinth");
                return None;
            };
            let (name, icon_url) = projects
                .get(&version.project_id)
                .cloned()
                .unwrap_or_else(|| (version.name.clone(), None));
            Some(ResolvedAlternative {
                provider: ProviderId::Modrinth,
                project_id: version.project_id.clone(),
                version_id: version.version_id.clone(),
                name,
                version_number: version.version_number.clone(),
                icon_url,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_list_with_alternatives() {
        let raw = br#"{"bad-mods":[{"hash":"aaa","alternatives":[{"hash":"bbb"}]},{"hash":"ccc","alternatives":[]}]}"#;
        let list: BadModList = serde_json::from_slice(raw).unwrap();

        assert_eq!(list.bad_mods.len(), 2);
        assert_eq!(list.bad_mods[0].alternatives[0].hash, "bbb");
        assert!(list.bad_mods[1].alternatives.is_empty());
    }

    #[test]
    fn missing_fields_default_to_empty() {
        let list: BadModList = serde_json::from_slice(br#"{}"#).unwrap();
        assert!(list.bad_mods.is_empty());

        let list: BadModList = serde_json::from_slice(br#"{"bad-mods":[{"hash":"aaa"}]}"#).unwrap();
        assert!(list.bad_mods[0].alternatives.is_empty());
    }

    #[test]
    fn find_ignores_case_and_whitespace() {
        let list: BadModList =
            serde_json::from_slice(br#"{"bad-mods":[{"hash":" ABCdef "}]}"#).unwrap();

        assert!(list.is_bad("abcdef"));
        assert!(list.is_bad("ABCDEF"));
        assert!(!list.is_bad("abcde0"));
    }
}
