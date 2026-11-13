use std::path::{Path, PathBuf};
use std::sync::Arc;

use interfrost::api::minecraft::{DownloadType, VersionInfo};
use oneclient_common::GameLoader;
use polyio::Checksum;
use serde::Deserialize;

use super::properties::Properties;
use super::store::{DEFAULT_PORT, LocalServer, accept_eula, save_server};
use crate::state::LauncherState;
use crate::{LauncherError, LauncherResult};

const SERVER_JAR: &str = "server.jar";
const FABRIC_JAR: &str = "fabric-server-launch.jar";
const INSTALLER_JAR: &str = "installer.jar";

fn fail(message: impl Into<String>) -> LauncherError {
    LauncherError::Server(message.into())
}

async fn version_info(state: &LauncherState, mc_version: &str) -> LauncherResult<VersionInfo> {
    let mc = state.services.mc();
    let mut metadata = state.metadata.lock().await;
    let (version, _, _) =
        crate::game::resolve_minecraft_version(&mut metadata, &mc, mc_version).await?;
    drop(metadata);
    Ok(
        crate::game::download_version_info(&mc, None, &version, GameLoader::Vanilla, None, false)
            .await?,
    )
}

pub async fn java_for(state: &LauncherState, server: &LocalServer) -> LauncherResult<PathBuf> {
    if let Some(path) = server.java_path.as_ref().filter(|p| p.exists()) {
        return Ok(path.clone());
    }
    let major = match server.java_major {
        Some(major) => major,
        None => version_info(state, &server.mc_version)
            .await?
            .java_version
            .map(|java| java.major_version)
            .ok_or_else(|| fail("this version does not say which Java it needs"))?,
    };
    Ok(PathBuf::from(
        state
            .java
            .prepare(major, true, true, None)
            .await?
            .absolute_path,
    ))
}

async fn download(
    state: &LauncherState,
    url: &str,
    dest: &Path,
    sha1: Option<&str>,
) -> LauncherResult<()> {
    if let Some(sha1) = sha1
        && oneclient_net::matches_on_disk(dest, sha1).await
    {
        return Ok(());
    }
    oneclient_net::download_verified(
        &state.services.requester,
        &state.services.events,
        url,
        dest,
        sha1.map(Checksum::sha1).as_ref(),
        0,
        None,
    )
    .await?;
    Ok(())
}

async fn get_json<T: serde::de::DeserializeOwned>(
    state: &LauncherState,
    url: &str,
) -> LauncherResult<T> {
    let url: url::Url = url.parse()?;
    Ok(state
        .services
        .requester
        .send_as::<T>(reqwest::Request::new(reqwest::Method::GET, url))
        .await?)
}

async fn loader_version(state: &LauncherState, server: &LocalServer) -> LauncherResult<String> {
    if let Some(version) = server.loader_version.clone().filter(|v| !v.is_empty()) {
        return Ok(version);
    }
    let mc = state.services.mc();
    let mut metadata = state.metadata.lock().await;
    crate::game::get_loader_version(&mut metadata, &mc, &server.mc_version, server.loader, None)
        .await?
        .map(|v| v.id)
        .ok_or_else(|| {
            fail(format!(
                "no {} build for {}",
                server.loader, server.mc_version
            ))
        })
}

