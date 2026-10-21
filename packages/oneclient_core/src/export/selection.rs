use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::{ExportSource, validate_mod_name};

/// Folder rules apply to descendants. A more specific file/folder rule overrides its parent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportSelection {
    pub rules: BTreeMap<String, bool>,
    pub open_in_prism: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportPreset {
    All,
    Personal,
    Distribution,
}

impl ExportSelection {
    pub fn selected(&self, path: &str) -> bool {
        self.rules
            .iter()
            .filter(|(prefix, _)| descendant(path, prefix))
            .max_by_key(|(prefix, _)| prefix.len())
            .is_some_and(|(_, selected)| *selected)
    }

    pub fn set(&mut self, path: String, selected: bool) {
        self.rules.retain(|child, _| !descendant(child, &path));
        self.rules.insert(path, selected);
    }

    fn may_include(&self, path: &str) -> bool {
        self.selected(path)
            || self
                .rules
                .iter()
                .any(|(child, selected)| *selected && descendant(child, path))
    }

    pub fn initial(items: &[ExportItem]) -> Self {
        Self::preset(ExportPreset::Personal, items)
    }

    pub fn preset(preset: ExportPreset, items: &[ExportItem]) -> Self {
        if preset == ExportPreset::All {
            return Self {
                rules: BTreeMap::from([(String::new(), true)]),
                open_in_prism: false,
            };
        }
        let rules = items
            .iter()
            .filter(|item| {
                matches!(
                    item.path.as_str(),
                    "mods" | "config" | "defaultconfigs" | "oneconfig" | "OneConfig"
                ) || (preset == ExportPreset::Personal
                    && (matches!(
                        item.path.as_str(),
                        "shaderpacks"
                            | "resourcepacks"
                            | "texturepacks"
                            | "saves"
                            | "servers.dat"
                            | "servers.dat_old"
                    ) || (item.name.starts_with("options") && item.name.ends_with(".txt"))))
            })
            .map(|item| (item.path.clone(), true))
            .collect();
        Self {
            rules,
            open_in_prism: false,
        }
    }
}

