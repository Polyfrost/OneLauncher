use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_content::packages::{
    DependencyKind, DependencyResolution, FileIdentity, InstalledPackage, PackageStore,
    ReleaseType, ResolvedDependency, VersionDetail, VersionSummary, curseforge_fingerprint,
    modrinth_client_only, modrinth_plugin_versions, store::artifact_absolute_path,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::LocalServer;
use crate::state::LauncherState;
use crate::{LauncherError, LauncherResult};

/// A jar in the server's mods folder with what its own metadata says about it
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerMod {
    pub file_name: String,
    pub size: u64,
    pub enabled: bool,
    pub name: Option<String>,
    pub version: Option<String>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub icon: Option<Vec<u8>>,
    pub source: Option<ModSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModSource {
    pub provider: ProviderId,
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
}

const SOURCES_FILE: &str = "oneclient-mods.json";

const DISABLED: &str = ".disabled";
const META_FILES: [&str; 7] = [
    "fabric.mod.json",
    "quilt.mod.json",
    "META-INF/neoforge.mods.toml",
    "META-INF/mods.toml",
    "mcmod.info",
    "paper-plugin.yml",
    "plugin.yml",
];

pub async fn list_mods(dir: &Path, folder: &str) -> Vec<ServerMod> {
    let mut out = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir.join(folder)).await else {
        return out;
    };
    let sources = read_sources(dir).await;
    let mut present = HashSet::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let enabled = file_name.ends_with(".jar");
        if !enabled && !file_name.ends_with(&format!(".jar{DISABLED}")) {
            continue;
        }
        let meta = entry.metadata().await.ok();
        let size = meta.as_ref().map_or(0, std::fs::Metadata::len);
        let modified = meta.and_then(|m| m.modified().ok());
        let path = entry.path();
        let cached = META_CACHE
            .lock()
            .unwrap()
            .get(&path)
            .filter(|(s, m, _)| *s == size && *m == modified)
            .map(|(_, _, found)| found.clone());
        let mut found = match cached {
            Some(found) => found,
            None => {
                let found = read_meta(&path).await;
                META_CACHE
                    .lock()
                    .unwrap()
                    .insert(path.clone(), (size, modified, found.clone()));
                found
            }
        };
        present.insert(path);
        found.source = sources.get(file_name.trim_end_matches(DISABLED)).cloned();
        found.file_name = file_name;
        found.size = size;
        found.enabled = enabled;
        out.push(found);
    }
    let mods = dir.join(folder);
    META_CACHE
        .lock()
        .unwrap()
        .retain(|path, _| path.parent() != Some(mods.as_path()) || present.contains(path));
    out
}

type CachedMeta = (u64, Option<std::time::SystemTime>, ServerMod);

static META_CACHE: LazyLock<Mutex<HashMap<PathBuf, CachedMeta>>> = LazyLock::new(Default::default);

async fn read_meta(jar: &Path) -> ServerMod {
    let Ok(entries) = polyio::read_zip_file_entries(jar, |name| META_FILES.contains(&name)).await
    else {
        return ServerMod::default();
    };
    let Some((file, bytes)) = META_FILES
        .iter()
        .find_map(|wanted| entries.iter().find(|(name, _)| name == wanted))
    else {
        return ServerMod::default();
    };
    let mut meta = parse_meta(file, bytes);
    if let Some(icon_path) = meta.icon_path.take() {
        meta.found.icon = polyio::read_zip_file_entries(jar, |name| name == icon_path)
            .await
            .ok()
            .and_then(|mut e| e.pop())
            .map(|(_, bytes)| bytes);
    }
    meta.found
}

struct Parsed {
    found: ServerMod,
    icon_path: Option<String>,
}

