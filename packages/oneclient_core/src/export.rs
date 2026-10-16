//! Portable instance exports. Files are copied into ZIP entries, never exported as symlinks.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use async_zip::{Compression, ZipEntryBuilder, tokio::write::ZipFileWriter};
use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_content::packages::store::{PackageStore, artifact_absolute_path};
use oneclient_events::{GroupedProgressSession, TaskCategory, TaskPhase};
use tokio::io::AsyncWriteExt;
use tokio_util::compat::TokioAsyncReadCompatExt;

mod selection;
pub use selection::{ExportItem, ExportPreset, ExportSelection};

use crate::{Cluster, LauncherState, settings::GameSettingsProfile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Mods,
    Prism,
}

#[derive(Debug)]
pub struct ExportReport {
    pub path: PathBuf,
    pub files: usize,
    pub mods: usize,
}

/// Also usable without a launcher database, e.g. for importing/exporting an existing game folder.
#[derive(Clone)]
pub struct ExportSource {
    pub name: String,
    pub mc_version: String,
    pub loader: GameLoader,
    pub loader_version: Option<String>,
    pub mods_dir: PathBuf,
    pub game_dir: PathBuf,
    pub settings: GameSettingsProfile,
    /// Enabled cached files which may not yet have been materialized into the mods folder.
    pub cached_mods: BTreeMap<String, PathBuf>,
    pub disabled_mods: BTreeSet<String>,
}

pub fn supports_prism(loader: GameLoader, mc_version: &str) -> bool {
    loader != GameLoader::Ornithe || mc_version == "1.8.9"
}

pub async fn export_cluster(
    state: &LauncherState,
    cluster: &Cluster,
    format: ExportFormat,
    destination: &Path,
) -> Result<ExportReport> {
    export_cluster_with_options(state, cluster, format, destination, None, None).await
}

pub async fn list_cluster_export_items(
    state: &LauncherState,
    cluster: &Cluster,
    parent: String,
) -> Result<Vec<ExportItem>> {
    let (source, _) = prepare_source(state, cluster, ExportFormat::Mods).await?;
    tokio::task::spawn_blocking(move || selection::list_items(&source, &parent)).await?
}

pub async fn export_cluster_with_options(
    state: &LauncherState,
    cluster: &Cluster,
    format: ExportFormat,
    destination: &Path,
    selection: Option<ExportSelection>,
    progress: Option<GroupedProgressSession>,
) -> Result<ExportReport> {
    let (source, dependency_override) = prepare_source(state, cluster, format).await?;
    export_inner(
        source,
        format,
        destination,
        Some(dependency_override),
        selection,
        progress,
    )
    .await
}

async fn prepare_source(
    state: &LauncherState,
    cluster: &Cluster,
    format: ExportFormat,
) -> Result<(ExportSource, PathBuf)> {
    let global = state.settings.read().global_game_settings.clone();
    let settings = oneclient_cluster::profiles::resolve_cluster_profile(
        &state.services.db,
        &global,
        cluster.setting_profile_name.as_deref(),
    )
    .await?;
    let mut loader_version = cluster.mc_loader_version.clone();
    if format == ExportFormat::Prism && cluster.mc_loader.is_modded() && loader_version.is_none() {
        let mut metadata = state.metadata.lock().await;
        loader_version = crate::game::get_loader_version(
            &mut metadata,
            &state.services.mc(),
            &cluster.mc_version,
            cluster.mc_loader,
            None,
        )
        .await?
        .map(|version| version.id);
    }
    let mut source = ExportSource {
        name: cluster.name.clone(),
        mc_version: cluster.mc_version.clone(),
        loader: cluster.mc_loader,
        loader_version,
        mods_dir: oneclient_common::paths::cluster_mods_dir(&cluster.folder_name)?,
        game_dir: cluster.game_dir()?,
        settings,
        cached_mods: BTreeMap::new(),
        disabled_mods: BTreeSet::new(),
    };
    for linked in PackageStore::list_linked_artifacts(cluster.id, &state.services.content()).await?
    {
        if linked.content_type != ContentType::Mod {
            continue;
        }
        validate_mod_name(&linked.cluster_file_name)?;
        if !linked.enabled {
            source.disabled_mods.insert(linked.cluster_file_name);
        } else if let Some(artifact) =
            oneclient_db::dao::artifact::get_artifact_by_hash(&state.services.db, &linked.hash)
                .await?
        {
            source.cached_mods.insert(
                linked.cluster_file_name,
                artifact_absolute_path(&artifact.path)?,
            );
        } else {
            bail!("A mod record is missing. Repair this instance before exporting.");
        }
    }
    // This override belongs to the selected instance, even when its game directory is shared.
    let dependency_override = cluster
        .dir()?
        .join("config/fabric_loader_dependencies.json");
    Ok((source, dependency_override))
}

