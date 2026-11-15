use std::path::{Path, PathBuf};
use std::sync::Arc;

use interfrost::api::minecraft::{DownloadType, VersionInfo};
use oneclient_common::GameLoader;
use polyio::Checksum;
use serde::Deserialize;

use super::properties::Properties;
use super::store::{DEFAULT_PORT, LocalServer, Platform, accept_eula, save_server};
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

async fn loader_java(state: &LauncherState, server: &LocalServer) -> Option<u32> {
    if !server.loader.is_modded() {
        return None;
    }
    let mc = state.services.mc();
    let mut metadata = state.metadata.lock().await;
    let (version, _, _) =
        crate::game::resolve_minecraft_version(&mut metadata, &mc, &server.mc_version)
            .await
            .ok()?;
    let loader_version = crate::game::get_loader_version(
        &mut metadata,
        &mc,
        &server.mc_version,
        server.loader,
        None,
    )
    .await
    .ok()
    .flatten();
    drop(metadata);
    crate::game::download_version_info(
        &mc,
        None,
        &version,
        server.loader,
        loader_version.as_ref(),
        false,
    )
    .await
    .ok()?
    .java_version
    .map(|java| java.major_version)
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

pub(super) async fn get_json<T: serde::de::DeserializeOwned>(
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
pub type Progress<'a> = &'a (dyn Fn(String) + Send + Sync);

pub async fn install_server(
    state: &Arc<LauncherState>,
    server: &mut LocalServer,
    progress: Progress<'_>,
) -> LauncherResult<()> {
    let dir = server.dir()?;
    tokio::fs::create_dir_all(&dir).await?;

    let info = version_info(state, &server.mc_version).await?;
    let major = match loader_java(state, server).await {
        Some(major) => major,
        None => info
            .java_version
            .as_ref()
            .map(|java| java.major_version)
            .ok_or_else(|| fail("this version does not say which Java it needs"))?,
    };
    server.java_major = Some(major);
    let java = java_for(state, server).await?;

    let vanilla = info.downloads.get(&DownloadType::Server).ok_or_else(|| {
        fail(format!(
            "Mojang has no server download for {}",
            server.mc_version
        ))
    })?;

    server.launch = match (server.platform, server.loader) {
        (Platform::Paper, _) => {
            let jar = paper_jar(state, &server.mc_version).await?;
            oneclient_net::download_verified(
                &state.services.requester,
                &state.services.events,
                &jar.url,
                &dir.join(PAPER_JAR),
                jar.checksums.get("sha256").map(Checksum::sha256).as_ref(),
                0,
                None,
            )
            .await?;
            vec!["-jar".into(), PAPER_JAR.into(), "nogui".into()]
        }
        (Platform::Loader, GameLoader::Vanilla) => {
            download(
                state,
                &vanilla.url,
                &dir.join(SERVER_JAR),
                Some(&vanilla.sha1),
            )
            .await?;
            vec!["-jar".into(), SERVER_JAR.into(), "nogui".into()]
        }
        (Platform::Loader, GameLoader::Fabric) => {
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
        (Platform::Loader, GameLoader::Forge | GameLoader::NeoForge) => {
            let loader = loader_version(state, server).await?;
            let url = installer_url(server.loader, &server.mc_version, &loader);
            let installer = dir.join(INSTALLER_JAR);
            download(state, &url, &installer, None).await?;
            run_installer(&java, &dir, progress).await?;
            let _ = tokio::fs::remove_file(&installer).await;
            let _ = tokio::fs::remove_file(dir.join("installer.jar.log")).await;
            let _ = tokio::fs::remove_file(dir.join("installer-output.log")).await;
            server.loader_version = Some(loader);
            forge_launch(&dir, server.loader_version.as_deref().unwrap_or_default()).await?
        }
        (Platform::Loader, GameLoader::Ornithe) => {
            download(
                state,
                &vanilla.url,
                &dir.join(SERVER_JAR),
                Some(&vanilla.sha1),
            )
            .await?;
            let (loader, launch) = ornithe_launch(state, server, &dir).await?;
            server.loader_version = Some(loader);
            launch
        }
        (Platform::Loader, other) => {
            return Err(fail(format!("{other} servers are not supported yet")));
        }
    };

    write_defaults(server, &dir).await?;
    save_server(server).await
}

const PAPER_BUILDS: &str = "https://fill.papermc.io/v3/projects/paper/versions";
const PAPER_JAR: &str = "paper.jar";

#[derive(Clone, Deserialize)]
struct PaperDownload {
    url: String,
    #[serde(default)]
    checksums: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct PaperBuild {
    id: u32,
    channel: String,
    #[serde(default)]
    downloads: std::collections::HashMap<String, PaperDownload>,
}

fn pick_paper_build(builds: Vec<PaperBuild>) -> Option<PaperDownload> {
    let newest = |stable: bool| {
        builds
            .iter()
            .filter(|b| !stable || b.channel.eq_ignore_ascii_case("stable"))
            .filter(|b| b.downloads.contains_key("server:default"))
            .max_by_key(|b| b.id)
    };
    newest(true)
        .or_else(|| newest(false))
        .and_then(|b| b.downloads.get("server:default").cloned())
}

async fn paper_jar(state: &LauncherState, mc_version: &str) -> LauncherResult<PaperDownload> {
    let builds: Vec<PaperBuild> =
        get_json(state, &format!("{PAPER_BUILDS}/{mc_version}/builds")).await?;
    pick_paper_build(builds).ok_or_else(|| fail(format!("Paper has no build for {mc_version}")))
}

pub async fn platform_available(
    state: &LauncherState,
    platform: Platform,
    mc_version: &str,
) -> bool {
    match platform {
        Platform::Loader => true,
        Platform::Paper => paper_jar(state, mc_version).await.is_ok(),
    }
}

const ORNITHE_META: &str = "https://meta.ornithemc.net/v3/versions/gen2/fabric-loader";
const ORNITHE_LIBRARIES: &str = "https://meta.ornithemc.net/v3/versions/gen2/libraries";
const ORNITHE_SERVER_JAR: &str = "server-ornithe.jar";

#[derive(Deserialize)]
struct OrnitheProfile {
    #[serde(rename = "mainClass")]
    main_class: String,
    #[serde(default)]
    arguments: OrnitheArguments,
    libraries: Vec<OrnitheLibrary>,
}

#[derive(Default, Deserialize)]
struct OrnitheArguments {
    #[serde(default)]
    jvm: Vec<String>,
}

#[derive(Deserialize)]
struct OrnitheLibrary {
    name: String,
    url: String,
    sha1: Option<String>,
}

fn maven_path(coordinate: &str) -> Option<String> {
    let mut parts = coordinate.split(':');
    let (group, artifact, version) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || [group, artifact, version]
            .iter()
            .any(|p| p.is_empty() || p.contains(['/', '\\']) || p.contains(".."))
    {
        return None;
    }
    Some(format!(
        "{}/{artifact}/{version}/{artifact}-{version}.jar",
        group.replace('.', "/")
    ))
}

async fn ornithe_launch(
    state: &LauncherState,
    server: &LocalServer,
    dir: &Path,
) -> LauncherResult<(String, Vec<String>)> {
    #[derive(Deserialize)]
    struct Build {
        loader: BuildLoader,
    }
    #[derive(Deserialize)]
    struct BuildLoader {
        version: String,
        stable: bool,
    }

    let mc = &server.mc_version;
    let builds: Vec<Build> = get_json(state, &format!("{ORNITHE_META}/{mc}")).await?;
    let wanted = server.loader_version.as_deref().filter(|v| !v.is_empty());
    let loader = builds
        .iter()
        .find(|b| Some(b.loader.version.as_str()) == wanted)
        .or_else(|| builds.iter().find(|b| b.loader.stable))
        .or(builds.first())
        .map(|b| b.loader.version.clone())
        .ok_or_else(|| fail(format!("Ornithe has no loader for {mc}")))?;
    let profile: OrnitheProfile =
        get_json(state, &format!("{ORNITHE_META}/{mc}/{loader}/server/json")).await?;
    let upgrades: Vec<OrnitheLibrary> =
        get_json(state, &format!("{ORNITHE_LIBRARIES}/{mc}")).await?;

    let mut classpath = Vec::new();
    let mut replaced = std::collections::HashSet::new();
    for (index, library) in upgrades.iter().chain(&profile.libraries).enumerate() {
        let path = maven_path(&library.name)
            .ok_or_else(|| fail(format!("Ornithe lists a bad library {}", library.name)))?;
        let relative = format!("libraries/{path}");
        let dest = dir.join(&relative);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let url = format!("{}/{path}", library.url.trim_end_matches('/'));
        download(state, &url, &dest, library.sha1.as_deref()).await?;
        if index < upgrades.len() {
            for name in polyio::zip_entry_names(&dest).await? {
                if let Some(root) = class_root(&name) {
                    replaced.insert(root);
                }
            }
        }
        classpath.push(relative);
    }
    strip_classes(
        &dir.join(SERVER_JAR),
        &dir.join(ORNITHE_SERVER_JAR),
        &replaced,
    )
    .await?;

    let separator = if cfg!(windows) { ";" } else { ":" };
    let mut launch = profile.arguments.jvm;
    launch.push(format!("-Dfabric.gameJarPath={ORNITHE_SERVER_JAR}"));
    launch.push("-cp".into());
    launch.push(classpath.join(separator));
    launch.push(profile.main_class);
    launch.push("nogui".into());
    Ok((loader, launch))
}

fn class_root(entry: &str) -> Option<String> {
    let (package, _) = entry.strip_suffix(".class")?.rsplit_once('/')?;
    Some(package.split('/').take(3).collect::<Vec<_>>().join("/"))
}

async fn strip_classes(
    source: &Path,
    target: &Path,
    roots: &std::collections::HashSet<String>,
) -> LauncherResult<()> {
    let log4j = roots.contains("org/apache/logging");
    let kept = polyio::read_zip_file_entries(source, |name| {
        name != "META-INF/INDEX.LIST"
            && !(log4j && name == "META-INF/log4j-provider.properties")
            && !roots
                .iter()
                .any(|root| name.starts_with(&format!("{root}/")))
    })
    .await?;
    let tmp = target.with_extension("jar.tmp");
    let file = tokio::fs::File::create(&tmp).await?;
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    for (name, bytes) in kept {
        let entry = async_zip::ZipEntryBuilder::new(name.into(), async_zip::Compression::Deflate);
        writer
            .write_entry_whole(entry, &bytes)
            .await
            .map_err(|e| fail(e.to_string()))?;
    }
    writer.close().await.map_err(|e| fail(e.to_string()))?;
    tokio::fs::rename(&tmp, target).await?;
    Ok(())
}

fn installer_url(loader: GameLoader, mc_version: &str, version: &str) -> String {
    let full = if version.starts_with(&format!("{mc_version}-")) {
        version.to_string()
    } else {
        format!("{mc_version}-{version}")
    };
    if loader == GameLoader::NeoForge {
        if mc_version == "1.20.1" {
            return format!(
                "https://maven.neoforged.net/releases/net/neoforged/forge/{full}/forge-{full}-installer.jar"
            );
        }
        return format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
        );
    }
    format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/forge-{full}-installer.jar"
    )
}

async fn run_installer(java: &Path, dir: &Path, progress: Progress<'_>) -> LauncherResult<()> {
    let mut command = tokio::process::Command::new(console_java(java));
    command
        .arg("-jar")
        .arg(INSTALLER_JAR)
        .arg("--installServer")
        .current_dir(dir);
    let status = run_logged(command, &dir.join("installer-output.log"), progress).await?;
    if !status.success() {
        return Err(fail(format!("the loader installer exited with {status}")));
    }
    Ok(())
}

fn console_java(java: &Path) -> PathBuf {
    let console = java.with_file_name(if cfg!(windows) { "java.exe" } else { "java" });
    if java.file_stem().is_some_and(|stem| stem == "javaw") && console.exists() {
        console
    } else {
        java.to_path_buf()
    }
}

async fn run_logged(
    mut command: tokio::process::Command,
    log: &Path,
    progress: Progress<'_>,
) -> LauncherResult<std::process::ExitStatus> {
    use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

    async fn pump(
        stream: Option<impl AsyncRead + Unpin>,
        log: &std::sync::Mutex<std::fs::File>,
        progress: Progress<'_>,
    ) {
        let Some(stream) = stream else {
            return;
        };
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();
        while matches!(reader.read_until(b'\n', &mut buf).await, Ok(n) if n > 0) {
            let line = String::from_utf8_lossy(&buf)
                .trim_end_matches(['\r', '\n'])
                .to_string();
            buf.clear();
            let _ = std::io::Write::write_all(
                &mut *log.lock().unwrap(),
                format!("{line}\n").as_bytes(),
            );
            progress(line);
        }
    }

    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    oneclient_common::process::no_window(command.as_std_mut());
    let mut child = command.spawn()?;
    let log = std::sync::Mutex::new(std::fs::File::create(log)?);
    tokio::join!(
        pump(child.stdout.take(), &log, progress),
        pump(child.stderr.take(), &log, progress),
    );
    Ok(child.wait().await?)
}

/// Modern Forge and NeoForge leave an argument file, older Forge a runnable jar
async fn forge_launch(dir: &Path, loader_version: &str) -> LauncherResult<Vec<String>> {
    let wanted = if cfg!(windows) {
        "win_args.txt"
    } else {
        "unix_args.txt"
    };
    let dir_owned = dir.to_path_buf();
    let found = tokio::task::spawn_blocking(move || {
        let mut found = Vec::new();
        find_files(&dir_owned.join("libraries"), wanted, &mut found);
        found
    })
    .await
    .unwrap_or_default();
    let args_file = pick_for_version(found, loader_version);
    if let Some(args) = args_file {
        let relative = args
            .strip_prefix(dir)
            .unwrap_or(&args)
            .to_string_lossy()
            .replace('\\', "/");
        return Ok(vec![format!("@{relative}"), "nogui".into()]);
    }

    let mut jars = Vec::new();
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        if (name.starts_with("forge-") || name.starts_with("neoforge-"))
            && name.ends_with(".jar")
            && !name.contains("installer")
        {
            jars.push(PathBuf::from(name));
        }
    }
    if let Some(jar) = pick_for_version(jars, loader_version) {
        return Ok(vec![
            "-jar".into(),
            jar.to_string_lossy().to_string(),
            "nogui".into(),
        ]);
    }
    Err(fail(
        "the loader installer finished but left nothing to start",
    ))
}

fn find_files(dir: &Path, name: &str, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_files(&path, name, found);
        } else if entry.file_name() == name {
            found.push(path);
        }
    }
}