/// Downloads the server software into the server folder and records how to start it
pub async fn install_server(
    state: &Arc<LauncherState>,
    server: &mut LocalServer,
) -> LauncherResult<()> {
    let dir = server.dir()?;
    tokio::fs::create_dir_all(&dir).await?;

    let info = version_info(state, &server.mc_version).await?;
    let major = info
        .java_version
        .as_ref()
        .map(|java| java.major_version)
        .ok_or_else(|| fail("this version does not say which Java it needs"))?;
    server.java_major = Some(major);
    let java = java_for(state, server).await?;

    let vanilla = info.downloads.get(&DownloadType::Server).ok_or_else(|| {
        fail(format!(
            "Mojang has no server download for {}",
            server.mc_version
        ))
    })?;

    server.launch = match server.loader {
        GameLoader::Vanilla => {
            download(
                state,
                &vanilla.url,
                &dir.join(SERVER_JAR),
                Some(&vanilla.sha1),
            )
            .await?;
            vec!["-jar".into(), SERVER_JAR.into(), "nogui".into()]
        }
        GameLoader::Fabric => {
            #[derive(Deserialize)]
            struct Installer {
                version: String,
                stable: bool,
            }
            let loader = loader_version(state, server).await?;
            let installers: Vec<Installer> =
                get_json(state, "https://meta.fabricmc.net/v2/versions/installer").await?;
            let installer = installers
                .iter()
                .find(|i| i.stable)
                .or(installers.first())
                .ok_or_else(|| fail("Fabric lists no installer"))?;
            // The launcher reads server.jar from the folder instead of fetching it again
            download(
                state,
                &vanilla.url,
                &dir.join(SERVER_JAR),
                Some(&vanilla.sha1),
            )
            .await?;
            let url = format!(
                "https://meta.fabricmc.net/v2/versions/loader/{}/{loader}/{}/server/jar",
                server.mc_version, installer.version
            );
            download(state, &url, &dir.join(FABRIC_JAR), None).await?;
            server.loader_version = Some(loader);
            vec!["-jar".into(), FABRIC_JAR.into(), "nogui".into()]
        }
        GameLoader::Forge | GameLoader::NeoForge => {
            let loader = loader_version(state, server).await?;
            let url = installer_url(server.loader, &server.mc_version, &loader);
            let installer = dir.join(INSTALLER_JAR);
            download(state, &url, &installer, None).await?;
            run_installer(&java, &dir).await?;
            let _ = tokio::fs::remove_file(&installer).await;
            let _ = tokio::fs::remove_file(dir.join("installer.jar.log")).await;
            server.loader_version = Some(loader);
            forge_launch(&dir).await?
        }
        other => return Err(fail(format!("{other} servers are not supported yet"))),
    };

    write_defaults(server, &dir).await?;
    save_server(server).await
}

fn installer_url(loader: GameLoader, mc_version: &str, version: &str) -> String {
    if loader == GameLoader::NeoForge {
        return format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
        );
    }
    let full = if version.starts_with(&format!("{mc_version}-")) {
        version.to_string()
    } else {
        format!("{mc_version}-{version}")
    };
    format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/forge-{full}-installer.jar"
    )
}

async fn run_installer(java: &Path, dir: &Path) -> LauncherResult<()> {
    let mut command = tokio::process::Command::new(java);
    command
        .arg("-jar")
        .arg(INSTALLER_JAR)
        .arg("--installServer")
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    oneclient_common::process::no_window(command.as_std_mut());
    let status = command.status().await?;
    if !status.success() {
        return Err(fail(format!("the loader installer exited with {status}")));
    }
    Ok(())
}

/// Modern Forge and NeoForge leave an argument file, older Forge a runnable jar
async fn forge_launch(dir: &Path) -> LauncherResult<Vec<String>> {
    let wanted = if cfg!(windows) {
        "win_args.txt"
    } else {
        "unix_args.txt"
    };
    let dir_owned = dir.to_path_buf();
    let args_file =
        tokio::task::spawn_blocking(move || find_file(&dir_owned.join("libraries"), wanted))
            .await
            .ok()
            .flatten();
    if let Some(args) = args_file {
        let relative = args
            .strip_prefix(dir)
            .unwrap_or(&args)
            .to_string_lossy()
            .replace('\\', "/");
        return Ok(vec![format!("@{relative}"), "nogui".into()]);
    }

    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        if (name.starts_with("forge-") || name.starts_with("neoforge-"))
            && name.ends_with(".jar")
            && !name.contains("installer")
        {
            return Ok(vec!["-jar".into(), name, "nogui".into()]);
        }
    }
    Err(fail(
        "the loader installer finished but left nothing to start",
    ))
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if entry.file_name() == name {
            return Some(path);
        }
    }
    None
}

async fn write_defaults(server: &LocalServer, dir: &Path) -> LauncherResult<()> {
    let mut props = Properties::read(dir).await;
    if props.get("server-port").is_none() {
        props.set("server-port", &DEFAULT_PORT.to_string());
    }
    if props.get("motd").is_none() {
        props.set("motd", &server.name);
    }
    props.write(dir).await?;
    if server.eula {
        accept_eula(dir).await?;
    }
    Ok(())
}

