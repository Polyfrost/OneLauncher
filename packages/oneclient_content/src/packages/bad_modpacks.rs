use std::sync::{Arc, PoisonError, RwLock};

use serde::Deserialize;

use super::bad_mods::{
    ProjectIds, fetch_flag_list, fetch_md_file, lenient_list, md_file_name, non_blank, same_text,
};
use super::{ProjectDetail, ProviderId};
use crate::ctx::ContentCtx;
use crate::error::ContentResult;

const EXPLANATIONS_DIR: &str = "/oneclient/bad_modpacks_mds/";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BadModpackList {
    #[serde(rename = "bad-modpacks", default, deserialize_with = "lenient_list")]
    pub bad_modpacks: Vec<BadModpack>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BadModpack {
    #[serde(default, deserialize_with = "non_blank")]
    pub hash: Option<String>,
    #[serde(default)]
    pub project_ids: ProjectIds,
    #[serde(default, deserialize_with = "non_blank")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "non_blank")]
    pub author: Option<String>,
    #[serde(default, deserialize_with = "non_blank")]
    pub explanation: Option<String>,
}

impl BadModpack {
    fn is_name_only(&self) -> bool {
        self.hash.is_none()
            && self.project_ids.is_empty()
            && self.name.is_some()
            && self.author.is_some()
    }
}

impl BadModpackList {
    pub fn find(&self, sha1: &str) -> Option<&BadModpack> {
        let sha1 = polyio::normalize_hash(sha1);
        self.bad_modpacks.iter().find(|entry| {
            entry
                .hash
                .as_deref()
                .is_some_and(|hash| polyio::normalize_hash(hash) == sha1)
        })
    }

    pub fn find_project(&self, provider: ProviderId, project_id: &str) -> Option<&BadModpack> {
        self.bad_modpacks
            .iter()
            .find(|entry| entry.project_ids.get(provider) == Some(project_id))
    }

    pub fn find_by_name_and_author(&self, project: &ProjectDetail) -> Option<&BadModpack> {
        let authors: Vec<&str> = std::iter::once(project.author.as_str())
            .chain(project.members.iter().map(|member| member.name.as_str()))
            .collect();
        self.find_name_and_author(&project.name, &authors)
    }

    pub fn has_name_only_entries(&self) -> bool {
        self.bad_modpacks.iter().any(BadModpack::is_name_only)
    }

    fn find_name_and_author(&self, name: &str, authors: &[&str]) -> Option<&BadModpack> {
        self.bad_modpacks.iter().find(|entry| {
            let (true, Some(entry_name), Some(entry_author)) = (
                entry.is_name_only(),
                entry.name.as_deref(),
                entry.author.as_deref(),
            ) else {
                return false;
            };
            same_text(entry_name, name)
                && authors.iter().any(|author| same_text(entry_author, author))
        })
    }
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn fetch_bad_modpacks(ctx: &ContentCtx) -> ContentResult<BadModpackList> {
    fetch_flag_list("bad-modpacks.json", ctx).await
}

static BAD_MODPACKS: RwLock<Option<Arc<BadModpackList>>> = RwLock::new(None);

pub async fn refresh_bad_modpacks(ctx: &ContentCtx) -> ContentResult<Arc<BadModpackList>> {
    let list = Arc::new(fetch_bad_modpacks(ctx).await?);
    *BAD_MODPACKS.write().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(&list));
    Ok(list)
}

pub async fn load_bad_modpacks(ctx: &ContentCtx) -> Arc<BadModpackList> {
    let loaded = BAD_MODPACKS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(list) = loaded {
        return list;
    }

    refresh_bad_modpacks(ctx).await.unwrap_or_else(|err| {
        tracing::warn!(%err, "bad modpacks list unavailable, modpacks are not screened");
        Arc::default()
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn fetch_modpack_explanation(entry: &BadModpack, ctx: &ContentCtx) -> Option<String> {
    let path = entry.explanation.as_deref()?;
    let Some(file_name) = md_file_name(EXPLANATIONS_DIR, path) else {
        tracing::warn!(
            path,
            "ignoring bad modpack explanation outside {EXPLANATIONS_DIR}"
        );
        return None;
    };
    fetch_md_file(path, "bad_modpacks_mds", file_name, ctx).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &[u8] = br#"{
        "bad-modpacks": [
            {
                "hash": "28b67391892582747bd17a7525318a954e54c80a",
                "project-ids": { "modrinth": "asdasd", "curseforge": 12345 },
                "name": "Joe",
                "author": "Doue",
                "explanation": "/oneclient/bad_modpacks_mds/test.md"
            }
        ]
    }"#;

    #[test]
    fn parses_full_entry() {
        let list: BadModpackList = serde_json::from_slice(FULL).unwrap();
        let entry = &list.bad_modpacks[0];

        assert_eq!(
            entry.hash.as_deref(),
            Some("28b67391892582747bd17a7525318a954e54c80a")
        );
        assert_eq!(entry.project_ids.get(ProviderId::Modrinth), Some("asdasd"));
        assert_eq!(entry.project_ids.get(ProviderId::CurseForge), Some("12345"));
        assert_eq!(entry.name.as_deref(), Some("Joe"));
        assert_eq!(entry.author.as_deref(), Some("Doue"));
        assert_eq!(
            entry.explanation.as_deref(),
            Some("/oneclient/bad_modpacks_mds/test.md")
        );
    }

    #[test]
    fn hash_and_project_id_match() {
        let list: BadModpackList = serde_json::from_slice(FULL).unwrap();

        assert!(
            list.find("28B67391892582747BD17A7525318A954E54C80A")
                .is_some()
        );
        assert!(list.find("aaaa").is_none());
        assert!(list.find_project(ProviderId::Modrinth, "asdasd").is_some());
        assert!(
            list.find_project(ProviderId::CurseForge, "asdasd")
                .is_none()
        );
    }

    #[test]
    fn name_and_author_only_for_entries_without_ids() {
        let list: BadModpackList = serde_json::from_slice(FULL).unwrap();
        assert!(!list.has_name_only_entries());
        assert!(list.find_name_and_author("Joe", &["Doue"]).is_none());

        let list: BadModpackList =
            serde_json::from_slice(br#"{"bad-modpacks":[{"name":"Joe","author":"Doue"}]}"#)
                .unwrap();
        assert!(list.has_name_only_entries());
        assert!(list.find_name_and_author(" joe ", &["x", "DOUE"]).is_some());
        assert!(list.find_name_and_author("Joe", &["x"]).is_none());
    }

    #[test]
    fn malformed_entries_are_skipped() {
        let raw = br#"{"bad-modpacks":[{"hash":5},"nope",{"hash":"ccc"}]}"#;
        let list: BadModpackList = serde_json::from_slice(raw).unwrap();

        assert_eq!(list.bad_modpacks.len(), 1);
        assert!(list.find("ccc").is_some());
    }

    #[test]
    fn explanation_must_be_in_its_own_folder() {
        assert_eq!(
            md_file_name(EXPLANATIONS_DIR, "/oneclient/bad_modpacks_mds/test.md"),
            Some("test.md")
        );
        assert_eq!(
            md_file_name(EXPLANATIONS_DIR, "/oneclient/bad_mods_mds/test.md"),
            None
        );
        assert_eq!(
            md_file_name(EXPLANATIONS_DIR, "/oneclient/bad_modpacks_mds/../x.md"),
            None
        );
    }
}