fn pick_for_version(mut found: Vec<PathBuf>, loader_version: &str) -> Option<PathBuf> {
    found.sort();
    let matching = (!loader_version.is_empty())
        .then(|| {
            found
                .iter()
                .find(|p| p.to_string_lossy().contains(loader_version))
                .cloned()
        })
        .flatten();
    matching.or_else(|| found.pop())
}

async fn write_defaults(server: &LocalServer, dir: &Path) -> LauncherResult<()> {
    let mut props = Properties::load(dir).await?;
    if props.get("server-port").is_none() {
        props.set("server-port", &DEFAULT_PORT.to_string());
    }
    if props.get("motd").is_none() {
        props.set("motd", &server.name);
    }
    props.write(dir).await?;
    super::ensure_lists(dir, &server.mc_version).await?;
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
        let origin = match (link.provider, &link.project_id, &link.version_id) {
            (Some(provider), Some(project_id), Some(version_id)) => Some(super::ModSource {
                provider,
                project_id: project_id.clone(),
                version_id: version_id.clone(),
                version_number: link.display_version.clone().unwrap_or_default(),
            }),
            _ => None,
        };
        candidates.push((source, name.to_owned(), meta, origin));
    }

    let listed = listed_client_only(
        state,
        candidates.iter().filter_map(|(_, _, _, o)| o.as_ref()),
    )
    .await;
    let metas: Vec<ModMeta> = candidates
        .iter()
        .map(|(_, _, meta, origin)| {
            let mut meta = meta.clone();
            meta.client_only |= origin
                .as_ref()
                .is_some_and(|o| listed.contains(&(o.provider, o.project_id.clone())));
            meta
        })
        .collect();
    let keep = server_side(&metas);
    let mut copied = 0;
    let mut origins = Vec::new();
    for ((source, name, _, origin), keep) in candidates.iter().zip(&keep) {
        if *keep {
            tokio::fs::copy(source, mods.join(name)).await?;
            copied += 1;
            if let Some(origin) = origin {
                origins.push((name.to_string_lossy().to_string(), origin.clone()));
            }
        }
    }
    super::record_sources(dir, "mods", origins).await?;

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

