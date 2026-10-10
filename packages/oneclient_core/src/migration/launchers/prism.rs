//! Prism Launcher and the MultiMC family share one on-disk format: a global
//! `<launcher>.cfg`, an `instances/` folder (relocatable through `InstanceDir`)
//! holding `instance.cfg` + `mmc-pack.json` per instance, and `instgroups.json`

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::LauncherResult;
use oneclient_common::domain::{GameLoader, ProviderId};

use super::ini::Ini;
use super::java;
use super::{ExternalDetection, ExternalInstance, ExternalLauncher, ExternalSettings, LinkedPack};

const ICON_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

fn config_file(launcher: ExternalLauncher) -> &'static str {
    match launcher {
        ExternalLauncher::PolyMc => "polymc.cfg",
        ExternalLauncher::MultiMc => "multimc.cfg",
        _ => "prismlauncher.cfg",
    }
}

pub fn default_roots() -> Vec<(ExternalLauncher, PathBuf)> {
    let mut roots = Vec::new();

    if let Some(dir) = super::env_dir("PRISM_DIR") {
        roots.push((ExternalLauncher::Prism, dir));
    }

    if let Some(data) = directories::BaseDirs::new().map(|d| d.data_dir().to_path_buf()) {
        roots.push((ExternalLauncher::Prism, data.join("PrismLauncher")));
        roots.push((ExternalLauncher::PolyMc, data.join("PolyMC")));
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(dir) =
            super::home_relative(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher")
        {
            roots.push((ExternalLauncher::Prism, dir));
        }
        if let Some(dir) = super::home_relative(".var/app/org.polymc.PolyMC/data/PolyMC") {
            roots.push((ExternalLauncher::PolyMc, dir));
        }
    }

    roots.retain(|(_, root)| root.is_dir());
    roots
}

/// Portable Prism keeps its data in `UserData/` next to the executable, so
/// both the install folder and that subfolder are accepted
pub fn looks_like_root(root: &Path) -> Option<ExternalLauncher> {
    [
        ExternalLauncher::Prism,
        ExternalLauncher::PolyMc,
        ExternalLauncher::MultiMc,
    ]
    .into_iter()
    .find(|launcher| {
        root.join(config_file(*launcher)).is_file()
            || root.join("UserData").join(config_file(*launcher)).is_file()
    })
}

#[tracing::instrument]
pub async fn detect_at(
    launcher: ExternalLauncher,
    root: &Path,
) -> LauncherResult<Option<ExternalDetection>> {
    let root = if root.join("UserData").join(config_file(launcher)).is_file() {
        root.join("UserData")
    } else {
        root.to_path_buf()
    };

    let global = match polyio::read_to_string(root.join(config_file(launcher))).await {
        Ok(text) => Ini::parse(&text),
        Err(_) => Ini::default(),
    };

    let instances_dir = resolve_dir(&root, global.get("InstanceDir").unwrap_or("instances"));
    let icons_dir = resolve_dir(&root, global.get("IconsDir").unwrap_or("icons"));
    if !instances_dir.is_dir() {
        tracing::debug!(dir = %instances_dir.display(), "prism: no instances folder");
        return Ok(None);
    }

    let groups = read_groups(&instances_dir).await;

    let mut instances = Vec::new();
    let mut entries = polyio::read_dir(&instances_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let folder = entry.file_name().to_string_lossy().into_owned();
        // `.tmp`, `_LAUNCHER_TEMP` and friends are the launcher's scratch space
        if folder.starts_with(['.', '_']) || !entry.path().is_dir() {
            continue;
        }

        let source = Source {
            root: &root,
            global: &global,
            icons_dir: &icons_dir,
            groups: &groups,
        };
        match read_instance(launcher, &entry.path(), &folder, &source).await {
            Ok(Some(instance)) => instances.push(instance),
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(folder, error = %err, "prism: skipping unreadable instance");
            }
        }
    }

    instances.sort_by_key(|instance| instance.name.to_lowercase());
    tracing::info!(
        launcher = launcher.display_name(),
        instances = instances.len(),
        "detected third-party launcher"
    );

    Ok(Some(ExternalDetection {
        launcher,
        root,
        instances,
    }))
}