pub async fn export_instance(
    source: ExportSource,
    format: ExportFormat,
    destination: &Path,
) -> Result<ExportReport> {
    export_inner(source, format, destination, None, None, None).await
}

pub async fn export_instance_selected(
    source: ExportSource,
    destination: &Path,
    selection: ExportSelection,
    progress: Option<GroupedProgressSession>,
) -> Result<ExportReport> {
    export_inner(
        source,
        ExportFormat::Prism,
        destination,
        None,
        Some(selection),
        progress,
    )
    .await
}

async fn export_inner(
    source: ExportSource,
    format: ExportFormat,
    destination: &Path,
    dependency_override: Option<PathBuf>,
    selection: Option<ExportSelection>,
    progress: Option<GroupedProgressSession>,
) -> Result<ExportReport> {
    let template = if format == ExportFormat::Prism {
        prism_files(&source)?
    } else {
        Vec::new()
    };
    let destination = destination.to_path_buf();
    let plan_destination = destination.clone();
    let files = tokio::task::spawn_blocking(move || {
        if format == ExportFormat::Prism
            && let Some(selection) = selection.as_ref()
        {
            return selection::collect_selected(
                &source,
                &plan_destination,
                dependency_override.as_deref(),
                selection,
            );
        }
        collect_files(
            &source,
            format,
            &plan_destination,
            dependency_override.as_deref(),
        )
    })
    .await
    .context("Could not prepare the export")??;
    let mods = files
        .keys()
        .filter(|name| format == ExportFormat::Mods || name.starts_with(".minecraft/mods/"))
        .count();
    let count = template.len() + files.len();
    let mut total_bytes: u64 = template
        .iter()
        .map(|(_, bytes)| bytes.len().max(1) as u64)
        .sum();
    for path in files.values() {
        total_bytes += tokio::fs::metadata(path).await?.len().max(1);
    }
    if let Some(session) = &progress {
        session.expect(TaskCategory::Exports, (count + 1) as u64, total_bytes + 1);
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp =
        TemporaryExport(parent.join(format!(".oneclient-export-{}.tmp", uuid::Uuid::new_v4())));
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp.0)
        .await
        .with_context(|| format!("Could not create an export in {}", parent.display()))?;
    let mut writer = ZipFileWriter::with_tokio(file);
    for (name, bytes) in template {
        let task = progress
            .as_ref()
            .map(|s| s.child(&name, bytes.len().max(1) as u64, TaskCategory::Exports));
        if let Some(task) = &task {
            task.set_phase(TaskPhase::Exporting);
        }
        writer
            .write_entry_whole(
                ZipEntryBuilder::new(name.into(), Compression::Deflate),
                &bytes,
            )
            .await?;
        if let Some(task) = task {
            task.finish();
        }
    }
    for (name, path) in files {
        let file = tokio::fs::File::open(&path)
            .await
            .with_context(|| format!("Could not read {}", path.display()))?;
        let size = file.metadata().await?.len().max(1);
        let task = progress
            .as_ref()
            .map(|s| s.child(&name, size, TaskCategory::Exports));
        if let Some(task) = &task {
            task.set_phase(TaskPhase::Exporting);
        }
        let mut entry = writer
            .write_entry_stream(ZipEntryBuilder::new(name.into(), Compression::Deflate))
            .await?;
        use futures_lite::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let mut file = file.compat();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut copied = 0u64;
        let mut last_update = std::time::Instant::now();
        loop {
            let read = file
                .read(&mut buffer)
                .await
                .with_context(|| format!("Could not read {}", path.display()))?;
            if read == 0 {
                break;
            }
            entry.write_all(&buffer[..read]).await?;
            copied += read as u64;
            if last_update.elapsed() >= std::time::Duration::from_millis(100) {
                if let Some(task) = &task {
                    task.set_progress(copied, None);
                }
                last_update = std::time::Instant::now();
            }
        }
        entry.close().await?;
        if let Some(task) = task {
            task.finish();
        }
    }
    let finalizing = progress
        .as_ref()
        .map(|s| s.child("Writing ZIP directory", 1, TaskCategory::Exports));
    if let Some(task) = &finalizing {
        task.set_phase(TaskPhase::Finalizing);
    }
    let mut file = writer.close().await?.into_inner();
    file.flush().await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(&temp.0, &destination)
        .await
        .with_context(|| format!("Could not save {}", destination.display()))?;
    if let Some(task) = finalizing {
        task.finish();
    }
    if let Some(session) = progress {
        session.finish();
    }
    Ok(ExportReport {
        path: destination,
        files: count,
        mods,
    })
}