fn parse_meta(file: &str, bytes: &[u8]) -> Parsed {
    if file.ends_with(".toml") {
        return parse_toml(&String::from_utf8_lossy(bytes));
    }
    if file.ends_with(".yml") {
        return parse_plugin_yml(&String::from_utf8_lossy(bytes));
    }
    if file == "mcmod.info" {
        return parse_mcmod(bytes);
    }
    let json: Value = serde_json::from_slice(bytes).unwrap_or_default();
    let quilt = file == "quilt.mod.json";
    let meta = if quilt {
        json.pointer("/quilt_loader/metadata")
    } else {
        Some(&json)
    };
    let text = |v: Option<&Value>| {
        v.and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    };

    let authors = if quilt {
        meta.and_then(|m| m.get("contributors"))
            .and_then(Value::as_object)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    } else {
        json.get("authors")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|a| {
                        a.as_str()
                            .or_else(|| a.get("name")?.as_str())
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    // An icon is either one path or a map of sizes to paths, the largest reads best scaled down
    let icon_path = meta
        .and_then(|m| m.get("icon"))
        .and_then(|icon| match icon {
            Value::String(path) => Some(path.clone()),
            Value::Object(sizes) => sizes
                .iter()
                .max_by_key(|(size, _)| size.parse::<u32>().unwrap_or(0))
                .and_then(|(_, path)| path.as_str().map(str::to_string)),
            _ => None,
        });

    Parsed {
        found: ServerMod {
            name: text(meta.and_then(|m| m.get("name"))),
            version: text(if quilt {
                json.pointer("/quilt_loader/version")
            } else {
                json.get("version")
            }),
            authors,
            description: text(meta.and_then(|m| m.get("description"))),
            ..ServerMod::default()
        },
        icon_path: icon_path.map(|p| p.trim_start_matches('/').to_string()),
    }
}

/// shortcut: reads single-line `key = "value"` pairs of the first mod, multi-line values are skipped
fn parse_toml(text: &str) -> Parsed {
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let (k, v) = line.split_once('=')?;
            if k.trim() != key {
                return None;
            }
            let v = v.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            (!v.is_empty() && !v.contains("${")).then(|| v.to_string())
        })
    };
    Parsed {
        found: ServerMod {
            name: value("displayName"),
            version: value("version"),
            authors: value("authors").map(|a| vec![a]).unwrap_or_default(),
            description: value("description"),
            ..ServerMod::default()
        },
        icon_path: value("logoFile"),
    }
}

fn usable_text(value: &str) -> Option<String> {
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    (!value.is_empty() && !value.contains("${")).then(|| value.to_string())
}

fn parse_mcmod(bytes: &[u8]) -> Parsed {
    let text = String::from_utf8_lossy(bytes);
    let json: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default();
    let first = json
        .get("modList")
        .unwrap_or(&json)
        .as_array()
        .and_then(|mods| mods.first());
    let text = |key: &str| {
        first
            .and_then(|m| m.get(key))
            .and_then(Value::as_str)
            .and_then(usable_text)
    };
    let authors = first
        .and_then(|m| m.get("authorList").or_else(|| m.get("authors")))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .filter_map(usable_text)
                .collect()
        })
        .unwrap_or_default();
    Parsed {
        found: ServerMod {
            name: text("name"),
            version: text("version"),
            authors,
            description: text("description"),
            ..ServerMod::default()
        },
        icon_path: text("logoFile").map(|p| p.trim_start_matches('/').to_string()),
    }
}

fn parse_plugin_yml(text: &str) -> Parsed {
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let rest = line.strip_prefix(key)?.strip_prefix(':')?.trim();
            (!rest.starts_with(['[', '|', '>'])).then(|| usable_text(rest))?
        })
    };
    let mut authors: Vec<String> = value("author").into_iter().collect();
    let mut lines = text.lines();
    if let Some(rest) = lines.find_map(|line| line.strip_prefix("authors:")) {
        let rest = rest.trim();
        if rest.starts_with('[') {
            authors.extend(
                rest.trim_matches(['[', ']'])
                    .split(',')
                    .filter_map(usable_text),
            );
        } else {
            authors.extend(
                lines
                    .map(str::trim_start)
                    .take_while(|line| line.starts_with("- "))
                    .filter_map(|line| usable_text(&line[2..])),
            );
        }
    }
    Parsed {
        found: ServerMod {
            name: value("name"),
            version: value("version"),
            authors,
            description: value("description"),
            ..ServerMod::default()
        },
        icon_path: None,
    }
}