fn parse_forge_meta(text: &str) -> ModMeta {
    let Ok(doc) = text.parse::<toml::Table>() else {
        return ModMeta::default();
    };
    let ids = doc
        .get("mods")
        .and_then(toml::Value::as_array)
        .map(|mods| {
            mods.iter()
                .filter_map(|m| m.get("modId")?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let depends = doc
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .map(|deps| {
            deps.values()
                .filter_map(toml::Value::as_array)
                .flatten()
                .filter(|dep| {
                    let text = |key: &str| dep.get(key).and_then(toml::Value::as_str);
                    let required = dep
                        .get("mandatory")
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(false)
                        || text("type").is_some_and(|t| t.eq_ignore_ascii_case("required"));
                    required && !text("side").is_some_and(|s| s.eq_ignore_ascii_case("client"))
                })
                .filter_map(|dep| dep.get("modId")?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    ModMeta {
        ids,
        client_only: doc
            .get("clientSideOnly")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        depends,
    }
}

const META_FILES: [&str; 4] = [
    "fabric.mod.json",
    "quilt.mod.json",
    "META-INF/mods.toml",
    "META-INF/neoforge.mods.toml",
];

async fn mod_meta(jar: &Path) -> ModMeta {
    let Ok(entries) = polyio::read_zip_file_entries(jar, |name| META_FILES.contains(&name)).await
    else {
        return ModMeta::default();
    };
    entries
        .iter()
        .map(|(name, bytes)| {
            if name.ends_with(".toml") {
                parse_forge_meta(&String::from_utf8_lossy(bytes))
            } else {
                parse_mod_meta(name, bytes)
            }
        })
        .fold(ModMeta::default(), |mut all, meta| {
            all.ids.extend(meta.ids);
            all.depends.extend(meta.depends);
            all.client_only |= meta.client_only;
            all
        })
}

pub(super) fn cf_client_only(game_versions: &[String]) -> bool {
    let tagged = |tag: &str| game_versions.iter().any(|v| v.eq_ignore_ascii_case(tag));
    tagged("client") && !tagged("server")
}

async fn listed_client_only(
    state: &LauncherState,
    origins: impl Iterator<Item = &super::ModSource>,
) -> std::collections::HashSet<(oneclient_common::domain::ProviderId, String)> {
    use oneclient_common::domain::ProviderId;
    let ctx = state.services.content();
    let (mut modrinth, mut curseforge) = (Vec::new(), Vec::new());
    for origin in origins {
        match origin.provider {
            ProviderId::Modrinth => modrinth.push(origin.project_id.clone()),
            ProviderId::CurseForge => curseforge.push(origin.version_id.clone()),
            ProviderId::Local => {}
        }
    }
    let mut out = std::collections::HashSet::new();
    match oneclient_content::packages::modrinth_client_only(&modrinth, &ctx).await {
        Ok(ids) => out.extend(ids.into_iter().map(|id| (ProviderId::Modrinth, id))),
        Err(err) => tracing::warn!(%err, "could not ask Modrinth which mods are client-only"),
    }
    if !curseforge.is_empty() {
        let versions = match state.services.packages.get(ProviderId::CurseForge) {
            Ok(provider) => provider.get_versions(&curseforge, &ctx).await,
            Err(err) => Err(err),
        };
        match versions {
            Ok(versions) => out.extend(
                versions
                    .into_iter()
                    .filter(|v| cf_client_only(&v.game_versions))
                    .map(|v| (ProviderId::CurseForge, v.project_id)),
            ),
            Err(err) => tracing::warn!(%err, "could not ask CurseForge which mods are client-only"),
        }
    }
    out
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

#[must_use]
pub fn supports_loader(loader: GameLoader) -> bool {
    matches!(
        loader,
        GameLoader::Vanilla
            | GameLoader::Fabric
            | GameLoader::Ornithe
            | GameLoader::Forge
            | GameLoader::NeoForge
    )
}

pub async fn read_mrpack(path: &Path) -> LauncherResult<PackInfo> {
    let index = mrpack_index(path).await?;
    let (loader, loader_version) = pack_loader(&index.dependencies);
    if !supports_loader(loader) {
        return Err(fail(format!("{loader} servers are not supported yet")));
    }
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportGuess {
    pub mc_version: Option<String>,
    pub loader: Option<GameLoader>,
    pub loader_version: Option<String>,
    pub platform: Platform,
    pub port: Option<u16>,
}

pub async fn detect_import(dir: &Path) -> ImportGuess {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || guess_import(&dir))
        .await
        .unwrap_or_default()
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn neoforge_mc(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let (major, minor) = (
        parts.next()?.parse::<u32>().ok()?,
        parts.next()?.parse::<u32>().ok()?,
    );
    Some(match (major, minor) {
        (major, 0) if major < 25 => format!("1.{major}"),
        (major, minor) if major < 25 => format!("1.{major}.{minor}"),
        (major, minor) => format!("{major}.{minor}"),
    })
}

fn guess_import(dir: &Path) -> ImportGuess {
    let root = names_in(dir);
    let has = |name: &str| root.iter().any(|n| n == name);
    let mut guess = ImportGuess {
        port: std::fs::read(dir.join(super::properties::FILE))
            .ok()
            .and_then(|bytes| {
                Properties::parse(&String::from_utf8_lossy(&bytes))
                    .get("server-port")
                    .and_then(|port| port.trim().parse().ok())
            }),
        ..ImportGuess::default()
    };

    let logged = std::fs::read_to_string(dir.join("logs").join("latest.log"))
        .ok()
        .and_then(|log| {
            log.lines().find_map(|line| {
                line.split_once("Starting minecraft server version ")
                    .map(|(_, version)| version.trim().to_string())
            })
        });
    let unpacked = names_in(&dir.join("versions"))
        .into_iter()
        .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
        .next_back()
        .or_else(|| {
            names_in(&dir.join("cache")).into_iter().find_map(|n| {
                n.strip_prefix("mojang_")?
                    .strip_suffix(".jar")
                    .map(str::to_string)
            })
        })
        .or_else(|| {
            names_in(&dir.join("libraries/net/minecraft/server"))
                .into_iter()
                .next_back()
                .map(|v| v.split('-').next().unwrap_or(&v).to_string())
        });

    let forge = names_in(&dir.join("libraries/net/minecraftforge/forge"));
    let neoforge = names_in(&dir.join("libraries/net/neoforged/neoforge"));
    let old_neoforge = names_in(&dir.join("libraries/net/neoforged/forge"));
    let fabric = names_in(&dir.join("libraries/net/fabricmc/fabric-loader"));
    let paper = root
        .iter()
        .any(|n| n.starts_with("paper") && n.ends_with(".jar"))
        || names_in(&dir.join("cache"))
            .iter()
            .any(|n| n.starts_with("mojang_"));

    let mut from_loader = None;
    if paper {
        guess.platform = Platform::Paper;
        guess.loader = Some(GameLoader::Vanilla);
    } else if let Some(version) = neoforge.last() {
        guess.loader = Some(GameLoader::NeoForge);
        guess.loader_version = Some(version.clone());
        from_loader = neoforge_mc(version);
    } else if let Some((mc, version)) = old_neoforge
        .last()
        .or(forge.last())
        .and_then(|v| v.split_once('-'))
    {
        guess.loader = Some(if old_neoforge.is_empty() {
            GameLoader::Forge
        } else {
            GameLoader::NeoForge
        });
        guess.loader_version = Some(version.to_string());
        from_loader = Some(mc.to_string());
    } else if has("fabric-server-launch.jar") || has(".fabric") {
        guess.loader = Some(GameLoader::Fabric);
        guess.loader_version = fabric.last().cloned();
    } else if has("server.jar") {
        guess.loader = Some(GameLoader::Vanilla);
    }
    guess.mc_version = logged.or(from_loader).or(unpacked);
    guess
}

const PACK_HOSTS: [&str; 4] = [
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

fn allowed_pack_download(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| PACK_HOSTS.contains(&host))
    })
}

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
            .iter()
            .find(|url| allowed_pack_download(url))
            .ok_or_else(|| {
                fail(format!(
                    "{} has no download from a host Modrinth packs may use",
                    file.path
                ))
            })?;
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
    fn class_roots_cover_the_library_not_the_game() {
        assert_eq!(
            class_root("org/apache/logging/log4j/Logger.class").as_deref(),
            Some("org/apache/logging")
        );
        assert_eq!(
            class_root("joptsimple/OptionParser.class").as_deref(),
            Some("joptsimple")
        );
        assert_eq!(class_root("Main.class"), None);
        assert_eq!(class_root("META-INF/MANIFEST.MF"), None);
    }

    #[test]
    fn maven_coordinates_become_repository_paths() {
        assert_eq!(
            maven_path("net.ornithemc:calamus-intermediary-gen2:1.8.9").as_deref(),
            Some(
                "net/ornithemc/calamus-intermediary-gen2/1.8.9/calamus-intermediary-gen2-1.8.9.jar"
            )
        );
        assert_eq!(maven_path("net.fabricmc:../evil:1.0"), None);
        assert_eq!(maven_path("a:b:c:natives"), None);
    }

    #[test]
    fn neoforge_1_20_1_lives_under_the_old_forge_artifact() {
        let want = "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar";
        assert_eq!(
            installer_url(GameLoader::NeoForge, "1.20.1", "47.1.106"),
            want
        );
        assert_eq!(
            installer_url(GameLoader::NeoForge, "1.20.1", "1.20.1-47.1.106"),
            want
        );
        assert_eq!(
            installer_url(GameLoader::NeoForge, "1.21.1", "21.1.77"),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.77/neoforge-21.1.77-installer.jar"
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
    fn forge_mods_toml_marks_client_mods_and_their_dependents() {
        let minimap = parse_forge_meta(
            "modLoader=\"javafml\"\nclientSideOnly=true\n[[mods]]\nmodId=\"minimap\"\nversion=\"${file.jarVersion}\"\n",
        );
        let addon = parse_forge_meta(
            "[[mods]]\nmodId=\"minimapaddon\"\n[[dependencies.minimapaddon]]\nmodId=\"minimap\"\nmandatory=true\nside=\"BOTH\"\n[[dependencies.minimapaddon]]\nmodId=\"jei\"\ntype=\"required\"\nside=\"CLIENT\"\n",
        );
        let jei = parse_forge_meta("[[mods]]\nmodId=\"jei\"\n");
        assert!(minimap.client_only);
        assert_eq!(addon.depends, ["minimap"]);
        assert_eq!(
            server_side(&[minimap, addon, jei]),
            vec![false, false, true]
        );
    }

    #[test]
    fn curseforge_client_tag_without_server_tag_means_client_only() {
        let tags = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(cf_client_only(&tags(&["1.20.1", "Client"])));
        assert!(!cf_client_only(&tags(&["1.20.1", "Client", "Server"])));
        assert!(!cf_client_only(&tags(&["1.20.1"])));
    }

    #[tokio::test]
    async fn installers_run_on_the_console_java_next_to_javaw() {
        let tmp = polyio::tempdir().await.expect("dir");
        let bin = tmp.dir_path();
        let (javaw, java) = if cfg!(windows) {
            (bin.join("javaw.exe"), bin.join("java.exe"))
        } else {
            (bin.join("javaw"), bin.join("java"))
        };
        polyio::write(&javaw, b"").await.unwrap();
        assert_eq!(console_java(&javaw), javaw);
        polyio::write(&java, b"").await.unwrap();
        assert_eq!(console_java(&javaw), java);
        assert_eq!(console_java(&java), java);
    }

    #[test]
    fn forge_args_file_matches_the_installed_loader_version() {
        let found = vec![
            PathBuf::from("libraries/net/minecraftforge/forge/1.20.1-47.2.0/win_args.txt"),
            PathBuf::from("libraries/net/minecraftforge/forge/1.20.1-47.3.0/win_args.txt"),
        ];
        assert_eq!(
            pick_for_version(found.clone(), "47.2.0"),
            Some(found[0].clone())
        );
        assert_eq!(pick_for_version(found.clone(), ""), Some(found[1].clone()));
        assert_eq!(pick_for_version(Vec::new(), "47.2.0"), None);
    }

    #[test]
    fn modpack_files_only_download_from_allowed_hosts() {
        assert!(allowed_pack_download(
            "https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar"
        ));
        assert!(allowed_pack_download(
            "https://github.com/a/b/releases/download/1/x.jar"
        ));
        assert!(!allowed_pack_download("http://cdn.modrinth.com/data/x.jar"));
        assert!(!allowed_pack_download(
            "https://cdn.modrinth.com.evil.io/x.jar"
        ));
        assert!(!allowed_pack_download("https://example.com/x.jar"));
    }

    #[tokio::test]
    async fn imported_folders_reveal_their_server_software() {
        let tmp = polyio::tempdir().await.expect("dir");
        let root = tmp.dir_path();

        let paper = root.join("paper");
        polyio::create_dir_all(paper.join("cache")).await.unwrap();
        polyio::write(paper.join("paper-1.21.4-232.jar"), b"")
            .await
            .unwrap();
        polyio::write(paper.join("cache/mojang_1.21.4.jar"), b"")
            .await
            .unwrap();
        polyio::write(paper.join("server.properties"), b"server-port=25570\n")
            .await
            .unwrap();
        let guess = detect_import(&paper).await;
        assert_eq!(guess.platform, Platform::Paper);
        assert_eq!(guess.mc_version.as_deref(), Some("1.21.4"));
        assert_eq!(guess.port, Some(25570));

        let forge = root.join("forge");
        polyio::create_dir_all(forge.join("libraries/net/minecraftforge/forge/1.20.1-47.2.0"))
            .await
            .unwrap();
        let guess = detect_import(&forge).await;
        assert_eq!(guess.loader, Some(GameLoader::Forge));
        assert_eq!(guess.loader_version.as_deref(), Some("47.2.0"));
        assert_eq!(guess.mc_version.as_deref(), Some("1.20.1"));

        let fabric = root.join("fabric");
        polyio::create_dir_all(fabric.join("logs")).await.unwrap();
        polyio::write(fabric.join("fabric-server-launch.jar"), b"")
            .await
            .unwrap();
        polyio::write(
            fabric.join("logs/latest.log"),
            b"[12:00:00] [Server thread/INFO]: Starting minecraft server version 1.21.1\n",
        )
        .await
        .unwrap();
        let guess = detect_import(&fabric).await;
        assert_eq!(guess.loader, Some(GameLoader::Fabric));
        assert_eq!(guess.mc_version.as_deref(), Some("1.21.1"));

        assert_eq!(neoforge_mc("21.1.77").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_mc("21.0.10").as_deref(), Some("1.21"));
    }

    #[test]
    fn paper_prefers_the_newest_stable_build() {
        let builds: Vec<PaperBuild> = serde_json::from_str(
            r#"[
                {"id": 40, "channel": "BETA", "downloads": {"server:default": {"url": "b40"}}},
                {"id": 38, "channel": "STABLE", "downloads": {"server:default": {"url": "s38", "checksums": {"sha256": "aa"}}}},
                {"id": 39, "channel": "STABLE", "downloads": {"server:default": {"url": "s39"}}}
            ]"#,
        )
        .unwrap();
        assert_eq!(
            pick_paper_build(builds).map(|d| d.url).as_deref(),
            Some("s39")
        );
        let only_beta: Vec<PaperBuild> = serde_json::from_str(
            r#"[{"id": 2, "channel": "BETA", "downloads": {"server:default": {"url": "b2"}}}]"#,
        )
        .unwrap();
        assert_eq!(
            pick_paper_build(only_beta).map(|d| d.url).as_deref(),
            Some("b2")
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