struct TemporaryExport(PathBuf);
impl Drop for TemporaryExport {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn validate_mod_name(name: &str) -> Result<()> {
    let mut parts = Path::new(name).components();
    if name.contains(['\\', '/'])
        || !matches!(parts.next(), Some(Component::Normal(_)))
        || parts.next().is_some()
    {
        bail!("Invalid mod file name: {name}");
    }
    Ok(())
}

fn collect_files(
    source: &ExportSource,
    format: ExportFormat,
    destination: &Path,
    dependency_override: Option<&Path>,
) -> Result<BTreeMap<String, PathBuf>> {
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let output = std::fs::canonicalize(parent)?
        .join(destination.file_name().context("Choose a ZIP file name")?);
    for root in [&source.mods_dir, &source.game_dir] {
        if let Ok(root) = std::fs::canonicalize(root)
            && output.starts_with(root)
        {
            bail!("Save the export outside the instance's game and mods folders.");
        }
    }
    let mut mods = BTreeMap::new();
    collect_tree(
        &source.mods_dir,
        "",
        &mut mods,
        &mut BTreeSet::new(),
        &source.disabled_mods,
    )?;
    for name in &source.disabled_mods {
        mods.remove(name);
    }
    for (name, path) in &source.cached_mods {
        validate_mod_name(name)?;
        if source.disabled_mods.contains(name) || mods.contains_key(name) {
            continue;
        }
        if !path.is_file() {
            bail!("Mod {name} is missing. Install or repair this instance before exporting.");
        }
        mods.insert(name.clone(), path.clone());
    }
    let mut files = if format == ExportFormat::Prism {
        mods.into_iter()
            .map(|(name, path)| (format!(".minecraft/mods/{name}"), path))
            .collect()
    } else {
        mods
    };
    if format == ExportFormat::Prism {
        // Copy actual game settings, including dedicated/shared OneConfig layouts. Do not
        // copy launcher databases, accounts, caches, logs, game downloads, or worlds.
        for folder in ["config", "defaultconfigs", "oneconfig", "OneConfig"] {
            collect_tree(
                &source.game_dir.join(folder),
                &format!(".minecraft/{folder}"),
                &mut files,
                &mut BTreeSet::new(),
                &BTreeSet::new(),
            )?;
        }
        if source.game_dir.is_dir() {
            for entry in std::fs::read_dir(&source.game_dir)? {
                let entry = entry?;
                let name = entry.file_name();
                if let Some(name) = name.to_str()
                    && name.starts_with("options")
                    && name.ends_with(".txt")
                    && entry.path().is_file()
                {
                    files.insert(format!(".minecraft/{name}"), entry.path());
                }
            }
        }
        if let Some(path) = dependency_override.filter(|path| path.is_file()) {
            files.insert(
                ".minecraft/config/fabric_loader_dependencies.json".into(),
                path.to_path_buf(),
            );
        }
    }
    // A save dialog may select an existing file. Never overwrite a source file, including a cache file.
    let existing_output = std::fs::canonicalize(destination).unwrap_or(output);
    for path in files.values() {
        if std::fs::canonicalize(path)? == existing_output {
            bail!("The export would overwrite a source file.");
        }
    }
    Ok(files)
}

fn collect_tree(
    root: &Path,
    prefix: &str,
    files: &mut BTreeMap<String, PathBuf>,
    ancestors: &mut BTreeSet<PathBuf>,
    ignored: &BTreeSet<String>,
) -> Result<()> {
    let metadata = match std::fs::metadata(root) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && !root.is_symlink() => {
            return Ok(());
        }
        Err(err) => return Err(err).with_context(|| format!("Could not read {}", root.display())),
    };
    if !metadata.is_dir() {
        bail!("Expected a folder at {}", root.display());
    }
    let canonical = std::fs::canonicalize(root)?;
    if !ancestors.insert(canonical.clone()) {
        bail!("Circular folder link at {}", root.display());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("A file name is not valid Unicode"))?;
        // Backslashes are path separators on the recipient's Windows computer.
        if name.contains('\\') {
            bail!("Cannot export a file with a backslash in its name: {name}");
        }
        let archive_name = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if ignored.contains(&archive_name) {
            continue;
        }
        let metadata = std::fs::metadata(entry.path())
            .with_context(|| format!("Could not read {}", entry.path().display()))?;
        if metadata.is_dir() {
            collect_tree(&entry.path(), &archive_name, files, ancestors, ignored)?;
        } else if metadata.is_file() {
            files.insert(archive_name, entry.path());
        }
    }
    ancestors.remove(&canonical);
    Ok(())
}

fn ini_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
    )
}

