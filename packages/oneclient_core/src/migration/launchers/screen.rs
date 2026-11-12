use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oneclient_common::domain::GameLoader;
use oneclient_content::ContentCtx;
use oneclient_content::modpacks;
use oneclient_content::packages::{
    BadMod, BadModList, FileIdentity, ProjectDetail, ProviderId, ProviderVersionLookup,
    ResolvedAlternative, fetch_explanation, fetch_improvements, resolve_alternatives,
};
use oneclient_db::models::ClusterRow;

use crate::{LauncherError, LauncherResult};
use crate::clusters::flag_pack_mods;
use crate::clusters::modpack::{ModpackSource, PreparedModpack, kind_for, prepare_modpack};
use crate::state::LauncherState;

use super::ExternalInstance;
use super::import::{ModJar, hash_mod_files, list_mod_files, mod_files_stamp, scan_mod_jars};

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
    pub hash: String,
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
    let files = list_mod_files(&instance.game_dir).await;
    let mut screened = ScreenedInstance {
        game_dir: instance.game_dir.clone(),
        name: instance.name.clone(),
        mc_version: instance.mc_version.clone(),
        flagged: Vec::new(),
        unpublished: Vec::new(),
        unconfirmed: 0,
        mods_stamp: mod_files_stamp(&files),
    };
    if !instance.loader.is_modded() {
        return screened;
    }

    let jars = hash_mod_files(files).await;
    if jars.is_empty() {
        return screened;
    }
    let (found, complete) = identify(&jars, content).await;
    let check = check_result(&jars, &found, complete);
    screened.unpublished = check.mods;
    screened.unconfirmed = check.unconfirmed;

    let mods: Vec<&ModJar> = jars.iter().filter(|jar| !jar.name.contains('/')).collect();
    if list.bad_mods.is_empty() || mods.is_empty() {
        return screened;
    }
    let projects = fetch_projects(&found, content).await;
    let target = screening_target(
        &instance.name,
        &instance.mc_version,
        instance.loader,
        instance.loader_version.clone(),
    );

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

#[tracing::instrument(skip(state))]
pub async fn screen_modpack_file(
    state: &Arc<LauncherState>,
    path: &Path,
) -> LauncherResult<(ScreenedInstance, PreparedModpack)> {
    let content = state.services.content();
    let list = fetch_improvements(&content).await?;
    let prepared = prepare_modpack(state, &ModpackSource::File(path.to_path_buf()), None).await?;
    let manifest = &prepared.manifest;

    let mut screened = ScreenedInstance {
        game_dir: path.to_path_buf(),
        name: prepared.instance_name.clone(),
        mc_version: manifest.mc_version.clone(),
        flagged: Vec::new(),
        unpublished: prepared
            .unlisted
            .iter()
            .map(|unlisted| UnpublishedFile {
                name: unlisted.name.clone(),
                enabled: true,
                hash: unlisted.sha1.clone(),
            })
            .collect(),
        unconfirmed: prepared.mods_unconfirmed,
        mods_stamp: 0,
    };
    if prepared.share.is_some() {
        return Err(LauncherError::ShareNotScreened);
    }
    if list.bad_mods.is_empty() || !manifest.loader.is_modded() {
        return Ok((screened, prepared));
    }

    let target = screening_target(
        &prepared.instance_name,
        &manifest.mc_version,
        manifest.loader,
        manifest.loader_version.clone(),
    );
    let present = pack_projects(manifest);
    let disabled = disabled_packages(manifest);
    screened.flagged = flag_pack_mods(&list, &prepared, Some(&target), &content)
        .await
        .into_iter()
        .filter(|file| !disabled.contains(&file.package_id))
        .map(|file| FlaggedImportMod {
            alternatives: file
                .alternatives
                .into_iter()
                .map(|resolved| ImportAlternative {
                    already_installed: resolved.project_id != file.package_id
                        && present.contains(&(resolved.provider, resolved.project_id.clone())),
                    resolved,
                })
                .collect(),
            hash: file.package_id,
            file_name: file.name.clone(),
            name: file.name,
            enabled: true,
            explanation: file.explanation,
        })
        .collect();
    Ok((screened, prepared))
}