async fn read_sources(dir: &Path) -> HashMap<String, ModSource> {
    tokio::fs::read(dir.join(SOURCES_FILE))
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub async fn record_sources(
    dir: &Path,
    folder: &str,
    added: impl IntoIterator<Item = (String, ModSource)>,
) -> LauncherResult<()> {
    let mut sources = read_sources(dir).await;
    sources.extend(added);
    let mut present = HashSet::new();
    if let Ok(mut entries) = tokio::fs::read_dir(dir.join(folder)).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            present.insert(name.trim_end_matches(DISABLED).to_string());
        }
    }
    sources.retain(|file, _| present.contains(file));
    tokio::fs::write(dir.join(SOURCES_FILE), serde_json::to_vec_pretty(&sources)?).await?;
    Ok(())
}

static UNKNOWN: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

pub async fn identify_mods(
    state: &LauncherState,
    dir: &Path,
    folder: &str,
) -> LauncherResult<bool> {
    let sources = read_sources(dir).await;
    let mut pending = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(dir.join(folder)).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let base = name.trim_end_matches(DISABLED).to_string();
            let path = entry.path();
            if base.ends_with(".jar")
                && !sources.contains_key(&base)
                && !UNKNOWN.lock().unwrap().contains(&path)
            {
                pending.push((base, path));
            }
        }
    }
    if pending.is_empty() {
        return Ok(false);
    }

    let ctx = state.services.content();
    let mut found = Vec::new();
    let mut identities = Vec::new();
    for (base, path) in pending {
        let read = path.clone();
        let identity = tokio::task::spawn_blocking(move || {
            std::fs::read(&read).map(|bytes| FileIdentity {
                sha1: polyio::normalize_hash(&polyio::sha1_bytes(&bytes)),
                cf_fingerprint: Some(curseforge_fingerprint(&bytes)),
            })
        })
        .await
        .map_err(std::io::Error::other)??;
        match oneclient_db::dao::artifact::get_release_by_hash(&ctx.db, &identity.sha1).await? {
            Some(row) => found.push((
                base,
                ModSource {
                    provider: ProviderId::from_repr(row.provider as u8)
                        .unwrap_or(ProviderId::Modrinth),
                    project_id: row.project_id,
                    version_id: row.version_id,
                    version_number: row.display_version,
                },
            )),
            None => identities.push((base, path, identity)),
        }
    }

    let lookup: Vec<FileIdentity> = identities.iter().map(|(_, _, i)| i.clone()).collect();
    let (matched, complete) = state
        .services
        .packages
        .lookup_versions_checked(&lookup, &ctx)
        .await?;
    for (base, path, identity) in identities {
        match matched.get(&identity.sha1) {
            Some((provider, version)) => found.push((
                base,
                ModSource {
                    provider: *provider,
                    project_id: version.project_id.clone(),
                    version_id: version.version_id.clone(),
                    version_number: version.version_number.clone(),
                },
            )),
            None if complete => {
                UNKNOWN.lock().unwrap().insert(path);
            }
            None => {}
        }
    }

    let any = !found.is_empty();
    if any {
        record_sources(dir, folder, found).await?;
    }
    Ok(any)
}