/// Copies the instance's enabled mods and its configs, leaving out client-only mods and
/// every mod that needs one. Instance mods live in the package store, not in the game folder
pub async fn copy_from_instance(
    state: &LauncherState,
    cluster_id: i64,
    game_dir: &Path,
    dir: &Path,
) -> LauncherResult<(usize, usize)> {
    let mods = dir.join("mods");
    tokio::fs::create_dir_all(&mods).await?;
    let ctx = state.services.content();
    let linked =
        oneclient_content::packages::PackageStore::list_linked_artifacts(cluster_id, &ctx).await?;

    let mut candidates = Vec::new();
    for link in linked
        .iter()
        .filter(|l| l.enabled && l.content_type == oneclient_common::domain::ContentType::Mod)
    {
        let Some(row) =
            oneclient_db::dao::artifact::get_artifact_by_hash(&ctx.db, &link.hash).await?
        else {
            continue;
        };
        let source = oneclient_content::packages::store::artifact_absolute_path(&row.path)?;
        let Some(name) = Path::new(&link.cluster_file_name).file_name() else {
            continue;
        };
        if !source.exists() {
            tracing::warn!("{} is missing from the package store", source.display());
            continue;
        }
        let meta = mod_meta(&source).await;
        candidates.push((source, name.to_owned(), meta));
    }

    let metas: Vec<ModMeta> = candidates.iter().map(|(_, _, m)| m.clone()).collect();
    let keep = server_side(&metas);
    let mut copied = 0;
    for ((source, name, _), keep) in candidates.iter().zip(&keep) {
        if *keep {
            tokio::fs::copy(source, mods.join(name)).await?;
            copied += 1;
        }
    }

    let config = game_dir.join("config");
    if tokio::fs::try_exists(&config).await.unwrap_or(false) {
        polyio::copy_dir(&config, &dir.join("config"), &[]).await?;
    }
    Ok((copied, candidates.len() - copied))
}

/// What a Fabric or Quilt jar says about itself. Other jars come back empty and are kept
#[derive(Debug, Clone, Default, PartialEq)]
struct ModMeta {
    /// The mod id plus every id it provides
    ids: Vec<String>,
    client_only: bool,
    depends: Vec<String>,
}

fn parse_mod_meta(file: &str, bytes: &[u8]) -> ModMeta {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return ModMeta::default();
    };
    let strings = |value: Option<&serde_json::Value>| -> Vec<String> {
        match value {
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .or_else(|| item.get("id").and_then(|id| id.as_str()))
                        .map(str::to_string)
                })
                .collect(),
            Some(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
            _ => Vec::new(),
        }
    };
    if file == "quilt.mod.json" {
        let loader = json.get("quilt_loader");
        let mut ids: Vec<String> = loader
            .and_then(|l| l.get("id"))
            .and_then(|id| id.as_str())
            .map(str::to_string)
            .into_iter()
            .collect();
        ids.extend(strings(loader.and_then(|l| l.get("provides"))));
        return ModMeta {
            ids,
            client_only: json
                .pointer("/minecraft/environment")
                .and_then(|e| e.as_str())
                == Some("client"),
            depends: strings(loader.and_then(|l| l.get("depends"))),
        };
    }
    let mut ids: Vec<String> = json
        .get("id")
        .and_then(|id| id.as_str())
        .map(str::to_string)
        .into_iter()
        .collect();
    ids.extend(strings(json.get("provides")));
    ModMeta {
        ids,
        client_only: json.get("environment").and_then(|e| e.as_str()) == Some("client"),
        depends: strings(json.get("depends")),
    }
}

/// shortcut: only Fabric and Quilt mods declare their side in the jar, Forge mods are always copied
async fn mod_meta(jar: &Path) -> ModMeta {
    let Ok(entries) = polyio::read_zip_file_entries(jar, |name| {
        name == "fabric.mod.json" || name == "quilt.mod.json"
    })
    .await
    else {
        return ModMeta::default();
    };
    entries
        .iter()
        .map(|(name, bytes)| parse_mod_meta(name, bytes))
        .next()
        .unwrap_or_default()
}

/// Which mods belong on a server: not client-only, and not needing a mod that was left out
fn server_side(mods: &[ModMeta]) -> Vec<bool> {
    let mut keep: Vec<bool> = mods.iter().map(|m| !m.client_only).collect();
    loop {
        let dropped: std::collections::HashSet<&str> = mods
            .iter()
            .zip(&keep)
            .filter(|(_, kept)| !**kept)
            .flat_map(|(m, _)| m.ids.iter().map(String::as_str))
            .collect();
        let mut changed = false;
        for (m, kept) in mods.iter().zip(keep.iter_mut()) {
            if *kept && m.depends.iter().any(|d| dropped.contains(d.as_str())) {
                *kept = false;
                changed = true;
            }
        }
        if !changed {
            return keep;
        }
    }
}