fn disabled_packages(manifest: &modpacks::ModpackManifest) -> HashSet<String> {
    manifest
        .contents
        .files
        .iter()
        .filter(|file| !file.enabled)
        .map(|file| file.kind.package_id())
        .collect()
}

fn pack_projects(manifest: &modpacks::ModpackManifest) -> HashSet<(ProviderId, String)> {
    let contents = &manifest.contents;
    contents
        .files
        .iter()
        .filter(|file| file.enabled)
        .filter_map(|file| match &file.kind {
            oneclient_content::bundles::BundleFileKind::Managed {
                provider,
                project_id,
                ..
            } => Some((*provider, project_id.clone())),
            oneclient_content::bundles::BundleFileKind::External { .. } => None,
        })
        .chain(
            contents
                .blocked
                .iter()
                .map(|file| (ProviderId::CurseForge, file.project_id.clone())),
        )
        .collect()
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
    mod_files_stamp(&list_mod_files(&instance.game_dir).await)
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
            hash: jar.hash.clone(),
        })
        .collect()
}

fn mark_installed(
    entry: &BadMod,
    alternatives: Vec<ResolvedAlternative>,
    flagged: &ModJar,
    mods: &[&ModJar],
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

fn file_name(file: &ModJar) -> String {
    file.path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn screening_target(
    name: &str,
    mc_version: &str,
    loader: GameLoader,
    loader_version: Option<String>,
) -> ClusterRow {
    ClusterRow {
        id: 0,
        name: name.to_string(),
        folder_name: String::new(),
        setting_profile_name: None,
        mc_version: mc_version.to_string(),
        mc_loader: loader as i64,
        stage: 0,
        mc_loader_version: loader_version,
        created_at: None,
        last_played: None,
        overall_played: None,
        linked_modpack_hash: None,
        kind: kind_for(loader) as i64,
        user_created: 1,
        description: None,
        tags: String::new(),
        cover_path: None,
        takes_bundles: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(name: &str, enabled: bool) -> ModJar {
        ModJar {
            path: PathBuf::from(name),
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
                    enabled: true,
                    hash: "sha-a.jar".to_string(),
                },
                UnpublishedFile {
                    name: "b.jar".to_string(),
                    enabled: false,
                    hash: "sha-b.jar".to_string(),
                },
            ]
        );

        let partial = check_result(&jars, &found, false);
        assert!(partial.mods.is_empty());
        assert_eq!(partial.unconfirmed, 2);
    }

    #[test]
    fn a_pack_ships_its_enabled_and_blocked_projects_only() {
        use oneclient_content::bundles::{BundleFile, BundleFileKind, BundleFileType};
        use oneclient_content::modpacks::{BlockedFile, ModpackContents, ModpackManifest};
        use oneclient_content::packages::ContentType;

        let managed = |project_id: &str, enabled: bool| BundleFile {
            enabled,
            hidden: false,
            path: format!("mods/{project_id}.jar"),
            size: 0,
            file_type: BundleFileType::Normal,
            kind: BundleFileKind::Managed {
                provider: ProviderId::Modrinth,
                project_id: project_id.to_string(),
                version_id: "v1".to_string(),
                sha1: format!("sha-{project_id}"),
            },
        };
        let manifest = ModpackManifest {
            name: "Pack".to_string(),
            version: "1.0".to_string(),
            summary: None,
            mc_version: "1.21.1".to_string(),
            loader: GameLoader::Fabric,
            loader_version: None,
            contents: ModpackContents {
                files: vec![managed("sodium", true), managed("optifine", false)],
                blocked: vec![BlockedFile {
                    project_id: "123".to_string(),
                    version_id: "456".to_string(),
                    project_name: "Blocked".to_string(),
                    file_name: "blocked.jar".to_string(),
                    path: "mods/blocked.jar".to_string(),
                    sha1: "sha-blocked".to_string(),
                    size: 0,
                    content_type: ContentType::Mod,
                    page_url: None,
                }],
                ..ModpackContents::default()
            },
            override_prefixes: Vec::new(),
        };

        assert_eq!(
            pack_projects(&manifest),
            HashSet::from([
                (ProviderId::Modrinth, "sodium".to_string()),
                (ProviderId::CurseForge, "123".to_string()),
            ])
        );
        assert_eq!(
            disabled_packages(&manifest),
            HashSet::from(["optifine".to_string()])
        );
    }
}
