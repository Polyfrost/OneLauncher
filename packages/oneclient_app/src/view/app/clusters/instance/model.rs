use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use freya::prelude::*;
use oneclient_common::domain::{GameLoader, ProviderId};
use oneclient_core::{ExternalDetection, ExternalInstance, FlaggedImportMod, GameVersionKind};

use super::details::DetailsState;
use crate::hooks::GameVersion;
use crate::i18n::t;

pub fn version_kinds() -> [(&'static str, GameVersionKind); 4] {
    [
        (
            t("oneclient.clusters.instance.model.kind.releases"),
            GameVersionKind::Release,
        ),
        (
            t("oneclient.clusters.instance.model.kind.snapshots"),
            GameVersionKind::Snapshot,
        ),
        (
            t("oneclient.clusters.instance.model.kind.beta"),
            GameVersionKind::Beta,
        ),
        (
            t("oneclient.clusters.instance.model.kind.alpha"),
            GameVersionKind::Alpha,
        ),
    ]
}

pub fn kind_bit(kind: GameVersionKind) -> u8 {
    let index = version_kinds()
        .iter()
        .position(|(_, k)| *k == kind)
        .unwrap_or(0);
    1 << index
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TypeChoice {
    OneClient,
    Scratch,
    Modpack,
    Import,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ImportSource {
    File,
    Launcher,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    AsIs,
    Improve,
}

#[derive(Clone, PartialEq, Eq)]
pub enum ModDecision {
    Keep,
    Remove,
    Replace {
        provider: ProviderId,
        project_id: String,
    },
}

pub type DecisionKey = (PathBuf, String);

pub fn decision_for(
    decisions: &HashMap<DecisionKey, ModDecision>,
    game_dir: &Path,
    flagged: &FlaggedImportMod,
) -> ModDecision {
    decisions
        .get(&(game_dir.to_path_buf(), flagged.hash.clone()))
        .cloned()
        .unwrap_or_else(|| default_decision(flagged))
}

fn default_decision(flagged: &FlaggedImportMod) -> ModDecision {
    flagged
        .alternatives
        .iter()
        .find(|alternative| alternative.already_installed)
        .or_else(|| {
            flagged
                .alternatives
                .iter()
                .find(|alternative| alternative.resolved.version_number.is_some())
        })
        .map_or(ModDecision::Keep, |alternative| ModDecision::Replace {
            provider: alternative.resolved.provider,
            project_id: alternative.resolved.project_id.clone(),
        })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ModpackOrigin {
    Browse,
    File,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FolderChoice {
    Shared,
    Dedicated,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoaderChoice {
    Fabric,
    Forge,
    NeoForge,
    Vanilla,
}

impl LoaderChoice {
    pub const ALL: [Self; 4] = [Self::Fabric, Self::Forge, Self::NeoForge, Self::Vanilla];

    pub fn primary(self) -> GameLoader {
        match self {
            Self::Fabric => GameLoader::Fabric,
            Self::Forge => GameLoader::Forge,
            Self::NeoForge => GameLoader::NeoForge,
            Self::Vanilla => GameLoader::Vanilla,
        }
    }

    pub fn name(self) -> String {
        self.primary().to_string()
    }

    pub fn mark(self) -> &'static str {
        match self {
            Self::Fabric => "icons/fabric.png",
            Self::Forge => "icons/forge.png",
            Self::NeoForge => "icons/neo-forge.png",
            Self::Vanilla => "icons/vanilla.png",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Fabric => "FA",
            Self::Forge => "FO",
            Self::NeoForge => "NF",
            Self::Vanilla => "MC",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Fabric => t("oneclient.clusters.instance.model.loader.fabric"),
            Self::Forge => t("oneclient.clusters.instance.model.loader.forge"),
            Self::NeoForge => t("oneclient.clusters.instance.model.loader.neoforge"),
            Self::Vanilla => t("oneclient.clusters.instance.model.loader.vanilla"),
        }
    }

    pub fn resolve(self, available: &[GameLoader]) -> Option<GameLoader> {
        match self {
            Self::Vanilla => Some(GameLoader::Vanilla),
            Self::Fabric => available
                .contains(&GameLoader::Fabric)
                .then_some(GameLoader::Fabric)
                .or_else(|| {
                    available
                        .contains(&GameLoader::Ornithe)
                        .then_some(GameLoader::Ornithe)
                }),
            other => {
                let loader = other.primary();
                available.contains(&loader).then_some(loader)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Type,
    Folder,
    Loader,
    Version,
    Bundles,
    Customize,
    Modpack,
    ImportSource,
    ImportMode,
    Import,
    ImportReview,
}

impl Step {
    pub fn label(self) -> &'static str {
        match self {
            Self::Type => t("oneclient.clusters.instance.model.step.type"),
            Self::Folder => t("oneclient.clusters.instance.model.step.folder"),
            Self::Loader => t("oneclient.clusters.instance.model.step.loader"),
            Self::Version => t("oneclient.clusters.instance.model.step.version"),
            Self::Bundles => t("oneclient.clusters.instance.model.step.bundles"),
            Self::Customize => t("oneclient.clusters.instance.model.step.customize"),
            Self::Modpack => t("oneclient.clusters.instance.model.step.source"),
            Self::ImportSource => t("oneclient.clusters.instance.model.step.source"),
            Self::ImportMode => t("oneclient.clusters.instance.model.step.import_mode"),
            Self::Import => t("oneclient.clusters.instance.model.step.import"),
            Self::ImportReview => t("oneclient.clusters.instance.model.step.import_review"),
        }
    }
}

const ONECLIENT_STEPS: [Step; 5] = [
    Step::Type,
    Step::Folder,
    Step::Version,
    Step::Bundles,
    Step::Customize,
];
const SCRATCH_STEPS: [Step; 4] = [Step::Type, Step::Version, Step::Loader, Step::Customize];
const MODPACK_STEPS: [Step; 2] = [Step::Type, Step::Modpack];
const IMPORT_FILE_STEPS: [Step; 2] = [Step::Type, Step::ImportSource];
const IMPORT_STEPS: [Step; 4] = [
    Step::Type,
    Step::ImportSource,
    Step::ImportMode,
    Step::Import,
];
const IMPROVE_STEPS: [Step; 5] = [
    Step::Type,
    Step::ImportSource,
    Step::ImportMode,
    Step::Import,
    Step::ImportReview,
];

pub fn step_order(choice: TypeChoice, source: ImportSource, mode: ImportMode) -> &'static [Step] {
    match (choice, source, mode) {
        (TypeChoice::OneClient, _, _) => &ONECLIENT_STEPS,
        (TypeChoice::Scratch, _, _) => &SCRATCH_STEPS,
        (TypeChoice::Modpack, _, _) => &MODPACK_STEPS,
        (TypeChoice::Import, ImportSource::File, _) => &IMPORT_FILE_STEPS,
        (TypeChoice::Import, ImportSource::Launcher, ImportMode::AsIs) => &IMPORT_STEPS,
        (TypeChoice::Import, ImportSource::Launcher, ImportMode::Improve) => &IMPROVE_STEPS,
    }
}

#[derive(Clone)]
pub struct VersionRow {
    pub id: String,
    pub meta: String,
    pub badge: Option<String>,
    pub date: String,
}

#[derive(Clone)]
pub enum VersionList {
    Curated(Arc<[VersionRow]>),
    Catalogue {
        all: Arc<[GameVersion]>,
        visible: Arc<[u32]>,
    },
}

impl VersionList {
    pub fn len(&self) -> usize {
        match self {
            Self::Curated(rows) => rows.len(),
            Self::Catalogue { visible, .. } => visible.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn row(&self, index: usize) -> Option<VersionRow> {
        match self {
            Self::Curated(rows) => rows.get(index).cloned(),
            Self::Catalogue { all, visible } => {
                let entry = all.get(*visible.get(index)? as usize)?;
                Some(VersionRow {
                    id: entry.id.clone(),
                    meta: String::new(),
                    badge: (!entry.kind.is_release()).then(|| entry.kind.label().to_string()),
                    date: entry.released.clone(),
                })
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct Wizard {
    pub step: State<usize>,
    pub choice: State<TypeChoice>,
    pub version: State<Option<String>>,
    pub filter: State<u8>,
    pub query: State<String>,
    pub loader: State<LoaderChoice>,
    pub loader_version: State<Option<String>>,
    pub declined: State<Option<HashSet<String>>>,
    pub modpack_origin: State<ModpackOrigin>,
    pub import_source: State<ImportSource>,
    /// Instances ticked on the import step, in the order they were ticked
    pub import_chosen: State<Vec<ExternalInstance>>,
    /// Installs the user pointed at by hand (portable launchers)
    pub import_extra: State<Vec<ExternalDetection>>,
    pub import_mode: State<ImportMode>,
    pub import_decisions: State<HashMap<DecisionKey, ModDecision>>,
    pub folder: State<FolderChoice>,
    pub details: DetailsState,
}