fn resolve_dir(root: &Path, configured: &str) -> PathBuf {
    let path = Path::new(configured);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

#[derive(Deserialize)]
struct GroupsFile {
    #[serde(default)]
    groups: HashMap<String, GroupEntry>,
}

#[derive(Deserialize)]
struct GroupEntry {
    #[serde(default)]
    instances: Vec<String>,
}

/// Instance folder name to the groups it sits in
async fn read_groups(instances_dir: &Path) -> HashMap<String, Vec<String>> {
    let Ok(file) = polyio::read_json::<GroupsFile>(instances_dir.join("instgroups.json")).await
    else {
        return HashMap::new();
    };

    let mut by_instance: HashMap<String, Vec<String>> = HashMap::new();
    for (group, entry) in file.groups {
        for instance in entry.instances {
            by_instance.entry(instance).or_default().push(group.clone());
        }
    }
    for groups in by_instance.values_mut() {
        groups.sort();
    }
    by_instance
}

#[derive(Deserialize)]
struct PackFile {
    #[serde(default)]
    components: Vec<Component>,
}

#[derive(Deserialize)]
struct Component {
    uid: String,
    version: Option<String>,
    #[serde(rename = "cachedVersion")]
    cached_version: Option<String>,
}

impl Component {
    fn version(&self) -> Option<String> {
        self.version
            .clone()
            .or_else(|| self.cached_version.clone())
            .filter(|v| !v.is_empty())
    }
}

fn loader_for_uid(uid: &str) -> Option<GameLoader> {
    match uid {
        "net.fabricmc.fabric-loader" => Some(GameLoader::Fabric),
        "org.quiltmc.quilt-loader" => Some(GameLoader::Quilt),
        "net.minecraftforge" => Some(GameLoader::Forge),
        "net.neoforged" => Some(GameLoader::NeoForge),
        _ => None,
    }
}

/// Launcher-wide state every instance read needs
struct Source<'a> {
    root: &'a Path,
    global: &'a Ini,
    icons_dir: &'a Path,
    groups: &'a HashMap<String, Vec<String>>,
}

async fn read_instance(
    launcher: ExternalLauncher,
    dir: &Path,
    folder: &str,
    source: &Source<'_>,
) -> LauncherResult<Option<ExternalInstance>> {
    let cfg_path = dir.join("instance.cfg");
    if !cfg_path.is_file() {
        return Ok(None);
    }
    let cfg = Ini::parse(&polyio::read_to_string(&cfg_path).await?);

    // Pre-2013 MultiMC instances use a jar-modding layout nothing can launch
    if cfg.get("InstanceType").is_some_and(|t| t != "OneSix") {
        tracing::debug!(folder, "prism: skipping non-OneSix instance");
        return Ok(None);
    }

    let pack: PackFile = match polyio::read_json(dir.join("mmc-pack.json")).await {
        Ok(pack) => pack,
        Err(err) => {
            tracing::debug!(folder, error = %err, "prism: instance has no readable mmc-pack.json");
            return Ok(None);
        }
    };

    let Some(mc_version) = pack
        .components
        .iter()
        .find(|c| c.uid == "net.minecraft")
        .and_then(Component::version)
    else {
        return Ok(None);
    };

    let (loader, loader_version) = pack
        .components
        .iter()
        .find_map(|c| loader_for_uid(&c.uid).map(|loader| (loader, c.version())))
        .unwrap_or((GameLoader::Vanilla, None));

    let game_dir = ["minecraft", ".minecraft"]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_dir())
        .unwrap_or_else(|| dir.join("minecraft"));

    let icon = match cfg.get("iconKey") {
        Some(key) => find_icon(source.icons_dir, key),
        None => None,
    };

    let mut settings = read_settings(&cfg);
    settings.java_major = java_major(&cfg, source.global, source.root).await;

    Ok(Some(ExternalInstance {
        launcher,
        id: folder.to_string(),
        name: cfg.get("name").unwrap_or(folder).to_string(),
        game_dir,
        mc_version,
        loader,
        loader_version,
        icon,
        notes: cfg.get("notes").map(str::to_string),
        groups: source.groups.get(folder).cloned().unwrap_or_default(),
        played_secs: cfg.number("totalTimePlayed").unwrap_or(0),
        settings,
        linked_pack: read_linked_pack(&cfg),
    }))
}

