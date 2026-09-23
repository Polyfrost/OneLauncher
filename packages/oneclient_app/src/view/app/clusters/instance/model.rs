use std::collections::HashSet;
use std::sync::Arc;

use freya::prelude::*;
use oneclient_common::domain::GameLoader;

use super::details::DetailsState;
use crate::components::IconType;
use crate::hooks::GameVersion;

pub const FILTERS: [&str; 5] = ["Releases", "Snapshots", "Beta", "Alpha", "All versions"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TypeChoice {
    OneClient,
    Scratch,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LoaderMark {
    Tinted(IconType),
    Image(&'static str),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoaderChoice {
    Fabric,
    Forge,
    NeoForge,
    Quilt,
    Vanilla,
}

impl LoaderChoice {
    pub const MODDED: [Self; 4] = [Self::Fabric, Self::Forge, Self::NeoForge, Self::Quilt];

    pub fn primary(self) -> GameLoader {
        match self {
            Self::Fabric => GameLoader::Fabric,
            Self::Forge => GameLoader::Forge,
            Self::NeoForge => GameLoader::NeoForge,
            Self::Quilt => GameLoader::Quilt,
            Self::Vanilla => GameLoader::Vanilla,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
            Self::Quilt => "Quilt",
            Self::Vanilla => "Vanilla",
        }
    }

    pub fn mark(self) -> Option<LoaderMark> {
        match self {
            Self::Fabric => Some(LoaderMark::Image("icons/fabric.png")),
            Self::Forge => Some(LoaderMark::Image("icons/forge.png")),
            Self::NeoForge => Some(LoaderMark::Image("icons/neo-forge.png")),
            Self::Quilt => Some(LoaderMark::Tinted(IconType::Quilt)),
            Self::Vanilla => Some(LoaderMark::Image("icons/vanilla.png")),
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Fabric => "FA",
            Self::Forge => "FO",
            Self::NeoForge => "NF",
            Self::Quilt => "QL",
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
            Self::Quilt => "A Fabric-compatible fork with extra loader features.",
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
}

impl Step {
    pub fn label(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Loader => "Loader",
            Self::Version => "Version",
            Self::Bundles => "Bundles",
            Self::Customize => "Details",
        }
    }
}

const ONECLIENT_STEPS: [Step; 4] = [Step::Type, Step::Version, Step::Bundles, Step::Customize];
const SCRATCH_STEPS: [Step; 4] = [Step::Type, Step::Loader, Step::Version, Step::Customize];
const UNCHOSEN_STEPS: [Step; 3] = [Step::Type, Step::Version, Step::Customize];

pub fn step_order(choice: Option<TypeChoice>) -> &'static [Step] {
    match choice {
        Some(TypeChoice::OneClient) => &ONECLIENT_STEPS,
        Some(TypeChoice::Scratch) => &SCRATCH_STEPS,
        None => &UNCHOSEN_STEPS,
    }
}

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
            Self::Curated(rows) => rows.get(index).map(|row| VersionRow {
                id: row.id.clone(),
                meta: row.meta.clone(),
                badge: row.badge.clone(),
                date: row.date.clone(),
            }),
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

    pub fn default_id(&self) -> Option<String> {
        match self {
            Self::Curated(rows) => rows.first().map(|row| row.id.clone()),
            Self::Catalogue { all, visible } => {
                let release = visible
                    .iter()
                    .map(|index| &all[*index as usize])
                    .find(|entry| entry.kind.is_release());
                release
                    .or_else(|| visible.first().map(|index| &all[*index as usize]))
                    .map(|entry| entry.id.clone())
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct Wizard {
    pub step: State<usize>,
    pub choice: State<Option<TypeChoice>>,
    pub version: State<Option<String>>,
    pub filter: State<usize>,
    pub query: State<String>,
    pub loader: State<LoaderChoice>,
    pub loader_version: State<Option<String>>,
    pub declined: State<Option<HashSet<String>>>,
    pub details: DetailsState,
}