pub async fn install_mod(
    state: &LauncherState,
    server: &LocalServer,
    provider_id: ProviderId,
    project_id: &str,
    version_id: &str,
) -> LauncherResult<()> {
    let ctx = state.services.content();
    let provider = state.services.packages.get(provider_id)?;
    let project = provider.get_project(project_id, &ctx).await?;
    let mut version = provider.get_version(project_id, version_id, &ctx).await?;
    if server.plugins() && provider_id == ProviderId::Modrinth {
        let builds = modrinth_plugin_versions(project_id, &server.mc_version, &ctx).await?;
        let pick = builds
            .iter()
            .find(|v| v.version_id == version.version_id)
            .or_else(|| newest_build(&builds))
            .ok_or_else(|| {
                LauncherError::Server(format!(
                    "{} has no Paper build for {}",
                    project.name, server.mc_version
                ))
            })?;
        if pick.version_id != version.version_id {
            version = provider
                .get_version(project_id, &pick.version_id, &ctx)
                .await?;
        }
    }
    let dir = server.dir()?;
    let folder = server.content_folder();
    let mods = dir.join(folder);
    tokio::fs::create_dir_all(&mods).await?;
    if let Err(err) = identify_mods(state, &dir, folder).await {
        tracing::warn!(%err, "could not identify the server's existing mods");
    }

    let sources = read_sources(&dir).await;
    let installed = list_mods(&dir, folder)
        .await
        .into_iter()
        .filter_map(|m| m.source)
        .filter(|s| s.provider == provider_id)
        .map(|s| s.project_id)
        .collect();
    if !server.plugins()
        && client_only(state, provider_id, [&version])
            .await
            .contains(&version.project_id)
    {
        return Err(LauncherError::Server(format!(
            "{} only runs on the client, a server cannot load it",
            project.name
        )));
    }
    let mut resolution = if server.plugins() {
        plugin_dependencies(state, provider_id, &version, &server.mc_version, installed).await
    } else {
        oneclient_content::packages::resolve_required_for(
            provider_id,
            &version,
            &server.mc_version,
            server.loader,
            installed,
            &ctx,
        )
        .await
        .unwrap_or_else(|err| {
            tracing::warn!(%err, "dependency resolution failed, installing the mod alone");
            Default::default()
        })
    };

    if !server.plugins() {
        let blocked = client_only(
            state,
            provider_id,
            resolution.install.iter().map(|dep| &dep.version),
        )
        .await;
        resolution
            .install
            .retain(|dep| !blocked.contains(&dep.version.project_id));
    }

    for (project, version) in std::iter::once((project, version)).chain(
        resolution
            .install
            .into_iter()
            .map(|dep| (dep.project, dep.version)),
    ) {
        let row =
            PackageStore::download_and_cache(provider_id, &project, &version, false, None, &ctx)
                .await?;
        let Some(file_name) = Path::new(&row.file_name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
        else {
            continue;
        };
        for (file, old) in &sources {
            if old.provider == provider_id && old.project_id == project.id {
                remove_if_present(&mods.join(file)).await?;
                remove_if_present(&mods.join(format!("{file}{DISABLED}"))).await?;
            }
        }
        tokio::fs::copy(artifact_absolute_path(&row.path)?, mods.join(&file_name)).await?;
        let source = ModSource {
            provider: provider_id,
            project_id: project.id,
            version_id: version.version_id,
            version_number: version.version_number,
        };
        record_sources(&dir, folder, [(file_name, source)]).await?;
    }
    Ok(())
}

async fn client_only<'a>(
    state: &LauncherState,
    provider_id: ProviderId,
    versions: impl IntoIterator<Item = &'a VersionDetail>,
) -> HashSet<String> {
    match provider_id {
        ProviderId::CurseForge => versions
            .into_iter()
            .filter(|v| super::install::cf_client_only(&v.game_versions))
            .map(|v| v.project_id.clone())
            .collect(),
        ProviderId::Modrinth => {
            let ids: Vec<String> = versions.into_iter().map(|v| v.project_id.clone()).collect();
            if ids.is_empty() {
                return HashSet::new();
            }
            modrinth_client_only(&ids, &state.services.content())
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(%err, "could not ask Modrinth which mods are client-only");
                    HashSet::new()
                })
        }
        ProviderId::Local => HashSet::new(),
    }
}

async fn plugin_builds(
    state: &LauncherState,
    source: &ModSource,
    mc_version: &str,
) -> LauncherResult<Vec<VersionSummary>> {
    let ctx = state.services.content();
    if source.provider == ProviderId::Modrinth {
        return Ok(modrinth_plugin_versions(&source.project_id, mc_version, &ctx).await?);
    }
    Ok(state
        .services
        .packages
        .get(source.provider)?
        .list_versions(&source.project_id, Some(mc_version), None, 0, 50, &ctx)
        .await?
        .items)
}

const MAX_PLUGIN_DEPENDENCIES: usize = 20;

fn required_projects(version: &VersionDetail) -> Vec<String> {
    version
        .dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Required)
        .filter_map(|d| d.project_id.clone())
        .collect()
}