fn prism_files(source: &ExportSource) -> Result<Vec<(String, Vec<u8>)>> {
    if !supports_prism(source.loader, &source.mc_version) {
        bail!(
            "The supplied Prism template supports Ornithe Gen2 1.8.9. Export mods.zip for other Ornithe versions."
        );
    }
    let loader_version = source
        .loader_version
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    if source.loader.is_modded() && loader_version.is_none() {
        bail!("Select or install a loader version before exporting this Prism instance.");
    }
    let ornithe = source.loader == GameLoader::Ornithe;
    let mut files = Vec::new();
    let mut pack: serde_json::Value = if ornithe {
        for &(name, bytes) in ORNITHE_FILES {
            files.push((name.into(), bytes.to_vec()));
        }
        serde_json::from_slice(ORNITHE_PACK)?
    } else {
        serde_json::json!({"formatVersion": 1, "components": [
            {"uid": "net.minecraft", "version": source.mc_version, "important": true}
        ]})
    };
    if let Some(version) = loader_version {
        let uid = match source.loader {
            GameLoader::Fabric | GameLoader::Ornithe => "net.fabricmc.fabric-loader",
            GameLoader::Quilt => "org.quiltmc.quilt-loader",
            GameLoader::Forge => "net.minecraftforge",
            GameLoader::NeoForge => "net.neoforged",
            GameLoader::Vanilla => "net.minecraft",
        };
        let version = if source.loader == GameLoader::Forge {
            version
                .strip_prefix(&format!("{}-", source.mc_version))
                .unwrap_or(version)
        } else {
            version
        };
        let components = pack["components"]
            .as_array_mut()
            .context("Invalid Prism template")?;
        if ornithe {
            let component = components
                .iter_mut()
                .find(|component| component["uid"] == uid)
                .context("The Ornithe template has no Fabric loader component")?;
            component["version"] = version.into();
            component["cachedVersion"] = version.into();
        } else {
            if matches!(source.loader, GameLoader::Fabric | GameLoader::Quilt) {
                components.push(serde_json::json!({"uid":"net.fabricmc.intermediary", "version":source.mc_version, "dependencyOnly":true}));
            }
            if source.loader.is_modded() {
                components.push(serde_json::json!({"uid":uid, "version":version}));
            }
        }
    }
    let mut config = if ornithe {
        String::from_utf8(ORNITHE_CONFIG.to_vec())?
    } else {
        "InstanceType=OneSix\n".into()
    };
    config = config
        .lines()
        .filter(|line| !line.starts_with("name=") && !line.starts_with("iconKey="))
        .collect::<Vec<_>>()
        .join("\n");
    config.push_str(&format!("\nname={}\n", ini_string(&source.name)));
    if ornithe {
        config.push_str("iconKey=oneclient\n");
    }
    if let Some(memory) = source.settings.mem_max {
        config.push_str(&format!(
            "OverrideMemory=true\nMinMemAlloc={}\nMaxMemAlloc={memory}\n",
            memory.min(512)
        ));
    }
    if source.settings.resolution.is_some() || source.settings.force_fullscreen.is_some() {
        config.push_str("OverrideWindow=true\n");
        if let Some(resolution) = source.settings.resolution {
            config.push_str(&format!(
                "MinecraftWinWidth={}\nMinecraftWinHeight={}\n",
                resolution.width, resolution.height
            ));
        }
        if let Some(fullscreen) = source.settings.force_fullscreen {
            config.push_str(&format!("LaunchMaximized={fullscreen}\n"));
        }
    }
    files.push(("instance.cfg".into(), config.into_bytes()));
    files.push(("mmc-pack.json".into(), serde_json::to_vec_pretty(&pack)?));
    Ok(files)
}

const ORNITHE_PACK: &[u8] = include_bytes!("../assets/prism/ornithe-gen2-1.8.9/mmc-pack.json");
const ORNITHE_CONFIG: &[u8] = include_bytes!("../assets/prism/ornithe-gen2-1.8.9/instance.cfg");
macro_rules! template_files {
    ($($name:literal),* $(,)?) => { &[$(($name, include_bytes!(concat!("../assets/prism/ornithe-gen2-1.8.9/", $name)) as &[u8])),*] };
}
const ORNITHE_FILES: &[(&str, &[u8])] = template_files![
    "oneclient.png",
    "patches/net.minecraft.json",
    "patches/net.fabricmc.intermediary.json",
    "patches/org.slf4j.slf4j-api.json",
    "patches/org.apache.logging.log4j.log4j-slf4j2-impl.json",
    "patches/org.apache.logging.log4j.log4j-api.json",
    "patches/org.apache.logging.log4j.log4j-core.json",
    "patches/it.unimi.dsi.fastutil.json",
    "patches/com.google.code.gson.gson.json",
    "patches/net.ornithemc.log4j-patch.json",
    "patches/net.sf.jopt-simple.jopt-simple.json",
    "patches/org.lwjgl.json",
    "patches/net.ornithemc.flap.json",
];

#[cfg(test)]
mod tests {
    use super::*;
    use async_zip::base::read1::seek::ZipArchiveReader;
    use futures_lite::io::{AsyncReadExt, Cursor};
    use polyio::testing::ScratchDir;

