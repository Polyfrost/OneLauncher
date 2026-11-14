use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use oneclient_common::domain::ProviderId;
use oneclient_content::packages::{
    FileIdentity, PackageStore, curseforge_fingerprint, store::artifact_absolute_path,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::LocalServer;
use crate::LauncherResult;
use crate::state::LauncherState;

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
const META_FILES: [&str; 4] = [
    "fabric.mod.json",
    "quilt.mod.json",
    "META-INF/neoforge.mods.toml",
    "META-INF/mods.toml",
];

pub async fn list_mods(dir: &Path) -> Vec<ServerMod> {
    let mut out = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir.join("mods")).await else {
        return out;
    };
    let sources = read_sources(dir).await;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let enabled = file_name.ends_with(".jar");
        if !enabled && !file_name.ends_with(&format!(".jar{DISABLED}")) {
            continue;
        }
        let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
        let mut found = read_meta(&entry.path()).await;
        found.source = sources.get(file_name.trim_end_matches(DISABLED)).cloned();
        found.file_name = file_name;
        found.size = size;
        found.enabled = enabled;
        out.push(found);
    }
    out
}

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

async fn read_sources(dir: &Path) -> HashMap<String, ModSource> {
    tokio::fs::read(dir.join(SOURCES_FILE))
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub async fn record_sources(
    dir: &Path,
    added: impl IntoIterator<Item = (String, ModSource)>,
) -> LauncherResult<()> {
    let mut sources = read_sources(dir).await;
    sources.extend(added);
    let mut present = HashSet::new();
    if let Ok(mut entries) = tokio::fs::read_dir(dir.join("mods")).await {
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

pub async fn identify_mods(state: &LauncherState, dir: &Path) -> LauncherResult<bool> {
    let sources = read_sources(dir).await;
    let mut pending = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(dir.join("mods")).await {
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
        record_sources(dir, found).await?;
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
    let version = provider.get_version(project_id, version_id, &ctx).await?;
    let dir = server.dir()?;
    let mods = dir.join("mods");
    tokio::fs::create_dir_all(&mods).await?;
    if let Err(err) = identify_mods(state, &dir).await {
        tracing::warn!(%err, "could not identify the server's existing mods");
    }

    let sources = read_sources(&dir).await;
    let installed = list_mods(&dir)
        .await
        .into_iter()
        .filter_map(|m| m.source)
        .filter(|s| s.provider == provider_id)
        .map(|s| s.project_id)
        .collect();
    let resolution = oneclient_content::packages::resolve_required_for(
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
    });

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
                let _ = tokio::fs::remove_file(mods.join(file)).await;
                let _ = tokio::fs::remove_file(mods.join(format!("{file}{DISABLED}"))).await;
            }
        }
        tokio::fs::copy(artifact_absolute_path(&row.path)?, mods.join(&file_name)).await?;
        let source = ModSource {
            provider: provider_id,
            project_id: project.id,
            version_id: version.version_id,
            version_number: version.version_number,
        };
        record_sources(&dir, [(file_name, source)]).await?;
    }
    Ok(())
}

/// Renames the jar so the loader skips it, or picks it up again
pub async fn set_mod_enabled(dir: &Path, file_name: &str, enabled: bool) -> std::io::Result<()> {
    let mods = dir.join("mods");
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
        polyio::write(dir.join("mods/a.jar.disabled"), b"a").await.unwrap();
        let source = |id: &str| ModSource {
            provider: ProviderId::Modrinth,
            project_id: id.into(),
            version_id: "v".into(),
            version_number: "1.0".into(),
        };
        record_sources(dir, [("a.jar".into(), source("a")), ("gone.jar".into(), source("g"))])
            .await
            .unwrap();

        let mods = list_mods(dir).await;
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].source, Some(source("a")));
        let saved = tokio::fs::read_to_string(dir.join(super::SOURCES_FILE)).await.unwrap();
        assert!(!saved.contains("gone.jar"));
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
