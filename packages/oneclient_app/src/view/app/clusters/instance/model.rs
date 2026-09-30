use std::collections::HashSet;
use std::sync::Arc;

use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_core::GameVersionKind;

use super::details::DetailsState;
use crate::hooks::GameVersion;

pub const VERSION_KINDS: [(&str, GameVersionKind); 4] = [
    ("Releases", GameVersionKind::Release),
    ("Snapshots", GameVersionKind::Snapshot),
    ("Beta", GameVersionKind::Beta),
    ("Alpha", GameVersionKind::Alpha),
];

pub fn kind_bit(kind: GameVersionKind) -> u8 {
    let index = VERSION_KINDS
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ModpackOrigin {
    Browse,
    File,
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
            Self::Fabric => {
                "Light and quick to start. The widest package selection on modern versions, and the legacy build covers 1.8.9 and older."
            }
            Self::Forge => "The oldest loader. Most 1.12.2 and 1.8.9 packages need it.",
            Self::NeoForge => "Forge's successor, for 1.20.2 and newer.",
            Self::Vanilla => {
                "Minecraft exactly as Mojang ships it. Packages cannot be installed without a loader."
            }
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
    Loader,
    Version,
    Bundles,
    Customize,
    Modpack,
}

impl Step {
    pub fn label(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Loader => "Loader",
            Self::Version => "Version",
            Self::Bundles => "Bundles",
            Self::Customize => "Details",
            Self::Modpack => "Source",
        }
    }
}

const ONECLIENT_STEPS: [Step; 4] = [Step::Type, Step::Version, Step::Bundles, Step::Customize];
const SCRATCH_STEPS: [Step; 4] = [Step::Type, Step::Version, Step::Loader, Step::Customize];
const MODPACK_STEPS: [Step; 2] = [Step::Type, Step::Modpack];

pub fn step_order(choice: TypeChoice) -> &'static [Step] {
    match choice {
        TypeChoice::OneClient => &ONECLIENT_STEPS,
        TypeChoice::Scratch => &SCRATCH_STEPS,
        TypeChoice::Modpack => &MODPACK_STEPS,
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
    pub details: DetailsState,
}