    fn source(root: &ScratchDir) -> ExportSource {
        let game = root.join("game");
        std::fs::create_dir_all(game.join("mods")).unwrap();
        ExportSource {
            name: "My Ornithe instance".into(),
            mc_version: "1.8.9".into(),
            loader: GameLoader::Ornithe,
            loader_version: Some("0.19.5".into()),
            mods_dir: game.join("mods"),
            game_dir: game,
            settings: GameSettingsProfile::default_global_profile(),
            cached_mods: BTreeMap::new(),
            disabled_mods: BTreeSet::new(),
        }
    }

    async fn archive(path: &Path) -> BTreeMap<String, Vec<u8>> {
        let bytes = tokio::fs::read(path).await.unwrap();
        let mut reader = ZipArchiveReader::open(Cursor::new(bytes)).await.unwrap();
        let names: Vec<_> = reader
            .cdrs()
            .iter()
            .map(|cdr| cdr.insecure_file_name.as_str().unwrap().to_string())
            .collect();
        let mut entries = BTreeMap::new();
        for (index, name) in names.into_iter().enumerate() {
            let mut bytes = Vec::new();
            reader
                .file(index)
                .await
                .unwrap()
                .read_to_end(&mut bytes)
                .await
                .unwrap();
            assert!(entries.insert(name, bytes).is_none(), "duplicate ZIP entry");
        }
        entries
    }