async fn plugin_dependencies(
    state: &LauncherState,
    provider_id: ProviderId,
    root: &VersionDetail,
    mc_version: &str,
    mut seen: HashSet<String>,
) -> DependencyResolution {
    let mut out = DependencyResolution::default();
    let Ok(provider) = state.services.packages.get(provider_id) else {
        return out;
    };
    let ctx = state.services.content();
    seen.insert(root.project_id.clone());
    let mut queue: std::collections::VecDeque<String> = required_projects(root).into();
    while let Some(project_id) = queue.pop_front() {
        if out.install.len() >= MAX_PLUGIN_DEPENDENCIES || !seen.insert(project_id.clone()) {
            continue;
        }
        let source = ModSource {
            provider: provider_id,
            project_id: project_id.clone(),
            version_id: String::new(),
            version_number: String::new(),
        };
        let pick = plugin_builds(state, &source, mc_version)
            .await
            .ok()
            .and_then(|builds| newest_build(&builds).map(|b| b.version_id.clone()));
        let found = match pick {
            Some(version_id) => tokio::join!(
                provider.get_project(&project_id, &ctx),
                provider.get_version(&project_id, &version_id, &ctx)
            ),
            None => {
                out.unresolved.push(project_id);
                continue;
            }
        };
        let (Ok(project), Ok(version)) = found else {
            out.unresolved.push(project_id);
            continue;
        };
        queue.extend(required_projects(&version));
        out.install.push(ResolvedDependency { project, version });
    }
    if !out.unresolved.is_empty() {
        tracing::warn!(missing = ?out.unresolved, "some plugin dependencies have no build for this server");
    }
    out
}