fn descendant(path: &str, parent: &str) -> bool {
    parent.is_empty()
        || path == parent
        || path
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> Vec<ExportItem> {
        [
            "mods",
            "config",
            "defaultconfigs",
            "oneconfig",
            "OneConfig",
            "options.txt",
            "optionsof.txt",
            "shaderpacks",
            "resourcepacks",
            "texturepacks",
            "saves",
            "servers.dat",
            "servers.dat_old",
            "logs",
            "screenshots",
            "session.json",
        ]
        .into_iter()
        .map(|path| ExportItem {
            path: path.into(),
            name: path.into(),
            folder: !path.contains('.'),
            bytes: 0,
        })
        .collect()
    }

    #[test]
    fn personal_and_distribution_presets_separate_game_data() {
        let personal = ExportSelection::preset(ExportPreset::Personal, &roots());
        let distribution = ExportSelection::preset(ExportPreset::Distribution, &roots());
        for path in [
            "mods/example.jar",
            "config/nested/settings.json",
            "defaultconfigs/mod.toml",
            "oneconfig/settings.json",
            "OneConfig/settings.json",
        ] {
            assert!(personal.selected(path), "{path}");
            assert!(distribution.selected(path), "{path}");
        }
        for path in [
            "shaderpacks/shader.zip",
            "resourcepacks/pack.zip",
            "texturepacks/pack.zip",
            "saves/world/level.dat",
            "servers.dat",
            "servers.dat_old",
            "options.txt",
            "optionsof.txt",
        ] {
            assert!(personal.selected(path), "{path}");
            assert!(!distribution.selected(path), "{path}");
        }
        for path in ["logs/latest.log", "screenshots/image.png", "session.json"] {
            assert!(!personal.selected(path));
            assert!(!distribution.selected(path));
        }
    }

    #[test]
    fn choose_all_includes_unloaded_folders_and_retains_nested_exclusions() {
        let mut all = ExportSelection::preset(ExportPreset::All, &roots());
        assert!(all.selected("new-folder/not-yet-loaded.txt"));
        all.set("saves".into(), false);
        all.set("config/private.json".into(), false);
        assert!(!all.selected("saves/world/level.dat"));
        assert!(!all.selected("config/private.json"));
        assert!(all.selected("config/public.json"));
        let saved: ExportSelection =
            serde_json::from_str(&serde_json::to_string(&all).unwrap()).unwrap();
        assert_eq!(saved, all);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportItem {
    pub path: String,
    pub name: String,
    pub folder: bool,
    pub bytes: u64,
}

fn validate_relative(path: &str) -> Result<()> {
    if path.contains('\\')
        || Path::new(path)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        bail!("Invalid export path: {path}");
    }
    Ok(())
}

fn resolve(source: &ExportSource, path: &str) -> Result<PathBuf> {
    validate_relative(path)?;
    if path == "mods" {
        return Ok(source.mods_dir.clone());
    }
    if let Some(name) = path.strip_prefix("mods/") {
        let materialized = source.mods_dir.join(name);
        if !materialized.exists()
            && let Some(cached) = source.cached_mods.get(name)
        {
            return Ok(cached.clone());
        }
        return Ok(materialized);
    }
    Ok(source.game_dir.join(path))
}

pub fn list_items(source: &ExportSource, parent: &str) -> Result<Vec<ExportItem>> {
    let root = resolve(source, parent)?;
    let mut items = BTreeMap::new();
    if root.is_dir() {
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("A file name is not valid Unicode"))?;
            let path = if parent.is_empty() {
                name.clone()
            } else {
                format!("{parent}/{name}")
            };
            validate_relative(&path)?;
            if path == "mods" && parent.is_empty() {
                continue;
            }
            if path
                .strip_prefix("mods/")
                .is_some_and(|name| source.disabled_mods.contains(name))
            {
                continue;
            }
            let metadata = std::fs::metadata(entry.path()).ok();
            items.insert(
                name.clone(),
                ExportItem {
                    path,
                    name,
                    folder: metadata.as_ref().is_some_and(|m| m.is_dir()),
                    bytes: metadata.map_or(0, |m| m.len()),
                },
            );
        }
    }
    if parent.is_empty() {
        items.insert(
            "mods".into(),
            ExportItem {
                path: "mods".into(),
                name: "mods".into(),
                folder: true,
                bytes: 0,
            },
        );
    } else if parent == "mods" {
        for (name, cached) in &source.cached_mods {
            validate_mod_name(name)?;
            if source.disabled_mods.contains(name) || items.contains_key(name) {
                continue;
            }
            items.insert(
                name.clone(),
                ExportItem {
                    path: format!("mods/{name}"),
                    name: name.clone(),
                    folder: false,
                    bytes: std::fs::metadata(cached).map_or(0, |m| m.len()),
                },
            );
        }
    }
    let mut items: Vec<_> = items.into_values().collect();
    items.sort_by(|a, b| {
        b.folder
            .cmp(&a.folder)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(items)
}

pub(super) fn collect_selected(
    source: &ExportSource,
    destination: &Path,
    dependency_override: Option<&Path>,
    selection: &ExportSelection,
) -> Result<BTreeMap<String, PathBuf>> {
    for path in selection.rules.keys() {
        validate_relative(path)?;
    }
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
    let mut files = BTreeMap::new();
    let mut ancestors = BTreeSet::new();
    for item in list_items(source, "")? {
        visit(source, &item.path, selection, &mut files, &mut ancestors)?;
    }
    let override_name = "config/fabric_loader_dependencies.json";
    if let Some(path) = dependency_override.filter(|p| p.is_file())
        && selection.selected(override_name)
    {
        files.insert(format!(".minecraft/{override_name}"), path.to_path_buf());
    }
    let existing_output = std::fs::canonicalize(destination).unwrap_or(output);
    for path in files.values() {
        if std::fs::canonicalize(path)? == existing_output {
            bail!("The export would overwrite a source file.");
        }
    }
    Ok(files)
}

fn visit(
    source: &ExportSource,
    relative: &str,
    selection: &ExportSelection,
    files: &mut BTreeMap<String, PathBuf>,
    ancestors: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    if !selection.may_include(relative) {
        return Ok(());
    }
    if relative
        .strip_prefix("mods/")
        .is_some_and(|name| source.disabled_mods.contains(name))
    {
        return Ok(());
    }
    let path = resolve(source, relative)?;
    if relative == "mods" && !path.exists() {
        for child in list_items(source, relative)? {
            visit(source, &child.path, selection, files, ancestors)?;
        }
        return Ok(());
    }
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && relative == "mods" => {
            return Ok(());
        }
        Err(err) => return Err(err).with_context(|| format!("Could not read {}", path.display())),
    };
    if metadata.is_file() {
        if selection.selected(relative) {
            files.insert(format!(".minecraft/{relative}"), path);
        }
    } else if metadata.is_dir() {
        let canonical = std::fs::canonicalize(&path)?;
        if !ancestors.insert(canonical.clone()) {
            bail!("Circular folder link at {}", path.display());
        }
        for child in list_items(source, relative)? {
            visit(source, &child.path, selection, files, ancestors)?;
        }
        ancestors.remove(&canonical);
    }
    Ok(())
}
