use std::sync::{Arc, PoisonError, RwLock};

use serde::Deserialize;

use super::bad_mods::{BadMod, BadModList, fetch_flag_list, fetch_md_file, lenient_list, md_file_name};
use crate::ctx::ContentCtx;
use crate::error::ContentResult;

const EXPLANATIONS_DIR: &str = "/oneclient/bad_modpacks_mds/";

#[derive(Debug, Clone, Default, Deserialize)]
struct BadModpackFile {
    #[serde(rename = "bad-modpacks", default, deserialize_with = "lenient_list")]
    bad_modpacks: Vec<BadMod>,
}

impl From<BadModpackFile> for BadModList {
    fn from(file: BadModpackFile) -> Self {
        Self {
            bad_mods: file.bad_modpacks,
        }
    }
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn fetch_bad_modpacks(ctx: &ContentCtx) -> ContentResult<BadModList> {
    Ok(fetch_flag_list::<BadModpackFile>("bad-modpacks.json", ctx)
        .await?
        .into())
}

static BAD_MODPACKS: RwLock<Option<Arc<BadModList>>> = RwLock::new(None);

pub async fn refresh_bad_modpacks(ctx: &ContentCtx) -> ContentResult<Arc<BadModList>> {
    let list = Arc::new(fetch_bad_modpacks(ctx).await?);
    *BAD_MODPACKS.write().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(&list));
    Ok(list)
}

pub async fn load_bad_modpacks(ctx: &ContentCtx) -> Arc<BadModList> {
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
pub async fn fetch_modpack_explanation(entry: &BadMod, ctx: &ContentCtx) -> Option<String> {
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
    use crate::packages::ProviderId;

    fn parse(raw: &[u8]) -> BadModList {
        serde_json::from_slice::<BadModpackFile>(raw).unwrap().into()
    }

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
        let list = parse(FULL);
        let entry = &list.bad_mods[0];

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
        let list = parse(FULL);

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
        let list = parse(FULL);
        assert!(!list.has_name_only_entries());
        assert!(list.find_name_and_author("Joe", &["Doue"]).is_none());

        let list = parse(br#"{"bad-modpacks":[{"name":"Joe","author":"Doue"}]}"#);
        assert!(list.has_name_only_entries());
        assert!(list.find_name_and_author(" joe ", &["x", "DOUE"]).is_some());
        assert!(list.find_name_and_author("Joe", &["x"]).is_none());
    }

    #[test]
    fn malformed_entries_are_skipped() {
        let raw = br#"{"bad-modpacks":[{"hash":5},"nope",{"hash":"ccc"}]}"#;
        let list = parse(raw);

        assert_eq!(list.bad_mods.len(), 1);
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
