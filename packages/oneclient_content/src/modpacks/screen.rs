use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::{ModpackManifest, tracked_content_type};
use crate::bundles::BundleFileKind;
use crate::bundles::overrides::OverrideLayers;
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::{BadMod, BadModList, ProjectDetail};
use oneclient_common::domain::{ContentType, ProviderId};

#[derive(Debug, Clone)]
pub struct FlaggedPackFile<'a> {
    pub package_id: String,
    pub name: String,
    pub entry: &'a BadMod,
}

struct Candidate {
    package_id: String,
    name: String,
    sha1: String,
    project: Option<(ProviderId, String)>,
}

#[tracing::instrument(level = "debug", skip(manifest))]
pub async fn bundled_mods(
    archive_path: &Path,
    manifest: &ModpackManifest,
) -> ContentResult<Vec<(String, String)>> {
    let prefixes: Vec<&str> = manifest
        .override_prefixes
        .iter()
        .map(String::as_str)
        .collect();
    let mut layers = OverrideLayers::open(archive_path, &prefixes).await?;
    let entries: Vec<(String, String)> = layers
        .entries()
        .iter()
        .filter(|(rel, _)| tracked_content_type(rel) == Some(ContentType::Mod))
        .cloned()
        .collect();

    let mut mods = Vec::new();
    for (rel, entry) in entries {
        let Some(bytes) = layers.read(&entry).await else {
            continue;
        };
        let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
        mods.push((name, polyio::normalize_hash(&polyio::sha1_bytes(&bytes))));
    }
    Ok(mods)
}

#[tracing::instrument(level = "debug", skip_all, fields(pack = %manifest.name))]
pub async fn screen_modpack<'a>(
    list: &'a BadModList,
    manifest: &ModpackManifest,
    bundled: &[(String, String)],
    ctx: &ContentCtx,
) -> Vec<FlaggedPackFile<'a>> {
    let candidates = candidates(manifest, bundled);

    let mut flagged = Vec::new();
    let mut unmatched = Vec::new();
    for candidate in candidates {
        let entry = list.find(&candidate.sha1).or_else(|| {
            candidate
                .project
                .as_ref()
                .and_then(|(provider, id)| list.find_project(*provider, id))
        });
        match entry {
            Some(entry) => flagged.push(FlaggedPackFile {
                package_id: candidate.package_id,
                name: candidate.name,
                entry,
            }),
            None => unmatched.push(candidate),
        }
    }

    if list.has_name_only_entries() {
        let projects = fetch_projects(&unmatched, ctx).await;
        for candidate in unmatched {
            let Some(project) = candidate.project.as_ref().and_then(|key| projects.get(key)) else {
                continue;
            };
            if let Some(entry) = list.find_by_name_and_author(project) {
                flagged.push(FlaggedPackFile {
                    package_id: candidate.package_id,
                    name: project.name.clone(),
                    entry,
                });
            }
        }
    }

    flagged
}

fn candidates(manifest: &ModpackManifest, bundled: &[(String, String)]) -> Vec<Candidate> {
    let contents = &manifest.contents;
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    for file in &contents.files {
        if file.content_type() != ContentType::Mod {
            continue;
        }
        let (sha1, project) = match &file.kind {
            BundleFileKind::Managed {
                provider,
                project_id,
                sha1,
                ..
            } => (sha1.clone(), Some((*provider, project_id.clone()))),
            BundleFileKind::External { file, .. } => (file.sha1.clone(), None),
        };
        candidates.push(Candidate {
            package_id: file.kind.package_id(),
            name: file.display_name(),
            sha1,
            project,
        });
    }

    for file in &contents.blocked {
        if file.content_type != ContentType::Mod {
            continue;
        }
        candidates.push(Candidate {
            package_id: file.project_id.clone(),
            name: file.project_name.clone(),
            sha1: file.sha1.clone(),
            project: Some((ProviderId::CurseForge, file.project_id.clone())),
        });
    }

    for (name, hash) in bundled {
        candidates.push(Candidate {
            package_id: hash.clone(),
            name: name.clone(),
            sha1: hash.clone(),
            project: None,
        });
    }

    candidates.retain(|candidate| seen.insert(candidate.package_id.clone()));
    candidates
}

async fn fetch_projects(
    candidates: &[Candidate],
    ctx: &ContentCtx,
) -> HashMap<(ProviderId, String), ProjectDetail> {
    let mut ids: HashMap<ProviderId, Vec<String>> = HashMap::new();
    for (provider, id) in candidates.iter().filter_map(|c| c.project.as_ref()) {
        ids.entry(*provider).or_default().push(id.clone());
    }

    let fetches = ids.into_iter().map(|(provider_id, ids)| async move {
        let fetched = match ctx.providers.get(provider_id) {
            Ok(provider) => provider.get_projects(&ids, ctx).await,
            Err(err) => Err(err),
        };
        match fetched {
            Ok(projects) => projects
                .into_iter()
                .map(|project| ((provider_id, project.id.clone()), project))
                .collect::<Vec<_>>(),
            Err(err) => {
                tracing::warn!(%err, ?provider_id, "could not fetch modpack projects for screening");
                Vec::new()
            }
        }
    });

    futures_util::future::join_all(fetches)
        .await
        .into_iter()
        .flatten()
        .collect()
}