/// Built-in icon keys (`grass`, `flame`, ...) have no file and are skipped
fn find_icon(icons_dir: &Path, key: &str) -> Option<PathBuf> {
    ICON_EXTENSIONS
        .iter()
        .map(|ext| icons_dir.join(format!("{key}.{ext}")))
        .find(|path| path.is_file())
}

/// The instance's own Java when it overrides one, otherwise the launcher's
///
/// Prism records the version it last saw as `JavaVersion`; the install's
/// `release` file covers configs written before it did
async fn java_major(cfg: &Ini, global: &Ini, root: &Path) -> Option<u32> {
    let source = if cfg.flag("OverrideJavaLocation") {
        cfg
    } else {
        global
    };
    if let Some(major) = source.get("JavaVersion").and_then(java::major_from_version) {
        return Some(major);
    }
    let path = source.get("JavaPath")?;
    java::major_from_install(&resolve_dir(root, path)).await
}

fn read_settings(cfg: &Ini) -> ExternalSettings {
    let mut settings = ExternalSettings::default();

    if cfg.flag("OverrideMemory") {
        settings.mem_max = cfg.number("MaxMemAlloc");
    }
    if cfg.flag("OverrideJavaArgs") {
        settings.jvm_args = cfg.get("JvmArgs").map(str::to_string);
    }
    if cfg.flag("OverrideWindow")
        && let (Some(width), Some(height)) = (
            cfg.number("MinecraftWinWidth"),
            cfg.number("MinecraftWinHeight"),
        )
    {
        settings.resolution = Some((width, height));
    }
    if cfg.flag("OverrideCommands") || cfg.flag("OverrideLaunchCmd") {
        settings.hook_pre = portable_hook(cfg.get("PreLaunchCommand"));
        settings.hook_wrapper = portable_hook(cfg.get("WrapperCommand"));
        settings.hook_post = portable_hook(cfg.get("PostExitCommand"));
    }
    if cfg.flag("OverrideEnv")
        && let Some(env) = cfg.get("Env")
    {
        settings.env = parse_env(env);
    }

    settings
}

/// Prism expands `$INST_*` variables before running a hook; OneClient does
/// not, so a hook relying on them would run with the literal text and break
fn portable_hook(command: Option<&str>) -> Option<String> {
    let command = command?.trim();
    if command.is_empty() {
        return None;
    }
    if command.contains("$INST_") {
        tracing::info!(
            command,
            "prism: dropping hook that relies on Prism-only variables"
        );
        return None;
    }
    Some(command.to_string())
}

fn parse_env(json: &str) -> Vec<(String, String)> {
    let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(json) else {
        return Vec::new();
    };
    let mut env: Vec<(String, String)> = map
        .into_iter()
        .filter_map(|(key, value)| Some((key, value.as_str()?.to_string())))
        .collect();
    env.sort();
    env
}