fn newest_build(builds: &[VersionSummary]) -> Option<&VersionSummary> {
    builds
        .iter()
        .filter(|v| matches!(v.release_type, ReleaseType::Release))
        .max_by_key(|v| v.published)
        .or_else(|| builds.iter().max_by_key(|v| v.published))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModUpdate {
    pub file_name: String,
    pub provider: ProviderId,
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
}

pub async fn check_mod_updates(
    state: &LauncherState,
    server: &LocalServer,
) -> LauncherResult<Vec<ModUpdate>> {
    if server.plugins() {
        let mut out = Vec::new();
        for (file_name, source) in sourced_mods(server, false).await? {
            let builds = plugin_builds(state, &source, &server.mc_version).await?;
            if let Some(newest) = newest_build(&builds)
                && newest.version_id != source.version_id
            {
                out.push(ModUpdate {
                    file_name,
                    provider: source.provider,
                    project_id: source.project_id,
                    version_id: newest.version_id.clone(),
                    version_number: newest.version_number.clone(),
                });
            }
        }
        return Ok(out);
    }
    Ok(newest_for_version(state, server, false)
        .await?
        .into_iter()
        .filter_map(|(file_name, source, newest)| {
            let (version_id, version_number) = newest?;
            (version_id != source.version_id).then_some(ModUpdate {
                file_name,
                provider: source.provider,
                project_id: source.project_id,
                version_id,
                version_number,
            })
        })
        .collect())
}

async fn sourced_mods(
    server: &LocalServer,
    enabled_only: bool,
) -> LauncherResult<Vec<(String, ModSource)>> {
    let dir = server.dir()?;
    Ok(list_mods(&dir, server.content_folder())
        .await
        .into_iter()
        .filter(|m| m.enabled || !enabled_only)
        .filter_map(|m| Some((m.file_name, m.source?)))
        .collect())
}

type Newest = (String, ModSource, Option<(String, String)>);

async fn newest_for_version(
    state: &LauncherState,
    server: &LocalServer,
    enabled_only: bool,
) -> LauncherResult<Vec<Newest>> {
    let dir = server.dir()?;
    let folder = server.content_folder();
    let ctx = state.services.content();
    let mut out = Vec::new();
    if server.plugins() {
        for (file_name, source) in sourced_mods(server, enabled_only).await? {
            let Ok(builds) = plugin_builds(state, &source, &server.mc_version).await else {
                continue;
            };
            let newest = newest_build(&builds)
                .map(|b| (b.version_id.clone(), b.version_number.clone()));
            out.push((file_name, source, newest));
        }
        return Ok(out);
    }
    let mut by_provider: HashMap<ProviderId, Vec<(String, InstalledPackage, ModSource)>> =
        HashMap::new();
    for (file_name, source) in sourced_mods(server, enabled_only).await? {
        let Ok(hash) = polyio::sha1_file(dir.join(folder).join(&file_name)).await else {
            continue;
        };
        by_provider.entry(source.provider).or_default().push((
            file_name,
            InstalledPackage {
                hash: polyio::normalize_hash(&hash),
                project_id: source.project_id.clone(),
                content_type: ContentType::Mod,
            },
            source,
        ));
    }
    for (provider_id, mods) in by_provider {
        let Ok(provider) = state.services.packages.get(provider_id) else {
            continue;
        };
        let packages: Vec<InstalledPackage> = mods.iter().map(|(_, p, _)| p.clone()).collect();
        let latest = provider
            .latest_for_game_version(&packages, &server.mc_version, server.loader, &ctx)
            .await?;
        for (file_name, package, source) in mods {
            let newest = latest
                .get(&package.hash)
                .map(|v| (v.version_id.clone(), v.version_number.clone()));
            out.push((file_name, source, newest));
        }
    }
    Ok(out)
}

pub async fn migrate_mods(state: &LauncherState, server: &LocalServer) -> LauncherResult<Vec<String>> {
    if !server.takes_content() {
        return Ok(Vec::new());
    }
    let dir = server.dir()?;
    let folder = server.content_folder();
    let mut disabled = Vec::new();
    for (file_name, source, newest) in newest_for_version(state, server, true).await? {
        match newest {
            Some((version_id, _)) if version_id == source.version_id => {}
            Some((version_id, _)) => {
                if let Err(err) =
                    install_mod(state, server, source.provider, &source.project_id, &version_id)
                        .await
                {
                    tracing::warn!(%err, "could not move {file_name} to the new version");
                    set_mod_enabled(&dir, folder, &file_name, false).await?;
                    disabled.push(file_name);
                }
            }
            None => {
                set_mod_enabled(&dir, folder, &file_name, false).await?;
                disabled.push(file_name);
            }
        }
    }
    Ok(disabled)
}

async fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match tokio::fs::remove_file(path).await {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

/// Renames the jar so the loader skips it, or picks it up again
pub async fn set_mod_enabled(
    dir: &Path,
    folder: &str,
    file_name: &str,
    enabled: bool,
) -> std::io::Result<()> {
    let mods = dir.join(folder);
    let base = file_name.trim_end_matches(DISABLED);
    let (from, to) = if enabled {
        (format!("{base}{DISABLED}"), base.to_string())
    } else {
        (base.to_string(), format!("{base}{DISABLED}"))
    };
    tokio::fs::rename(mods.join(from), mods.join(to)).await
}

#[cfg(test)]
mod tests {
    use super::{ModSource, list_mods, parse_meta, record_sources};
    use oneclient_common::domain::ProviderId;

    #[tokio::test]
    async fn sources_follow_disabled_jars_and_drop_deleted_ones() {
        let tmp = polyio::tempdir().await.expect("dir");
        let dir = tmp.dir_path();
        polyio::create_dir_all(dir.join("mods")).await.unwrap();
        polyio::write(dir.join("mods/a.jar.disabled"), b"a")
            .await
            .unwrap();
        let source = |id: &str| ModSource {
            provider: ProviderId::Modrinth,
            project_id: id.into(),
            version_id: "v".into(),
            version_number: "1.0".into(),
        };
        record_sources(
            dir,
            "mods",
            [
                ("a.jar".into(), source("a")),
                ("gone.jar".into(), source("g")),
            ],
        )
        .await
        .unwrap();

        let mods = list_mods(dir, "mods").await;
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].source, Some(source("a")));
        let saved = tokio::fs::read_to_string(dir.join(super::SOURCES_FILE))
            .await
            .unwrap();
        assert!(!saved.contains("gone.jar"));
    }

    #[test]
    fn newest_plugin_build_prefers_releases() {
        use oneclient_content::packages::{ReleaseType, VersionSummary};
        let build = |id: &str, day: u32, release_type| VersionSummary {
            version_id: id.into(),
            project_id: "p".into(),
            name: id.into(),
            version_number: id.into(),
            published: chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 1, day, 0, 0, 0)
                .unwrap(),
            release_type,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            downloads: 0,
            file_size: 0,
            dependencies: Vec::new(),
        };
        let builds = [
            build("old", 1, ReleaseType::Release),
            build("new", 5, ReleaseType::Release),
            build("beta", 9, ReleaseType::Beta),
        ];
        assert_eq!(
            super::newest_build(&builds).map(|v| v.version_id.as_str()),
            Some("new")
        );
        assert_eq!(
            super::newest_build(&builds[2..]).map(|v| v.version_id.as_str()),
            Some("beta")
        );
        assert!(super::newest_build(&[]).is_none());
    }

    #[test]
    fn reads_old_forge_mcmod_info_in_both_layouts() {
        let flat = parse_meta(
            "mcmod.info",
            "\u{feff}[{\"modid\":\"journeymap\",\"name\":\"JourneyMap\",\"version\":\"5.7.1\",\"description\":\"Maps\",\"authorList\":[\"techbrew\",\"mysticdrew\"],\"logoFile\":\"/assets/jm/logo.png\"}]".as_bytes(),
        );
        assert_eq!(flat.found.name.as_deref(), Some("JourneyMap"));
        assert_eq!(flat.found.authors, ["techbrew", "mysticdrew"]);
        assert_eq!(flat.icon_path.as_deref(), Some("assets/jm/logo.png"));

        let wrapped = parse_meta(
            "mcmod.info",
            br#"{"modListVersion":2,"modList":[{"modid":"x","name":"Thing","version":"${version}"}]}"#,
        );
        assert_eq!(wrapped.found.name.as_deref(), Some("Thing"));
        assert_eq!(wrapped.found.version, None);
    }

    #[test]
    fn reads_bukkit_plugin_yml() {
        let listed = parse_meta(
            "plugin.yml",
            b"name: LuckPerms\nversion: '5.4.102'\nmain: me.lucko.Main\nauthor: Luck\ndescription: \"Permissions\"\ncommands:\n  lp:\n    description: nested\n",
        );
        assert_eq!(listed.found.name.as_deref(), Some("LuckPerms"));
        assert_eq!(listed.found.version.as_deref(), Some("5.4.102"));
        assert_eq!(listed.found.description.as_deref(), Some("Permissions"));
        assert_eq!(listed.found.authors, ["Luck"]);

        let inline = parse_meta("plugin.yml", b"name: A\nauthors: [x, 'y']\n");
        assert_eq!(inline.found.authors, ["x", "y"]);
        let block = parse_meta(
            "plugin.yml",
            b"name: A\nauthors:\n  - x\n  - y\nversion: 1\n",
        );
        assert_eq!(block.found.authors, ["x", "y"]);
    }

    #[test]
    fn reads_fabric_and_forge_metadata() {
        let fabric = parse_meta(
            "fabric.mod.json",
            br#"{"id":"lithium","name":"Lithium","version":"0.15.0","authors":["jellysquid3",{"name":"2No2Name"}],"icon":{"16":"a.png","128":"assets/lithium/icon.png"}}"#,
        );
        assert_eq!(fabric.found.name.as_deref(), Some("Lithium"));
        assert_eq!(fabric.found.version.as_deref(), Some("0.15.0"));
        assert_eq!(fabric.found.authors, ["jellysquid3", "2No2Name"]);
        assert_eq!(fabric.icon_path.as_deref(), Some("assets/lithium/icon.png"));

        let forge = parse_meta(
            "META-INF/mods.toml",
            b"modLoader=\"javafml\"\nlogoFile=\"logo.png\"\n[[mods]]\nmodId=\"jei\"\nversion=\"${file.jarVersion}\"\ndisplayName=\"Just Enough Items\"\nauthors=\"mezz\"\n",
        );
        assert_eq!(forge.found.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(forge.found.version, None);
        assert_eq!(forge.found.authors, ["mezz"]);
        assert_eq!(forge.icon_path.as_deref(), Some("logo.png"));
    }
}
