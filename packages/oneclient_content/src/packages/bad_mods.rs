use std::collections::HashSet;
use std::sync::{Arc, PoisonError, RwLock};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

use oneclient_common::paths;
use oneclient_db::models::ClusterRow;
use oneclient_net::{EtagPolicy, fetch_cached};

use super::dependencies::pick_version;
use super::{ContentType, FileIdentity, ProjectDetail, ProviderId, VersionDetail, VersionLookup};
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BadModList {
    #[serde(rename = "bad-mods", default, deserialize_with = "lenient_list")]
    pub bad_mods: Vec<BadMod>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BadMod {
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
    #[serde(default, deserialize_with = "lenient_list")]
    pub alternatives: Vec<BadModAlternative>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BadModAlternative {
    #[serde(default, deserialize_with = "non_blank")]
    pub hash: Option<String>,
    #[serde(default)]
    pub project_ids: ProjectIds,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ProjectIds {
    #[serde(default, deserialize_with = "project_id")]
    pub modrinth: Option<String>,
    #[serde(default, deserialize_with = "project_id")]
    pub curseforge: Option<String>,
}

impl ProjectIds {
    pub fn get(&self, provider: ProviderId) -> Option<&str> {
        match provider {
            ProviderId::Modrinth => self.modrinth.as_deref(),
            ProviderId::CurseForge => self.curseforge.as_deref(),
            ProviderId::Local => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.modrinth.is_none() && self.curseforge.is_none()
    }
}

pub(super) fn lenient_list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let raw = Option::<Vec<serde_json::Value>>::deserialize(deserializer)?.unwrap_or_default();

    Ok(raw
        .into_iter()
        .enumerate()
        .filter_map(|(index, value)| match serde_json::from_value(value) {
            Ok(item) => Some(item),
            Err(err) => {
                tracing::warn!(index, %err, "skipping malformed bad mods entry");
                None
            }
        })
        .collect())
}

fn blank_to_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub(super) fn non_blank<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.and_then(blank_to_none))
}

fn project_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RawId {
        Text(String),
        Number(u64),
    }

    Ok(match Option::<RawId>::deserialize(deserializer)? {
        Some(RawId::Text(id)) => blank_to_none(id),
        Some(RawId::Number(id)) => Some(id.to_string()),
        None => None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAlternative {
    pub provider: ProviderId,
    pub project_id: String,
    pub name: String,
    pub version_id: Option<String>,
    pub version_number: Option<String>,
    pub icon_url: Option<String>,
}

impl BadModList {
    pub fn check(&self, project: &ProjectDetail, version: &VersionDetail) -> Option<&BadMod> {
        if project.content_type != ContentType::Mod {
            return None;
        }

        version
            .primary_file()
            .and_then(|file| self.find(&file.sha1))
            .or_else(|| self.find_project(project.provider, &project.id))
            .or_else(|| self.find_by_name_and_author(project))
    }

    pub fn find_by_name_and_author(&self, project: &ProjectDetail) -> Option<&BadMod> {
        let authors: Vec<&str> = std::iter::once(project.author.as_str())
            .chain(project.members.iter().map(|member| member.name.as_str()))
            .collect();
        self.find_name_and_author(&project.name, &authors)
    }

    pub fn has_name_only_entries(&self) -> bool {
        self.bad_mods.iter().any(|entry| {
            entry.hash.is_none()
                && entry.project_ids.is_empty()
                && entry.name.is_some()
                && entry.author.is_some()
        })
    }

    pub fn find(&self, sha1: &str) -> Option<&BadMod> {
        let sha1 = polyio::normalize_hash(sha1);
        self.bad_mods.iter().find(|entry| {
            entry
                .hash
                .as_deref()
                .is_some_and(|hash| polyio::normalize_hash(hash) == sha1)
        })
    }

    pub fn find_project(&self, provider: ProviderId, project_id: &str) -> Option<&BadMod> {
        self.bad_mods
            .iter()
            .find(|entry| entry.project_ids.get(provider) == Some(project_id))
    }

    fn find_name_and_author(&self, name: &str, authors: &[&str]) -> Option<&BadMod> {
        self.bad_mods.iter().find(|entry| {
            if entry.hash.is_some() || !entry.project_ids.is_empty() {
                return false;
            }
            let (Some(entry_name), Some(entry_author)) =
                (entry.name.as_deref(), entry.author.as_deref())
            else {
                return false;
            };
            same_text(entry_name, name)
                && authors.iter().any(|author| same_text(entry_author, author))
        })
    }
}

pub(super) fn same_text(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn fetch_bad_mods(ctx: &ContentCtx) -> ContentResult<BadModList> {
    fetch_flag_list("bad-mods.json", ctx).await
}

pub(super) async fn fetch_flag_list<T: DeserializeOwned>(
    file_name: &str,
    ctx: &ContentCtx,
) -> ContentResult<T> {
    let url = format!("{}/oneclient/{file_name}", ctx.net.config().meta_url_base);
    let cache_path = paths::caches_dir()?.join(file_name);

    let Some(fetched) = fetch_cached(&ctx.net, &url, &cache_path, EtagPolicy::CommitNow).await?
    else {
        return Err(ContentError::InvalidData {
            reason: format!("{file_name} is unavailable and not cached"),
        });
    };

    Ok(fetched.json()?)
}

static BAD_MODS: RwLock<Option<Arc<BadModList>>> = RwLock::new(None);

pub async fn refresh_bad_mods(ctx: &ContentCtx) -> ContentResult<Arc<BadModList>> {
    let list = Arc::new(fetch_bad_mods(ctx).await?);
    *BAD_MODS.write().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(&list));
    Ok(list)
}

pub async fn load_bad_mods(ctx: &ContentCtx) -> Arc<BadModList> {
    let loaded = BAD_MODS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(list) = loaded {
        return list;
    }

    refresh_bad_mods(ctx).await.unwrap_or_else(|err| {
        tracing::warn!(%err, "bad mods list unavailable, installs are not screened");
        Arc::default()
    })
}

const EXPLANATIONS_DIR: &str = "/oneclient/bad_mods_mds/";

fn explanation_file_name(path: &str) -> Option<&str> {
    md_file_name(EXPLANATIONS_DIR, path)
}

pub(super) fn md_file_name<'a>(dir: &str, path: &'a str) -> Option<&'a str> {
    let name = path.strip_prefix(dir)?;
    let valid = name.ends_with(".md")
        && name.len() > ".md".len()
        && !name.contains("..")
        && !name.contains(['/', '\\', '?', '#']);
    valid.then_some(name)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn fetch_explanation(entry: &BadMod, ctx: &ContentCtx) -> Option<String> {
    let path = entry.explanation.as_deref()?;
    let Some(file_name) = explanation_file_name(path) else {
        tracing::warn!(
            path,
            "ignoring bad mod explanation outside {EXPLANATIONS_DIR}"
        );
        return None;
    };
    fetch_md_file(path, "bad_mods_mds", file_name, ctx).await
}

pub(super) async fn fetch_md_file(
    path: &str,
    cache_dir: &str,
    file_name: &str,
    ctx: &ContentCtx,
) -> Option<String> {
    let url = format!("{}{path}", ctx.net.config().meta_url_base);
    let cache_path = match paths::caches_dir() {
        Ok(dir) => dir.join(cache_dir).join(file_name),
        Err(err) => {
            tracing::warn!(%err, "cannot resolve the cache dir for flagged content explanations");
            return None;
        }
    };

    match fetch_cached(&ctx.net, &url, &cache_path, EtagPolicy::CommitNow).await {
        Ok(Some(fetched)) => Some(fetched.text()).filter(|text| !text.trim().is_empty()),
        Ok(None) => {
            tracing::warn!(path, "flagged content explanation is unavailable and not cached");
            None
        }
        Err(err) => {
            tracing::warn!(%err, path, "failed to fetch flagged content explanation");
            None
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn resolve_alternatives(
    entry: &BadMod,
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> Vec<ResolvedAlternative> {
    let by_hash = lookup_alternative_hashes(entry, ctx).await;

    let hits: Vec<(&BadModAlternative, Option<&VersionDetail>)> = entry
        .alternatives
        .iter()
        .map(|alternative| {
            let hit = alternative
                .hash
                .as_deref()
                .and_then(|hash| by_hash.get(&polyio::normalize_hash(hash)));
            (alternative, hit)
        })
        .collect();

    let mut modrinth_ids = HashSet::new();
    let mut curseforge_ids = HashSet::new();
    for (alternative, hit) in &hits {
        match hit {
            Some(version) => {
                modrinth_ids.insert(version.project_id.clone());
            }
            None => {
                if let Some(id) = alternative.project_ids.modrinth.as_deref() {
                    if is_modrinth_id(id) {
                        modrinth_ids.insert(id.to_string());
                    } else {
                        tracing::warn!(id, "ignoring malformed Modrinth project id");
                    }
                }
                if let Some(id) = alternative.project_ids.curseforge.as_deref() {
                    curseforge_ids.insert(id.to_string());
                }
            }
        }
    }

    let (modrinth, curseforge) = tokio::join!(
        fetch_projects(ProviderId::Modrinth, modrinth_ids, ctx),
        fetch_projects(ProviderId::CurseForge, curseforge_ids, ctx),
    );

    let resolved = futures_util::future::join_all(hits.into_iter().map(|(alternative, hit)| {
        resolve_alternative(alternative, hit, &modrinth, &curseforge, cluster, ctx)
    }))
    .await;

    let mut seen = HashSet::new();
    resolved
        .into_iter()
        .flatten()
        .filter(|alternative| seen.insert((alternative.provider, alternative.project_id.clone())))
        .collect()
}

async fn lookup_alternative_hashes(entry: &BadMod, ctx: &ContentCtx) -> VersionLookup {
    let mut seen = HashSet::new();
    let identities: Vec<FileIdentity> = entry
        .alternatives
        .iter()
        .filter_map(|alt| alt.hash.as_deref())
        .map(FileIdentity::from_sha1)
        .filter(|identity| seen.insert(identity.sha1.clone()))
        .collect();

    if identities.is_empty() {
        return VersionLookup::new();
    }

    let lookup = match ctx.providers.get(ProviderId::Modrinth) {
        Ok(provider) => provider.lookup_versions(&identities, ctx).await,
        Err(err) => Err(err),
    };

    lookup.unwrap_or_else(|err| {
        tracing::warn!(%err, "failed to look up bad mod alternatives by hash");
        VersionLookup::new()
    })
}

fn is_modrinth_id(id: &str) -> bool {
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

async fn fetch_projects(
    provider_id: ProviderId,
    ids: HashSet<String>,
    ctx: &ContentCtx,
) -> Vec<ProjectDetail> {
    if ids.is_empty() {
        return Vec::new();
    }

    let ids: Vec<String> = ids.into_iter().collect();
    let fetched = match ctx.providers.get(provider_id) {
        Ok(provider) => provider.get_projects(&ids, ctx).await,
        Err(err) => Err(err),
    };

    fetched.unwrap_or_else(|err| {
        tracing::warn!(%err, ?provider_id, "failed to fetch alternative projects");
        Vec::new()
    })
}

fn find_project<'a>(projects: &'a [ProjectDetail], id: &str) -> Option<&'a ProjectDetail> {
    projects
        .iter()
        .find(|project| project.id == id || project.slug.eq_ignore_ascii_case(id))
}

async fn resolve_alternative(
    alternative: &BadModAlternative,
    hit: Option<&VersionDetail>,
    modrinth: &[ProjectDetail],
    curseforge: &[ProjectDetail],
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> Option<ResolvedAlternative> {
    if let Some(version) = hit {
        let (name, icon_url) = find_project(modrinth, &version.project_id)
            .map(|project| (project.name.clone(), project.icon_url.clone()))
            .unwrap_or_else(|| (version.name.clone(), None));

        return Some(ResolvedAlternative {
            provider: ProviderId::Modrinth,
            project_id: version.project_id.clone(),
            name,
            version_id: Some(version.version_id.clone()),
            version_number: Some(version.version_number.clone()),
            icon_url,
        });
    }

    for (provider_id, projects) in [
        (ProviderId::Modrinth, modrinth),
        (ProviderId::CurseForge, curseforge),
    ] {
        let Some(project) = alternative
            .project_ids
            .get(provider_id)
            .and_then(|id| find_project(projects, id))
        else {
            continue;
        };
        return Some(with_newest_version(provider_id, project, cluster, ctx).await);
    }

    tracing::warn!(
        ?alternative,
        "bad mod alternative not found by hash or project id"
    );
    None
}

async fn with_newest_version(
    provider_id: ProviderId,
    project: &ProjectDetail,
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> ResolvedAlternative {
    let picked = match ctx.providers.get(provider_id) {
        Ok(provider) => pick_version(provider, &project.id, cluster, ctx).await,
        Err(err) => Err(err),
    };

    let (version_id, version_number) = match picked {
        Ok(Some(pick)) => (Some(pick.version_id), Some(pick.version_number)),
        Ok(None) => (None, None),
        Err(err) => {
            tracing::warn!(%err, ?provider_id, project_id = %project.id, "failed to pick alternative version");
            (None, None)
        }
    };

    ResolvedAlternative {
        provider: provider_id,
        project_id: project.id.clone(),
        name: project.name.clone(),
        version_id,
        version_number,
        icon_url: project.icon_url.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &[u8] = br#"{
        "bad-mods": [
            {
                "hash": "28b67391892582747bd17a7525318a954e54c80a",
                "project-ids": { "modrinth": "1bokaNcj", "curseforge": "263420" },
                "name": "Xaero's Minimap",
                "author": "xaero96",
                "alternatives": [
                    {
                        "hash": "d1741855c0e2b433f615c25e61c211b594dc525d",
                        "project-ids": { "modrinth": "HXF82T3G", "curseforge": "220318" }
                    },
                    {
                        "hash": "04739b04815269648fb885526b2c2ff2266e0cc5",
                        "project-ids": { "modrinth": "OhduvhIc" }
                    }
                ]
            }
        ]
    }"#;

    #[test]
    fn parses_full_entry() {
        let list: BadModList = serde_json::from_slice(FULL).unwrap();
        let entry = &list.bad_mods[0];

        assert_eq!(
            entry.hash.as_deref(),
            Some("28b67391892582747bd17a7525318a954e54c80a")
        );
        assert_eq!(
            entry.project_ids.get(ProviderId::Modrinth),
            Some("1bokaNcj")
        );
        assert_eq!(
            entry.project_ids.get(ProviderId::CurseForge),
            Some("263420")
        );
        assert_eq!(entry.name.as_deref(), Some("Xaero's Minimap"));
        assert_eq!(entry.author.as_deref(), Some("xaero96"));

        assert_eq!(entry.alternatives.len(), 2);
        assert_eq!(
            entry.alternatives[1].project_ids.get(ProviderId::Modrinth),
            Some("OhduvhIc")
        );
        assert_eq!(entry.alternatives[1].project_ids.curseforge, None);
    }

    #[test]
    fn every_field_is_optional() {
        let list: BadModList = serde_json::from_slice(br#"{}"#).unwrap();
        assert!(list.bad_mods.is_empty());

        let list: BadModList = serde_json::from_slice(
            br#"{"bad-mods":[{"name":"X","author":"Y","alternatives":[{}]}]}"#,
        )
        .unwrap();
        let entry = &list.bad_mods[0];
        assert_eq!(entry.hash, None);
        assert!(entry.project_ids.is_empty());
        assert_eq!(entry.alternatives[0].hash, None);
        assert!(entry.alternatives[0].project_ids.is_empty());
    }

    #[test]
    fn blank_values_count_as_missing() {
        let raw = br#"{"bad-mods":[{"hash":"  ","name":"","project-ids":{"modrinth":" ","curseforge":""}}]}"#;
        let list: BadModList = serde_json::from_slice(raw).unwrap();
        let entry = &list.bad_mods[0];

        assert_eq!(entry.hash, None);
        assert_eq!(entry.name, None);
        assert!(entry.project_ids.is_empty());
    }

    #[test]
    fn curseforge_id_accepts_a_number() {
        let raw = br#"{"bad-mods":[{"project-ids":{"curseforge":263420}}]}"#;
        let list: BadModList = serde_json::from_slice(raw).unwrap();

        assert_eq!(
            list.bad_mods[0].project_ids.get(ProviderId::CurseForge),
            Some("263420")
        );
    }

    #[test]
    fn find_ignores_case_and_whitespace() {
        let list: BadModList =
            serde_json::from_slice(br#"{"bad-mods":[{"hash":" ABCdef "},{"name":"X"}]}"#).unwrap();

        assert!(list.find("abcdef").is_some());
        assert!(list.find("ABCDEF").is_some());
        assert!(list.find("abcde0").is_none());
    }

    #[test]
    fn project_id_matches_only_its_provider() {
        let list: BadModList = serde_json::from_slice(FULL).unwrap();

        assert!(
            list.find_project(ProviderId::Modrinth, "1bokaNcj")
                .is_some()
        );
        assert!(
            list.find_project(ProviderId::CurseForge, "263420")
                .is_some()
        );
        assert!(
            list.find_project(ProviderId::CurseForge, "1bokaNcj")
                .is_none()
        );
        assert!(list.find_project(ProviderId::Modrinth, "other").is_none());
    }

    const NAME_ONLY: &[u8] = br#"{"bad-mods":[{"name":"Xaero's Minimap","author":"xaero96"}]}"#;

    #[test]
    fn name_and_author_must_both_match() {
        let list: BadModList = serde_json::from_slice(NAME_ONLY).unwrap();

        assert!(
            list.find_name_and_author(" xaero's minimap ", &["someone", "XAERO96"])
                .is_some()
        );
        assert!(
            list.find_name_and_author("Xaero's Minimap", &["someone"])
                .is_none()
        );
        assert!(
            list.find_name_and_author("Xaero's World Map", &["xaero96"])
                .is_none()
        );
    }

    #[test]
    fn name_and_author_ignored_when_hash_or_project_id_present() {
        let list: BadModList = serde_json::from_slice(FULL).unwrap();
        assert!(
            list.find_name_and_author("Xaero's Minimap", &["xaero96"])
                .is_none()
        );

        let with_hash: BadModList = serde_json::from_slice(
            br#"{"bad-mods":[{"hash":"aaa","name":"Xaero's Minimap","author":"xaero96"}]}"#,
        )
        .unwrap();
        assert!(
            with_hash
                .find_name_and_author("Xaero's Minimap", &["xaero96"])
                .is_none()
        );

        let with_id: BadModList = serde_json::from_slice(
            br#"{"bad-mods":[{"project-ids":{"curseforge":"1"},"name":"Xaero's Minimap","author":"xaero96"}]}"#,
        )
        .unwrap();
        assert!(
            with_id
                .find_name_and_author("Xaero's Minimap", &["xaero96"])
                .is_none()
        );
    }

    #[test]
    fn malformed_entries_are_skipped_not_fatal() {
        let raw = br#"{"bad-mods":[
            {"hash":"aaa","alternatives":[{"hash":5},{"hash":"bbb"}]},
            {"hash":123},
            {"project-ids":"oops"},
            "not an object",
            {"hash":"ccc"}
        ]}"#;
        let list: BadModList = serde_json::from_slice(raw).unwrap();

        assert_eq!(list.bad_mods.len(), 2);
        assert_eq!(list.bad_mods[0].alternatives.len(), 1);
        assert_eq!(
            list.bad_mods[0].alternatives[0].hash.as_deref(),
            Some("bbb")
        );
        assert!(list.find("ccc").is_some());
    }

    #[test]
    fn null_list_is_empty() {
        let list: BadModList = serde_json::from_slice(br#"{"bad-mods":null}"#).unwrap();
        assert!(list.bad_mods.is_empty());
    }

    #[test]
    fn modrinth_ids_reject_url_breaking_characters() {
        assert!(is_modrinth_id("1bokaNcj"));
        assert!(is_modrinth_id("xaeros-minimap"));
        assert!(!is_modrinth_id("bad id"));
        assert!(!is_modrinth_id("a\"],[\"b"));
    }

    #[test]
    fn parses_explanation_path() {
        let raw = br#"{"bad-mods":[{"hash":"aaa","explanation":"/oneclient/bad_mods_mds/test-file.md"},{"hash":"bbb","explanation":"  "}]}"#;
        let list: BadModList = serde_json::from_slice(raw).unwrap();

        assert_eq!(
            list.bad_mods[0].explanation.as_deref(),
            Some("/oneclient/bad_mods_mds/test-file.md")
        );
        assert_eq!(list.bad_mods[1].explanation, None);
    }

    #[test]
    fn explanation_must_be_an_md_file_in_the_folder() {
        assert_eq!(
            explanation_file_name("/oneclient/bad_mods_mds/test-file.md"),
            Some("test-file.md")
        );
        assert_eq!(explanation_file_name("/oneclient/tos.json"), None);
        assert_eq!(
            explanation_file_name("/oneclient/bad_mods_mds/notes.txt"),
            None
        );
        assert_eq!(explanation_file_name("/oneclient/bad_mods_mds/.md"), None);
        assert_eq!(
            explanation_file_name("/oneclient/bad_mods_mds/../tos.md"),
            None
        );
        assert_eq!(
            explanation_file_name("/oneclient/bad_mods_mds/sub/file.md"),
            None
        );
        assert_eq!(
            explanation_file_name("/oneclient/bad_mods_mds/a.md?x=1"),
            None
        );
        assert_eq!(
            explanation_file_name("oneclient/bad_mods_mds/test-file.md"),
            None
        );
        assert_eq!(
            explanation_file_name("https://evil.example/oneclient/bad_mods_mds/x.md"),
            None
        );
    }

    #[test]
    fn name_without_author_never_matches() {
        let list: BadModList = serde_json::from_slice(br#"{"bad-mods":[{"name":"X"}]}"#).unwrap();

        assert!(list.find_name_and_author("X", &[""]).is_none());
    }
}