fn read_linked_pack(cfg: &Ini) -> Option<LinkedPack> {
    if !cfg.flag("ManagedPack") {
        return None;
    }
    let provider = match cfg.get("ManagedPackType")? {
        "modrinth" => ProviderId::Modrinth,
        "flame" => ProviderId::CurseForge,
        _ => return None,
    };
    Some(LinkedPack {
        provider,
        project_id: cfg.get("ManagedPackID")?.to_string(),
        version_id: cfg.get("ManagedPackVersionID")?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn write(path: PathBuf, text: &str) {
        polyio::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        polyio::write(path, text.as_bytes()).await.unwrap();
    }

    #[tokio::test]
    async fn reads_a_prism_install() {
        let tmp = polyio::tempdir().await.unwrap();
        let root = tmp.dir_path();

        write(
            root.join("prismlauncher.cfg"),
            "[General]\nInstanceDir=instances\nJavaVersion=17.0.8\n",
        )
        .await;
        write(
            root.join("instances/instgroups.json"),
            r#"{"formatVersion":"1","groups":{"Modded":{"hidden":false,"instances":["fabric"]}}}"#,
        )
        .await;
        write(
            root.join("instances/fabric/instance.cfg"),
            "[General]\nInstanceType=OneSix\nname=Fabulous\niconKey=custom\ntotalTimePlayed=3600\n\
             OverrideMemory=true\nMaxMemAlloc=6144\nOverrideJavaArgs=false\nJvmArgs=-Xss4M\n\
             OverrideCommands=true\nPreLaunchCommand=echo hi\nWrapperCommand=$INST_JAVA wrap\n\
             OverrideEnv=true\nEnv=\"{\\\"FOO\\\": \\\"bar\\\"}\"\n\
             ManagedPack=true\nManagedPackType=modrinth\nManagedPackID=abc\nManagedPackVersionID=def\n\
             OverrideJavaLocation=true\nJavaVersion=21.0.4\n",
        )
        .await;
        write(
            root.join("instances/fabric/mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"1.21.1"},
                {"uid":"net.fabricmc.intermediary","version":"1.21.1"},
                {"uid":"net.fabricmc.fabric-loader","version":"0.16.5"}],"formatVersion":1}"#,
        )
        .await;
        write(root.join("instances/fabric/.minecraft/options.txt"), "x").await;
        write(root.join("icons/custom.png"), "png").await;
        // Scratch folders and stray files are not instances
        write(
            root.join("instances/_LAUNCHER_TEMP/instance.cfg"),
            "name=tmp",
        )
        .await;

        let detection = detect_at(ExternalLauncher::Prism, root)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detection.instances.len(), 1);

        let instance = &detection.instances[0];
        assert_eq!(instance.name, "Fabulous");
        assert_eq!(instance.mc_version, "1.21.1");
        assert_eq!(instance.loader, GameLoader::Fabric);
        assert_eq!(instance.loader_version.as_deref(), Some("0.16.5"));
        assert_eq!(instance.game_dir, root.join("instances/fabric/.minecraft"));
        assert_eq!(instance.icon, Some(root.join("icons/custom.png")));
        assert_eq!(instance.groups, vec!["Modded".to_string()]);
        assert_eq!(instance.played_secs, 3600);
        assert_eq!(instance.settings.mem_max, Some(6144));
        assert_eq!(instance.settings.java_major, Some(21));
        assert_eq!(instance.settings.jvm_args, None);
        assert_eq!(instance.settings.hook_pre.as_deref(), Some("echo hi"));
        assert_eq!(instance.settings.hook_wrapper, None);
        assert_eq!(
            instance.settings.env,
            vec![("FOO".to_string(), "bar".to_string())]
        );
        assert_eq!(
            instance.linked_pack,
            Some(LinkedPack {
                provider: ProviderId::Modrinth,
                project_id: "abc".into(),
                version_id: "def".into(),
            })
        );
    }

    #[tokio::test]
    async fn portable_installs_are_recognised() {
        let tmp = polyio::tempdir().await.unwrap();
        let root = tmp.dir_path();
        write(root.join("UserData/prismlauncher.cfg"), "").await;
        assert_eq!(looks_like_root(root), Some(ExternalLauncher::Prism));

        let tmp = polyio::tempdir().await.unwrap();
        write(tmp.dir_path().join("multimc.cfg"), "").await;
        assert_eq!(
            looks_like_root(tmp.dir_path()),
            Some(ExternalLauncher::MultiMc)
        );
    }
}