#[derive(Deserialize)]
struct MrpackIndex {
    name: String,
    files: Vec<MrpackFile>,
    dependencies: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct MrpackFile {
    path: String,
    hashes: std::collections::HashMap<String, String>,
    #[serde(default)]
    env: Option<std::collections::HashMap<String, String>>,
    downloads: Vec<String>,
}

pub struct PackInfo {
    pub name: String,
    pub mc_version: String,
    pub loader: GameLoader,
    pub loader_version: Option<String>,
}

pub async fn read_mrpack(path: &Path) -> LauncherResult<PackInfo> {
    let index = mrpack_index(path).await?;
    let (loader, loader_version) = pack_loader(&index.dependencies);
    Ok(PackInfo {
        name: index.name,
        mc_version: index
            .dependencies
            .get("minecraft")
            .cloned()
            .ok_or_else(|| fail("the pack does not name a Minecraft version"))?,
        loader,
        loader_version,
    })
}

async fn mrpack_index(path: &Path) -> LauncherResult<MrpackIndex> {
    let entries = polyio::read_zip_file_entries(path, |name| name == "modrinth.index.json").await?;
    let (_, bytes) = entries
        .into_iter()
        .next()
        .ok_or_else(|| fail("not a Modrinth pack, modrinth.index.json is missing"))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn pack_loader(deps: &std::collections::HashMap<String, String>) -> (GameLoader, Option<String>) {
    [
        ("fabric-loader", GameLoader::Fabric),
        ("quilt-loader", GameLoader::Quilt),
        ("neoforge", GameLoader::NeoForge),
        ("forge", GameLoader::Forge),
    ]
    .into_iter()
    .find_map(|(key, loader)| deps.get(key).map(|v| (loader, Some(v.clone()))))
    .unwrap_or((GameLoader::Vanilla, None))
}

/// Unpacks a .mrpack as a server: server overrides, and every file the server can use
pub async fn unpack_mrpack(state: &LauncherState, pack: &Path, dir: &Path) -> LauncherResult<()> {
    let index = mrpack_index(pack).await?;
    for file in &index.files {
        if file
            .env
            .as_ref()
            .and_then(|env| env.get("server"))
            .is_some_and(|s| s == "unsupported")
        {
            continue;
        }
        let relative = Path::new(&file.path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(fail(format!("the pack lists an unsafe path {}", file.path)));
        }
        let url = file
            .downloads
            .first()
            .ok_or_else(|| fail(format!("{} has no download", file.path)))?;
        download(
            state,
            url,
            &dir.join(relative),
            file.hashes.get("sha1").map(String::as_str),
        )
        .await?;
    }
    for prefix in ["overrides/", "server-overrides/"] {
        polyio::extract_zip_filtered(
            pack,
            dir,
            Some(move |name: &str| name.starts_with(prefix)),
            Some(move |name: &str| name.strip_prefix(prefix).unwrap_or(name).to_string()),
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_installer_url_accepts_both_version_styles() {
        let want = "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar";
        assert_eq!(installer_url(GameLoader::Forge, "1.20.1", "47.2.0"), want);
        assert_eq!(
            installer_url(GameLoader::Forge, "1.20.1", "1.20.1-47.2.0"),
            want
        );
    }

    #[test]
    fn mods_needing_a_client_only_mod_stay_off_the_server() {
        let oneconfig = parse_mod_meta(
            "fabric.mod.json",
            br#"{"id":"oneconfig","provides":["oneconfigv1"],"environment":"client"}"#,
        );
        let hud = parse_mod_meta(
            "fabric.mod.json",
            br#"{"id":"evergreenhud","environment":"*","depends":{"oneconfigv1":">=1.6.2"}}"#,
        );
        let addon = parse_mod_meta(
            "fabric.mod.json",
            br#"{"id":"hudaddon","depends":{"evergreenhud":"*"}}"#,
        );
        let lithium = parse_mod_meta(
            "fabric.mod.json",
            br#"{"id":"lithium","depends":{"fabricloader":"*"}}"#,
        );
        let forge = ModMeta::default();
        assert_eq!(
            server_side(&[oneconfig, hud, addon, lithium, forge]),
            vec![false, false, false, true, true]
        );
    }

    #[test]
    fn pack_loader_reads_mrpack_dependencies() {
        let deps = [("minecraft", "1.21.1"), ("fabric-loader", "0.16.5")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert_eq!(
            pack_loader(&deps),
            (GameLoader::Fabric, Some("0.16.5".into()))
        );
    }
}