    #[tokio::test]
    async fn recommended_exports_include_personal_data_only_for_personal_use() {
        let root = ScratchDir::new("recommended-exports");
        let source = source(&root);
        for (name, bytes) in [
            ("config/mod.json", b"config".as_slice()),
            ("saves/world/level.dat", b"world".as_slice()),
            ("resourcepacks/pack.zip", b"pack".as_slice()),
            ("shaderpacks/shader.zip", b"shader".as_slice()),
            ("texturepacks/texture.zip", b"texture".as_slice()),
            ("servers.dat", b"servers".as_slice()),
            ("options.txt", b"options".as_slice()),
        ] {
            let path = source.game_dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        std::fs::write(source.mods_dir.join("mod.jar"), b"mod").unwrap();
        let roots = selection::list_items(&source, "").unwrap();
        for preset in [ExportPreset::Personal, ExportPreset::Distribution] {
            let output = root.join(format!("{preset:?}.zip"));
            export_instance_selected(
                source.clone(),
                &output,
                ExportSelection::preset(preset, &roots),
                None,
            )
            .await
            .unwrap();
            let files = archive(&output).await;
            assert_eq!(files[".minecraft/mods/mod.jar"], b"mod");
            assert_eq!(files[".minecraft/config/mod.json"], b"config");
            for personal in [
                "saves/world/level.dat",
                "servers.dat",
                "options.txt",
                "resourcepacks/pack.zip",
                "texturepacks/texture.zip",
                "shaderpacks/shader.zip",
            ] {
                assert_eq!(
                    files.contains_key(&format!(".minecraft/{personal}")),
                    preset == ExportPreset::Personal,
                    "{personal}"
                );
            }
        }
    }

    #[tokio::test]
    async fn selected_export_respects_nested_exclusions_and_saved_choices() {
        let root = ScratchDir::new("selected-export");
        let mut source = source(&root);
        std::fs::create_dir_all(source.game_dir.join("config/nested")).unwrap();
        std::fs::write(source.game_dir.join("config/keep.json"), b"kept").unwrap();
        std::fs::write(source.game_dir.join("config/nested/skip.json"), b"excluded").unwrap();
        std::fs::write(source.game_dir.join("options.txt"), b"options").unwrap();
        std::fs::create_dir_all(source.game_dir.join("saves/world")).unwrap();
        std::fs::write(source.game_dir.join("saves/world/level.dat"), b"world").unwrap();
        std::fs::write(source.mods_dir.join("disabled.jar"), b"disabled").unwrap();
        source.disabled_mods.insert("disabled.jar".into());
        let cached = root.join("cache.jar");
        std::fs::write(&cached, b"cached").unwrap();
        source.cached_mods.insert("cached.jar".into(), cached);
        let mut selection = ExportSelection::initial(&selection::list_items(&source, "").unwrap());
        selection.set("config/nested".into(), false);
        selection.set("saves/world".into(), true);
        selection.open_in_prism = true;
        let selection: ExportSelection =
            serde_json::from_slice(&serde_json::to_vec(&selection).unwrap()).unwrap();
        assert!(selection.open_in_prism);
        let report = export_instance_selected(source, &root.join("selected.zip"), selection, None)
            .await
            .unwrap();
        let files = archive(&report.path).await;
        assert_eq!(files[".minecraft/config/keep.json"], b"kept");
        assert_eq!(files[".minecraft/options.txt"], b"options");
        assert_eq!(files[".minecraft/saves/world/level.dat"], b"world");
        assert_eq!(files[".minecraft/mods/cached.jar"], b"cached");
        assert!(!files.contains_key(".minecraft/mods/disabled.jar"));
        assert!(!files.contains_key(".minecraft/config/nested/skip.json"));
        assert!(files.contains_key("oneclient.png"));
        assert!(String::from_utf8_lossy(&files["instance.cfg"]).contains("iconKey=oneclient\n"));
    }

    #[tokio::test]
    async fn export_progress_names_files_and_finalization_and_ends() {
        use oneclient_events::{Event, GroupedProgressEvent, ProgressEvent};
        let root = ScratchDir::new("export-progress");
        let source = source(&root);
        std::fs::write(source.mods_dir.join("example.jar"), vec![42u8; 1024 * 1024]).unwrap();
        let (events, mut receiver) = oneclient_events::EventBus::channel();
        let progress = GroupedProgressSession::start(&events, "Exporting test");
        let mut selection = ExportSelection::default();
        selection.set("mods".into(), true);
        export_instance_selected(source, &root.join("prism.zip"), selection, Some(progress))
            .await
            .unwrap();
        let mut names = Vec::new();
        let mut expected = 0;
        let mut finished = 0;
        let mut finalizing = false;
        let mut ended = false;
        while let Ok(event) = receiver.try_recv() {
            if let Event::Progress(ProgressEvent::Grouped(event)) = event {
                match event {
                    GroupedProgressEvent::Expect {
                        category: TaskCategory::Exports,
                        count,
                        total,
                        ..
                    } => {
                        expected = count;
                        assert!(total >= 1024 * 1024);
                    }
                    GroupedProgressEvent::AddChild { label, .. } => names.push(label),
                    GroupedProgressEvent::FinishChild { .. } => finished += 1,
                    GroupedProgressEvent::SetChildPhase {
                        phase: TaskPhase::Finalizing,
                        ..
                    } => finalizing = true,
                    GroupedProgressEvent::End { .. } => ended = true,
                    _ => {}
                }
            }
        }
        assert!(names.contains(&".minecraft/mods/example.jar".to_owned()));
        assert!(names.contains(&"Writing ZIP directory".to_owned()));
        assert_eq!(finished, expected);
        assert!(finalizing && ended);
    }

    #[tokio::test]
    async fn selected_paths_reject_traversal_and_preserve_existing_zip() {
        let root = ScratchDir::new("selected-invalid");
        let source = source(&root);
        let output = root.join("previous.zip");
        std::fs::write(&output, b"previous").unwrap();
        for path in [
            "../outside",
            "/absolute",
            "config/../../escape",
            "config\\escape",
        ] {
            let mut selection = ExportSelection::default();
            selection.set(path.into(), true);
            assert!(
                export_instance_selected(source.clone(), &output, selection, None)
                    .await
                    .is_err()
            );
            assert_eq!(std::fs::read(&output).unwrap(), b"previous");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn selected_export_ignores_unselected_broken_links_and_detects_selected_cycles() {
        let root = ScratchDir::new("selected-links");
        let source = source(&root);
        std::os::unix::fs::symlink(root.join("missing"), source.game_dir.join("broken")).unwrap();
        std::fs::create_dir_all(source.game_dir.join("config")).unwrap();
        std::os::unix::fs::symlink(
            source.game_dir.join("config"),
            source.game_dir.join("config/loop"),
        )
        .unwrap();
        let mut selection = ExportSelection::default();
        selection.set("mods".into(), true);
        export_instance_selected(
            source.clone(),
            &root.join("okay.zip"),
            selection.clone(),
            None,
        )
        .await
        .unwrap();
        selection.set("config".into(), true);
        assert!(
            export_instance_selected(
                source.clone(),
                &root.join("cycle.zip"),
                selection.clone(),
                None
            )
            .await
            .is_err()
        );
        selection.set("config".into(), false);
        selection.set("broken".into(), true);
        assert!(
            export_instance_selected(source, &root.join("broken.zip"), selection, None)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn mods_zip_is_flat_and_includes_manual_and_unmaterialized_mods() {
        let root = ScratchDir::new("export-mods");
        let mut source = source(&root);
        std::fs::write(source.mods_dir.join("manual.jar"), b"manual mod").unwrap();
        std::fs::write(source.mods_dir.join("off.jar"), b"disabled mod").unwrap();
        std::fs::create_dir_all(source.mods_dir.join("nested")).unwrap();
        std::fs::write(source.mods_dir.join("nested/legacy.jar"), b"nested mod").unwrap();
        let cache = root.join("cached.jar");
        std::fs::write(&cache, b"cached mod").unwrap();
        source.cached_mods.insert("cached-name.jar".into(), cache);
        source.disabled_mods.insert("off.jar".into());
        let report = export_instance(source, ExportFormat::Mods, &root.join("mods.zip"))
            .await
            .unwrap();
        let files = archive(&report.path).await;
        assert_eq!(report.mods, 3);
        assert_eq!(files["manual.jar"], b"manual mod");
        assert_eq!(files["cached-name.jar"], b"cached mod");
        assert_eq!(files["nested/legacy.jar"], b"nested mod");
        assert!(!files.contains_key("off.jar"));
        assert_eq!(files.len(), 3);
    }

    #[tokio::test]
    async fn ornithe_prism_preserves_patches_and_exports_configs_and_settings() {
        let root = ScratchDir::new("export-prism");
        let mut source = source(&root);
        source.name = "My = instance\nnot_an_ini_key=true".into();
        source.settings.mem_max = Some(4096);
        source.settings.resolution = Some(oneclient_common::Resolution::new(1280, 720));
        std::fs::write(source.mods_dir.join("example.jar"), b"mod bytes").unwrap();
        std::fs::create_dir_all(source.game_dir.join("config/nested")).unwrap();
        std::fs::create_dir_all(source.game_dir.join("oneconfig")).unwrap();
        std::fs::write(
            source.game_dir.join("config/nested/settings.json"),
            b"config bytes",
        )
        .unwrap();
        std::fs::write(source.game_dir.join("oneconfig/keys.json"), b"keybinds").unwrap();
        std::fs::write(source.game_dir.join("options.txt"), b"options bytes").unwrap();
        std::fs::write(source.game_dir.join("optionsof.txt"), b"optifine options").unwrap();
        // Launcher and personal game data must not leak into a portable instance.
        std::fs::write(source.game_dir.join("auth.json"), b"private account").unwrap();
        std::fs::write(
            source.game_dir.join("usercache.json"),
            b"private player history",
        )
        .unwrap();
        std::fs::create_dir_all(source.game_dir.join("logs")).unwrap();
        std::fs::write(source.game_dir.join("logs/latest.log"), b"private log").unwrap();
        let report = export_instance(source, ExportFormat::Prism, &root.join("prism.zip"))
            .await
            .unwrap();
        let files = archive(&report.path).await;
        assert_eq!(files[".minecraft/mods/example.jar"], b"mod bytes");
        assert_eq!(
            files[".minecraft/config/nested/settings.json"],
            b"config bytes"
        );
        assert_eq!(files[".minecraft/oneconfig/keys.json"], b"keybinds");
        assert_eq!(files[".minecraft/options.txt"], b"options bytes");
        assert_eq!(files[".minecraft/optionsof.txt"], b"optifine options");
        for &(name, bytes) in ORNITHE_FILES {
            assert_eq!(files[name], bytes);
        }
        let config = String::from_utf8(files["instance.cfg"].clone()).unwrap();
        assert!(config.contains("MaxMemAlloc=4096\n"));
        assert!(config.contains("MinecraftWinWidth=1280\n"));
        assert!(config.contains("name=\"My = instance\\nnot_an_ini_key=true\""));
        assert!(!config.contains("\nnot_an_ini_key=true"));
        assert!(config.contains("ModDownloadLoaders="));
        let pack: serde_json::Value = serde_json::from_slice(&files["mmc-pack.json"]).unwrap();
        assert!(
            pack["components"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["uid"] == "net.fabricmc.fabric-loader" && c["version"] == "0.19.5")
        );
        assert!(!files.keys().any(|name| name.contains("auth.json")
            || name.contains("usercache.json")
            || name.contains("latest.log")));
    }

    #[tokio::test]
    async fn standard_prism_components_match_the_instance_instead_of_the_ornithe_template() {
        for (loader, uid, version) in [
            (GameLoader::Vanilla, "net.minecraft", "1.21.1"),
            (GameLoader::Fabric, "net.fabricmc.fabric-loader", "0.19.5"),
            (GameLoader::Quilt, "org.quiltmc.quilt-loader", "0.28.1"),
            (GameLoader::Forge, "net.minecraftforge", "47.4.26"),
            (GameLoader::NeoForge, "net.neoforged", "21.1.1"),
        ] {
            let root = ScratchDir::new("export-loader");
            let mut source = source(&root);
            source.mc_version = "1.21.1".into();
            source.loader = loader;
            source.loader_version = (loader != GameLoader::Vanilla).then(|| version.into());
            let report = export_instance(source, ExportFormat::Prism, &root.join("prism.zip"))
                .await
                .unwrap();
            let files = archive(&report.path).await;
            let pack: serde_json::Value = serde_json::from_slice(&files["mmc-pack.json"]).unwrap();
            assert!(
                pack["components"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["uid"] == uid && c["version"] == version)
            );
            assert!(!files.contains_key("patches/net.minecraft.json"));
        }
    }

    #[tokio::test]
    async fn missing_mods_or_unsupported_templates_do_not_overwrite_a_previous_export() {
        let root = ScratchDir::new("export-failed");
        let output = root.join("previous.zip");
        std::fs::write(&output, b"previous export").unwrap();
        let mut source = source(&root);
        source
            .cached_mods
            .insert("missing.jar".into(), root.join("missing.jar"));
        assert!(
            export_instance(source.clone(), ExportFormat::Mods, &output)
                .await
                .is_err()
        );
        source.mc_version = "1.12.2".into();
        assert!(
            export_instance(source, ExportFormat::Prism, &output)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&output).unwrap(), b"previous export");
        assert!(!std::fs::read_dir(root.path()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".oneclient-export-")
        }));
    }

    #[tokio::test]
    async fn exports_cannot_overwrite_their_input_files() {
        let root = ScratchDir::new("export-overwrite");
        let mut source = source(&root);
        let mod_file = source.mods_dir.join("original.jar");
        std::fs::write(&mod_file, b"original bytes").unwrap();
        assert!(
            export_instance(source.clone(), ExportFormat::Mods, &mod_file)
                .await
                .is_err()
        );
        let cached = root.join("cached.jar");
        std::fs::write(&cached, b"cached bytes").unwrap();
        source
            .cached_mods
            .insert("cached.jar".into(), cached.clone());
        assert!(
            export_instance(source, ExportFormat::Mods, &cached)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(mod_file).unwrap(), b"original bytes");
        assert_eq!(std::fs::read(cached).unwrap(), b"cached bytes");
    }

    #[tokio::test]
    async fn a_valid_export_replaces_an_existing_zip_and_rejects_traversal_names() {
        let root = ScratchDir::new("export-replace");
        let mut source = source(&root);
        let output = root.join("mods.zip");
        std::fs::write(&output, b"old zip").unwrap();
        std::fs::write(source.mods_dir.join("example.jar"), b"new mod").unwrap();
        export_instance(source.clone(), ExportFormat::Mods, &output)
            .await
            .unwrap();
        assert_eq!(archive(&output).await["example.jar"], b"new mod");
        source
            .cached_mods
            .insert("../escaping.jar".into(), root.join("cache.jar"));
        assert!(
            export_instance(source, ExportFormat::Mods, &output)
                .await
                .is_err()
        );
        assert_eq!(archive(&output).await["example.jar"], b"new mod");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn linked_files_become_real_bytes_and_directory_cycles_fail_cleanly() {
        use std::os::unix::fs::symlink;
        let root = ScratchDir::new("export-links");
        let mut source = source(&root);
        let cache = root.join("cache.jar");
        std::fs::write(&cache, b"cached jar bytes").unwrap();
        symlink(&cache, source.mods_dir.join("linked.jar")).unwrap();
        symlink(
            root.join("absent.jar"),
            source.mods_dir.join("disabled.jar"),
        )
        .unwrap();
        source.disabled_mods.insert("disabled.jar".into());
        let output = root.join("mods.zip");
        export_instance(source.clone(), ExportFormat::Mods, &output)
            .await
            .unwrap();
        assert_eq!(archive(&output).await["linked.jar"], b"cached jar bytes");
        symlink(&source.mods_dir, source.mods_dir.join("cycle")).unwrap();
        assert!(
            export_instance(source, ExportFormat::Mods, &output)
                .await
                .is_err()
        );
        assert_eq!(archive(&output).await["linked.jar"], b"cached jar bytes");
    }
    #[tokio::test]
    async fn shared_game_configs_do_not_export_another_instances_mods() {
        let root = ScratchDir::new("export-shared");
        let mut source = source(&root);
        let own_mods = source.mods_dir.clone();
        source.game_dir = root.join("shared-game");
        std::fs::create_dir_all(source.game_dir.join("mods/other-instance")).unwrap();
        std::fs::create_dir_all(source.game_dir.join("config")).unwrap();
        std::fs::write(own_mods.join("ours.jar"), b"our mod").unwrap();
        std::fs::write(
            source.game_dir.join("mods/other-instance/theirs.jar"),
            b"other mod",
        )
        .unwrap();
        std::fs::write(source.game_dir.join("config/shared.json"), b"shared config").unwrap();
        let output = root.join("prism.zip");
        export_instance(source, ExportFormat::Prism, &output)
            .await
            .unwrap();
        let files = archive(&output).await;
        assert_eq!(files[".minecraft/mods/ours.jar"], b"our mod");
        assert_eq!(files[".minecraft/config/shared.json"], b"shared config");
        assert!(!files.keys().any(|name| name.contains("theirs.jar")));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_copy_failure_removes_the_partial_zip_and_keeps_the_previous_export() {
        use std::os::unix::fs::PermissionsExt;
        let root = ScratchDir::new("export-copy-failed");
        let source = source(&root);
        let unreadable = source.mods_dir.join("unreadable.jar");
        std::fs::write(&unreadable, b"mod").unwrap();
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0)).unwrap();
        let output = root.join("previous.zip");
        std::fs::write(&output, b"previous export").unwrap();
        let result = export_instance(source, ExportFormat::Prism, &output).await;
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(result.is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"previous export");
        assert!(!std::fs::read_dir(root.path()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".oneclient-export-")
        }));
    }
}
