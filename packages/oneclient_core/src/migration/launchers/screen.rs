use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use oneclient_content::ContentCtx;
use oneclient_content::packages::{
    BadMod, BadModList, ContentType, FileIdentity, ProjectDetail, ProviderId,
    ProviderVersionLookup, ResolvedAlternative, fetch_explanation, fetch_improvements,
    resolve_alternatives,
};
use oneclient_db::models::ClusterRow;

use crate::LauncherResult;
use crate::clusters::modpack::kind_for;
use crate::state::LauncherState;

use super::ExternalInstance;
use super::import::{LocalContent, ModJar, mod_files_stamp, scan_folders, scan_mod_jars};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlaggedImportMod {
    pub hash: String,
    pub file_name: String,
    pub name: String,
    pub enabled: bool,
    pub explanation: Option<String>,
    pub alternatives: Vec<ImportAlternative>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportAlternative {
    pub resolved: ResolvedAlternative,
    pub already_installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnpublishedFile {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenedInstance {
    pub game_dir: PathBuf,
    pub name: String,
    pub mc_version: String,
    pub flagged: Vec<FlaggedImportMod>,
    pub unpublished: Vec<UnpublishedFile>,
    pub unconfirmed: usize,
    pub mods_stamp: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnpublishedCheck {
    pub mods: Vec<UnpublishedFile>,
    pub unconfirmed: usize,
}

#[tracing::instrument(skip_all, fields(instances = instances.len()))]
pub async fn screen_instances(
    state: &Arc<LauncherState>,
    instances: &[ExternalInstance],
) -> LauncherResult<Vec<ScreenedInstance>> {
    let content = state.services.content();
    let list = fetch_improvements(&content).await?;

    let mut screened = Vec::with_capacity(instances.len());
    for instance in instances {
        screened.push(screen_instance(instance, &list, &content).await);
    }
    Ok(screened)
}

async fn screen_instance(
    instance: &ExternalInstance,
    list: &BadModList,
    content: &ContentCtx,
) -> ScreenedInstance {
    let mut screened = ScreenedInstance {
        game_dir: instance.game_dir.clone(),
        name: instance.name.clone(),
        mc_version: instance.mc_version.clone(),
        flagged: Vec::new(),
        unpublished: Vec::new(),
        unconfirmed: 0,
        mods_stamp: mod_files_stamp(&instance.game_dir).await,
    };
    if !instance.loader.is_modded() {
        return screened;
    }

    let jars = scan_mod_jars(&instance.game_dir).await;
    if jars.is_empty() {
        return screened;
    }
    let (found, complete) = identify(&jars, content).await;
    let check = check_result(&jars, &found, complete);
    screened.unpublished = check.mods;
    screened.unconfirmed = check.unconfirmed;

    let mods = instance_mods(instance).await;
    if list.bad_mods.is_empty() || mods.is_empty() {
        return screened;
    }
    let projects = fetch_projects(&found, content).await;
    let target = screening_target(instance);

    for file in &mods {
        let hit = found.get(&file.hash);
        let project = hit
            .and_then(|(provider, version)| projects.get(&(*provider, version.project_id.clone())));
        let entry = hit
            .zip(project)
            .and_then(|((_, version), project)| list.check(project, version))
            .or_else(|| list.find(&file.hash));
        let Some(entry) = entry else {
            continue;
        };

        let file_name = file_name(file);
        let (explanation, alternatives) = tokio::join!(
            fetch_explanation(entry, content),
            resolve_alternatives(entry, &target, content),
        );
        screened.flagged.push(FlaggedImportMod {
            hash: file.hash.clone(),
            name: project.map_or_else(|| file_name.clone(), |project| project.name.clone()),
            file_name,
            enabled: file.enabled,
            explanation,
            alternatives: mark_installed(entry, alternatives, file, &mods, &found),
        });
    }

    screened
}

#[tracing::instrument(skip_all, fields(instance = %instance.name))]
pub async fn find_unpublished_mods(
    state: &Arc<LauncherState>,
    instance: &ExternalInstance,
) -> UnpublishedCheck {
    if !instance.loader.is_modded() {
        return UnpublishedCheck::default();
    }
    let jars = scan_mod_jars(&instance.game_dir).await;
    if jars.is_empty() {
        return UnpublishedCheck::default();
    }
    let content = state.services.content();
    let (found, complete) = identify(&jars, &content).await;
    check_result(&jars, &found, complete)
}

pub async fn instance_mods_stamp(instance: &ExternalInstance) -> u64 {
    mod_files_stamp(&instance.game_dir).await
}

fn check_result(
    jars: &[ModJar],
    found: &ProviderVersionLookup,
    complete: bool,
) -> UnpublishedCheck {
    let unknown = unpublished(jars, found);
    if complete {
        UnpublishedCheck {
            mods: unknown,
            unconfirmed: 0,
        }
    } else {
        UnpublishedCheck {
            mods: Vec::new(),
            unconfirmed: unknown.len(),
        }
    }
}

async fn instance_mods(instance: &ExternalInstance) -> Vec<LocalContent> {
    let mut seen = HashSet::new();
    scan_folders(&instance.game_dir, &[ContentType::Mod])
        .await
        .files
        .into_iter()
        .filter(|file| seen.insert(file.hash.clone()))
        .collect()
}

async fn identify(jars: &[ModJar], content: &ContentCtx) -> (ProviderVersionLookup, bool) {
    let identities: Vec<FileIdentity> = jars
        .iter()
        .map(|jar| FileIdentity {
            cf_fingerprint: jar.cf_fingerprint,
            ..FileIdentity::from_sha1(&jar.hash)
        })
        .collect();
    let (found, complete) = content
        .providers
        .lookup_versions_checked(&identities, content)
        .await
        .unwrap_or_else(|err| {
            tracing::warn!(%err, "could not identify mods, screening by hash only");
            (HashMap::new(), false)
        });
    if !complete {
        tracing::warn!("a mod site could not be asked, so unpublished mods are not reported");
    }
    (found, complete)
}

fn unpublished(jars: &[ModJar], found: &ProviderVersionLookup) -> Vec<UnpublishedFile> {
    jars.iter()
        .filter(|jar| !found.contains_key(&jar.hash))
        .map(|jar| UnpublishedFile {
            name: jar.name.clone(),
            enabled: jar.enabled,
        })
        .collect()
}

fn mark_installed(
    entry: &BadMod,
    alternatives: Vec<ResolvedAlternative>,
    flagged: &LocalContent,
    mods: &[LocalContent],
    found: &ProviderVersionLookup,
) -> Vec<ImportAlternative> {
    let own = found
        .get(&flagged.hash)
        .map(|(provider, version)| (*provider, version.project_id.clone()));
    let others = || mods.iter().filter(|file| file.hash != flagged.hash);
    let hashes: HashSet<&str> = others().map(|file| file.hash.as_str()).collect();
    let present: HashSet<(ProviderId, String)> = others()
        .filter_map(|file| found.get(&file.hash))
        .map(|(provider, version)| (*provider, version.project_id.clone()))
        .filter(|id| Some(id) != own.as_ref())
        .collect();

    let mut installed = present.clone();
    for raw in &entry.alternatives {
        let ids = [
            (ProviderId::Modrinth, raw.project_ids.modrinth.as_ref()),
            (ProviderId::CurseForge, raw.project_ids.curseforge.as_ref()),
        ];
        let in_instance = raw
            .hash
            .as_deref()
            .is_some_and(|hash| hashes.contains(polyio::normalize_hash(hash).as_str()))
            || ids.iter().any(|(provider, id)| {
                id.is_some_and(|id| present.contains(&(*provider, id.clone())))
            });
        if in_instance {
            installed.extend(
                ids.into_iter()
                    .filter_map(|(provider, id)| Some((provider, id?.clone()))),
            );
        }
    }

    alternatives
        .into_iter()
        .map(|resolved| ImportAlternative {
            already_installed: installed
                .contains(&(resolved.provider, resolved.project_id.clone())),
            resolved,
        })
        .collect()
}

async fn fetch_projects(
    found: &ProviderVersionLookup,
    content: &ContentCtx,
) -> HashMap<(ProviderId, String), ProjectDetail> {
    let mut ids: HashMap<ProviderId, HashSet<String>> = HashMap::new();
    for (provider, version) in found.values() {
        ids.entry(*provider)
            .or_default()
            .insert(version.project_id.clone());
    }

    let mut projects = HashMap::new();
    for (provider_id, ids) in ids {
        let ids: Vec<String> = ids.into_iter().collect();
        let fetched = match content.providers.get(provider_id) {
            Ok(provider) => provider.get_projects(&ids, content).await,
            Err(err) => Err(err),
        };
        match fetched {
            Ok(fetched) => {
                for project in fetched {
                    projects.insert((provider_id, project.id.clone()), project);
                }
            }
            Err(err) => {
                tracing::warn!(%err, ?provider_id, "could not fetch projects of imported mods");
            }
        }
    }
    projects
}

fn file_name(file: &LocalContent) -> String {
    file.path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn screening_target(instance: &ExternalInstance) -> ClusterRow {
    ClusterRow {
        id: 0,
        name: instance.name.clone(),
        folder_name: String::new(),
        setting_profile_name: None,
        mc_version: instance.mc_version.clone(),
        mc_loader: instance.loader as i64,
        stage: 0,
        mc_loader_version: instance.loader_version.clone(),
        created_at: None,
        last_played: None,
        overall_played: None,
        linked_modpack_hash: None,
        kind: kind_for(instance.loader) as i64,
        user_created: 1,
        description: None,
        tags: String::new(),
        cover_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(name: &str, enabled: bool) -> ModJar {
        ModJar {
            name: name.to_string(),
            enabled,
            hash: format!("sha-{name}"),
            cf_fingerprint: None,
        }
    }

    #[test]
    fn unknown_mods_are_only_called_unpublished_when_every_site_answered() {
        let jars = [jar("a.jar", true), jar("b.jar", false)];
        let found = ProviderVersionLookup::new();

        let complete = check_result(&jars, &found, true);
        assert_eq!(complete.unconfirmed, 0);
        assert_eq!(
            complete.mods,
            [
                UnpublishedFile {
                    name: "a.jar".to_string(),
                    enabled: true
                },
                UnpublishedFile {
                    name: "b.jar".to_string(),
                    enabled: false
                },
            ]
        );

        let partial = check_result(&jars, &found, false);
        assert!(partial.mods.is_empty());
        assert_eq!(partial.unconfirmed, 2);
    }
}
