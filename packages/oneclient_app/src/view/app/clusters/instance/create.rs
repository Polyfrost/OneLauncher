use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_common::parse_mc_version;
use oneclient_content::packages::ProviderId;
use oneclient_core::clusters::ClusterKind;
use std::sync::Arc;

use oneclient_core::{BundleArchive, GameVersionKind};

use super::details::DetailsState;
use super::rail::{Rail, RowState, rail, steps_card};
use super::shell::{Shell, shell};
use super::steps;
use super::{file_provider, package_names};
use crate::components::{DynamicArt, IconType};
use crate::hooks::{
    ClusterAction, GameVersion, available_bundles, game_versions, java_majors,
    loader_game_versions, loader_versions, package_meta_batch, query_error, use_available_bundles,
    use_cluster_mutation, use_game_versions, use_java_majors, use_loader_game_versions,
    use_loader_versions, use_package_meta_batch, use_version_loaders, use_versions,
    version_loaders, versions_metadata,
};

pub(super) const FILTERS: [&str; 5] = ["Releases", "Snapshots", "Beta", "Alpha", "All versions"];

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
    pub(super) const MODDED: [Self; 4] = [Self::Fabric, Self::Forge, Self::NeoForge, Self::Quilt];

    fn primary(self) -> GameLoader {
        match self {
            Self::Fabric => GameLoader::Fabric,
            Self::Forge => GameLoader::Forge,
            Self::NeoForge => GameLoader::NeoForge,
            Self::Quilt => GameLoader::Quilt,
            Self::Vanilla => GameLoader::Vanilla,
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
            Self::Quilt => "Quilt",
            Self::Vanilla => "Vanilla",
        }
    }

    pub(super) fn mark(self) -> Option<LoaderMark> {
        match self {
            Self::Fabric => Some(LoaderMark::Image("icons/fabric.png")),
            Self::Forge => Some(LoaderMark::Image("icons/forge.png")),
            Self::NeoForge => Some(LoaderMark::Image("icons/neo-forge.png")),
            Self::Quilt => Some(LoaderMark::Tinted(IconType::Quilt)),
            Self::Vanilla => Some(LoaderMark::Image("icons/vanilla.png")),
        }
    }

    pub(super) fn short(self) -> &'static str {
        match self {
            Self::Fabric => "FA",
            Self::Forge => "FO",
            Self::NeoForge => "NF",
            Self::Quilt => "QL",
            Self::Vanilla => "MC",
        }
    }

    pub(super) fn blurb(self) -> &'static str {
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

    fn resolve(self, available: &[GameLoader]) -> Option<GameLoader> {
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
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Loader => "Loader",
            Self::Version => "Version",
            Self::Bundles => "Bundles",
            Self::Customize => "Details",
        }
    }
}

fn step_order(choice: Option<TypeChoice>) -> Vec<Step> {
    match choice {
        Some(TypeChoice::OneClient) => {
            vec![Step::Type, Step::Version, Step::Bundles, Step::Customize]
        }
        Some(TypeChoice::Scratch) => vec![Step::Type, Step::Loader, Step::Version, Step::Customize],
        None => vec![Step::Type, Step::Version, Step::Customize],
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
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Curated(rows) => rows.len(),
            Self::Catalogue { visible, .. } => visible.len(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(super) fn row(&self, index: usize) -> Option<VersionRow> {
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

    fn default_id(&self) -> Option<String> {
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
    pub(super) step: State<usize>,
    pub(super) choice: State<Option<TypeChoice>>,
    pub(super) version: State<Option<String>>,
    pub(super) filter: State<usize>,
    pub(super) query: State<String>,
    pub(super) loader: State<LoaderChoice>,
    pub(super) loader_version: State<Option<String>>,
    pub(super) declined: State<Option<HashSet<String>>>,
    pub(super) details: DetailsState,
}

pub struct Picks {
    pub(super) steps: Vec<Step>,
    pub(super) index: usize,
    pub(super) step: Step,
    pub(super) choice: Option<TypeChoice>,
    pub(super) kind: ClusterKind,
    pub(super) version: Option<String>,
    pub(super) loader: Option<GameLoader>,
    pub(super) loader_version: Option<String>,
    pub(super) loader_versions: Vec<String>,
    pub(super) versions: VersionList,
    pub(super) versions_loaded: bool,
    pub(super) versions_error: Option<String>,
    pub(super) filter: usize,
    pub(super) archives: Arc<[BundleArchive]>,
    pub(super) bundles_loaded: bool,
    pub(super) package_names: HashMap<String, String>,
    pub(super) declined: HashSet<String>,
    pub(super) name: String,
    pub(super) suggested: String,
}

impl Picks {
    pub(super) fn loader_label(&self) -> String {
        match (self.loader, self.loader_version.as_deref()) {
            (None | Some(GameLoader::Vanilla), _) => "Vanilla".to_string(),
            (Some(loader), Some(version)) => format!("{loader} {version}"),
            (Some(loader), None) => loader.to_string(),
        }
    }

    pub(super) fn taken_bundles(&self) -> Vec<String> {
        self.archives
            .iter()
            .map(|archive| archive.manifest.name.clone())
            .filter(|name| !self.declined.contains(name))
            .collect()
    }

    fn ready(&self) -> bool {
        match self.step {
            Step::Type => self.choice.is_some(),
            Step::Loader | Step::Bundles => true,
            Step::Version => self.version.is_some() && self.loader.is_some(),
            Step::Customize => !self.name.trim().is_empty(),
        }
    }
}

fn version_sort_key(version: &str) -> (u32, u32, u32) {
    oneclient_common::parse_mc_version(version)
        .map(|parsed| {
            (
                parsed.major,
                parsed.minor.unwrap_or(0),
                parsed.patch.unwrap_or(0),
            )
        })
        .unwrap_or((0, 0, 0))
}

fn matches_filter(kind: GameVersionKind, filter: usize) -> bool {
    match filter {
        0 => kind == GameVersionKind::Release,
        1 => kind == GameVersionKind::Snapshot,
        2 => kind == GameVersionKind::Beta,
        3 => kind == GameVersionKind::Alpha,
        _ => true,
    }
}

fn resolve(w: Wizard) -> Picks {
    let choice = *w.choice.read();
    let steps = step_order(choice);
    let index = (*w.step.read()).min(steps.len().saturating_sub(1));
    let step = steps[index];

    let oneclient_query = use_versions();
    let catalogue = versions_metadata(&oneclient_query);
    let mut oneclient: Vec<(String, GameLoader)> = catalogue
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let minor = entry.minor_version?;
            let loader = entry.loader.as_deref()?.parse::<GameLoader>().ok()?;
            Some((
                oneclient_common::version::format_mc_version(
                    entry.major_version,
                    minor,
                    entry.patch_version,
                ),
                loader,
            ))
        })
        .collect();
    oneclient.sort_by_key(|entry| std::cmp::Reverse(version_sort_key(&entry.0)));

    let vanilla_query = use_game_versions();
    let catalogue_loaded = catalogue.is_some();
    let all = game_versions(&vanilla_query);

    let loader_choice = *w.loader.read();
    let primary_query = use_loader_game_versions(loader_choice.primary());
    let ornithe_query = use_loader_game_versions(GameLoader::Ornithe);
    let allowed = match loader_game_versions(&primary_query) {
        None => None,
        Some(None) => Some(None),
        Some(Some(mut ids)) => {
            if loader_choice == LoaderChoice::Fabric
                && let Some(Some(extra)) = loader_game_versions(&ornithe_query)
            {
                ids.extend(extra);
            }
            Some(Some(ids.into_iter().collect::<HashSet<String>>()))
        }
    };

    let filter = *w.filter.read();
    let needle = w.query.read().trim().to_lowercase();

    let java_query = use_java_majors(oneclient.iter().map(|(id, _)| id.clone()).collect());
    let java = java_majors(&java_query);

    let catalogue_entries = all.clone().unwrap_or_else(|| Arc::from([]));
    let in_scope = |entry: &GameVersion| {
        matches_filter(entry.kind, filter)
            && match &allowed {
                Some(Some(ids)) => ids.contains(&entry.id),
                _ => true,
            }
    };

    let versions = if choice == Some(TypeChoice::OneClient) {
        VersionList::Curated(
            oneclient
                .iter()
                .filter(|(id, _)| needle.is_empty() || id.to_lowercase().contains(&needle))
                .map(|(id, loader)| VersionRow {
                    badge: java.get(id).map(|major| format!("Java {major}")),
                    id: id.clone(),
                    meta: loader.to_string(),
                    date: String::new(),
                })
                .collect(),
        )
    } else {
        let visible: Arc<[u32]> = catalogue_entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| in_scope(entry))
            .filter(|(_, entry)| needle.is_empty() || entry.id.to_lowercase().contains(&needle))
            .map(|(index, _)| index as u32)
            .collect();

        VersionList::Catalogue {
            all: catalogue_entries.clone(),
            visible,
        }
    };

    let versions_error = if choice == Some(TypeChoice::OneClient) {
        query_error(&oneclient_query)
    } else {
        query_error(&vanilla_query).or_else(|| query_error(&primary_query))
    };

    let versions_loaded = if choice == Some(TypeChoice::OneClient) {
        catalogue_loaded
    } else {
        all.is_some() && allowed.is_some()
    };

    let still_in_scope = |chosen: &String| {
        if choice == Some(TypeChoice::OneClient) {
            oneclient.iter().any(|(id, _)| id == chosen)
        } else {
            catalogue_entries
                .iter()
                .any(|entry| &entry.id == chosen && in_scope(entry))
        }
    };

    let version = (w.version.read().clone())
        .filter(still_in_scope)
        .or_else(|| versions.default_id());

    let available = version_loaders(&use_version_loaders(version.clone().unwrap_or_default()));
    let loader = match choice {
        Some(TypeChoice::OneClient) => version.as_ref().and_then(|chosen| {
            oneclient
                .iter()
                .find(|(id, _)| id == chosen)
                .map(|(_, loader)| *loader)
        }),
        Some(TypeChoice::Scratch) => loader_choice.resolve(&available),
        None => None,
    };

    let loader_versions_query = use_loader_versions(
        version.clone().unwrap_or_default(),
        loader.unwrap_or(GameLoader::Vanilla),
    );
    let available_loader_versions = loader_versions(&loader_versions_query);
    let loader_version = match (choice, loader) {
        (Some(TypeChoice::Scratch), Some(loader)) if loader != GameLoader::Vanilla => {
            (w.loader_version.read().clone())
                .filter(|chosen| available_loader_versions.iter().any(|v| v == chosen))
                .or_else(|| available_loader_versions.first().cloned())
        }
        _ => None,
    };

    let bundle_version = if choice == Some(TypeChoice::OneClient) {
        version.clone().unwrap_or_default()
    } else {
        String::new()
    };
    let bundles_query =
        use_available_bundles(bundle_version, loader.unwrap_or(GameLoader::Vanilla));
    let bundles = available_bundles(&bundles_query);
    let archives = bundles.clone().unwrap_or_default();

    let mut modrinth_ids = Vec::new();
    let mut curseforge_ids = Vec::new();
    for file in archives.iter().flat_map(|archive| &archive.manifest.files) {
        match file_provider(file) {
            ProviderId::Modrinth => modrinth_ids.push(file.kind.package_id()),
            ProviderId::CurseForge => curseforge_ids.push(file.kind.package_id()),
            ProviderId::Local => {}
        }
    }
    let modrinth_query = use_package_meta_batch(ProviderId::Modrinth, modrinth_ids);
    let curseforge_query = use_package_meta_batch(ProviderId::CurseForge, curseforge_ids);
    let names = if step == Step::Bundles {
        package_names(
            &archives,
            &package_meta_batch(&modrinth_query),
            &package_meta_batch(&curseforge_query),
        )
    } else {
        HashMap::new()
    };

    let kind = match (choice, loader) {
        (Some(TypeChoice::OneClient) | None, _) => ClusterKind::OneClient,
        (Some(TypeChoice::Scratch), Some(GameLoader::Vanilla) | None) => ClusterKind::Vanilla,
        (Some(TypeChoice::Scratch), Some(_)) => ClusterKind::Modded,
    };

    let suggested = match (&version, choice) {
        (Some(version), Some(TypeChoice::OneClient)) => version.clone(),
        (Some(version), _) if kind == ClusterKind::Vanilla => format!("{version} Vanilla"),
        (Some(version), _) => match loader {
            Some(loader) => format!("{version} {loader}"),
            None => version.clone(),
        },
        (None, _) => String::new(),
    };

    let typed = w.details.typed_name();
    let name = if *w.details.name_touched.read() && !typed.is_empty() {
        typed
    } else {
        suggested.clone()
    };

    Picks {
        steps,
        index,
        step,
        choice,
        kind,
        version,
        loader,
        loader_version,
        loader_versions: available_loader_versions,
        versions,
        versions_loaded,
        versions_error,
        filter,
        declined: w.declined.read().clone().unwrap_or_else(|| {
            archives
                .iter()
                .filter(|archive| !archive.manifest.enabled)
                .map(|archive| archive.manifest.name.clone())
                .collect()
        }),
        archives,
        bundles_loaded: bundles.is_some(),
        package_names: names,
        name,
        suggested,
    }
}

fn heading(picks: &Picks) -> (&'static str, String) {
    match picks.step {
        Step::Type => (
            "Choose a type",
            "Two ways to start. Packages can be added to either of them later.".to_string(),
        ),
        Step::Loader => (
            "Choose a mod loader",
            "Loaders are what packages install into. Pick one, then the Minecraft version it runs on."
                .to_string(),
        ),
        Step::Version => (
            "Choose a version",
            match picks.choice {
                Some(TypeChoice::OneClient) => {
                    "Only versions OneClient ships a build for are listed.".to_string()
                }
                _ => format!(
                    "Versions {} can run. Switch the release type to reach snapshots, betas and alphas.",
                    picks.loader_label()
                ),
            },
        ),
        Step::Bundles => (
            "Add bundles",
            "Curated sets of packages, installed and configured together. Take as many as you like."
                .to_string(),
        ),
        Step::Customize => (
            "Name the instance",
            "Everything here can be changed afterwards.".to_string(),
        ),
    }
}

fn footer_note(picks: &Picks) -> String {
    match picks.step {
        Step::Type => match picks.choice {
            Some(TypeChoice::OneClient) => {
                "Shares its game folder, worlds and packs with your other OneClient instances."
                    .to_string()
            }
            Some(TypeChoice::Scratch) => "Keeps its own game folder, worlds and packs.".to_string(),
            None => "Pick a type to continue.".to_string(),
        },
        Step::Loader => picks.loader_label(),
        Step::Version => match (&picks.version, picks.loader) {
            (Some(version), Some(_)) => {
                format!("{version} downloads the first time you launch it.")
            }
            (Some(version), None) => {
                format!("{} has no build for {version}.", picks.loader_label())
            }
            (None, _) => "Pick a version to continue.".to_string(),
        },
        Step::Bundles => {
            let taken = picks.taken_bundles().len();
            let total = picks.archives.len();
            if taken == 0 {
                "Nothing selected. OneConfig is installed either way.".to_string()
            } else {
                format!("{taken} of {total} selected. Bundles can be changed later.")
            }
        }
        Step::Customize => "The instance is created locally. Nothing is uploaded.".to_string(),
    }
}

fn create_action(wizard: Wizard, picks: &Picks) -> Option<ClusterAction> {
    let mc_version = picks.version.clone()?;
    let mc_loader = picks.loader?;

    Some(ClusterAction::CreateInstance {
        kind: picks.kind,
        name: picks.name.trim().to_string(),
        mc_version,
        mc_loader,
        mc_loader_version: picks.loader_version.clone(),
        description: wizard.details.description_value(),
        tags: wizard.details.tags.read().clone(),
        cover_source: wizard.details.cover.read().clone(),
        bundles: (picks.kind == ClusterKind::OneClient && picks.bundles_loaded)
            .then(|| picks.taken_bundles()),
    })
}

fn wizard_rail(wizard: Wizard, picks: &Picks) -> Element {
    let description = wizard.details.description.read().trim().to_string();
    let tags = wizard.details.tags.read().clone();

    let title = if picks.name.trim().is_empty() {
        "New instance".to_string()
    } else {
        picks.name.clone()
    };

    let subtitle = if description.is_empty() {
        match picks.choice {
            None => "Pick a type to get started".to_string(),
            Some(TypeChoice::OneClient) => match &picks.version {
                Some(version) => format!("OneClient · {version}"),
                None => "OneClient".to_string(),
            },
            Some(TypeChoice::Scratch) => match &picks.version {
                Some(version) => format!("{version} · {}", picks.loader_label()),
                None => picks.loader_label(),
            },
        }
    } else {
        description
    };

    let rows = picks
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let state = if index < picks.index {
                RowState::Done
            } else if index == picks.index {
                RowState::Current
            } else {
                RowState::Pending
            };
            let value = if state == RowState::Pending {
                String::new()
            } else {
                step_value(wizard, picks, *step)
            };
            (step.label(), value, state)
        })
        .collect();

    let parsed = picks.version.as_deref().and_then(parse_mc_version);
    let art = match parsed {
        Some(parsed) => DynamicArt::for_version(parsed.major, parsed.key(), picks.loader),
        None => DynamicArt::fallback(),
    }
    .picked_cover(wizard.details.cover.read().clone());

    rail(Rail {
        art,
        title,
        subtitle,
        card: steps_card(
            format!("Step {} of {}", picks.index + 1, picks.steps.len()),
            rows,
        ),
        tags: if picks.step == Step::Customize {
            tags
        } else {
            Vec::new()
        },
    })
}

fn step_value(wizard: Wizard, picks: &Picks, step: Step) -> String {
    match step {
        Step::Type => match picks.choice {
            Some(TypeChoice::OneClient) => "OneClient".to_string(),
            Some(TypeChoice::Scratch) => "From scratch".to_string(),
            None => String::new(),
        },
        Step::Loader => picks.loader_label(),
        Step::Version => picks
            .version
            .clone()
            .unwrap_or_else(|| "Not chosen".to_string()),
        Step::Bundles => {
            let taken = picks.taken_bundles().len();
            if taken == 0 {
                "None".to_string()
            } else {
                format!("{taken} selected")
            }
        }
        Step::Customize => match wizard.details.tags.read().len() {
            0 => "Optional".to_string(),
            1 => "1 tag".to_string(),
            many => format!("{many} tags"),
        },
    }
}

#[derive(PartialEq)]
pub struct CreateInstanceModal {
    on_close: EventHandler<()>,
}

impl CreateInstanceModal {
    pub fn new(on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            on_close: on_close.into(),
        }
    }
}

impl Component for CreateInstanceModal {
    fn render(&self) -> impl IntoElement {
        let mutation = use_cluster_mutation();
        let wizard = Wizard {
            step: use_state(|| 0usize),
            choice: use_state(|| Some(TypeChoice::OneClient)),
            version: use_state(|| None::<String>),
            filter: use_state(|| 0usize),
            query: use_state(String::new),
            loader: use_state(|| LoaderChoice::Fabric),
            loader_version: use_state(|| None::<String>),
            declined: use_state(|| None::<HashSet<String>>),
            details: DetailsState::blank(),
        };

        let picks = resolve(wizard);
        let action = create_action(wizard, &picks);
        let (title, subtitle) = heading(&picks);

        let first = picks.index == 0;
        let last = picks.step == Step::Customize;
        let index = picks.index;
        let mut step = wizard.step;
        let mut query = wizard.query;

        let close_x = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_created = self.on_close.clone();

        shell(Shell {
            rail: wizard_rail(wizard, &picks),
            eyebrow: format!("Step {} of {}", picks.index + 1, picks.steps.len()),
            title: title.to_string(),
            subtitle,
            body: steps::body(wizard, &picks),
            scrolls_itself: picks.step == Step::Version,
            note: footer_note(&picks),
            secondary_label: if first { "Cancel" } else { "Back" }.to_string(),
            primary_label: if last { "Create instance" } else { "Next" }.to_string(),
            primary_enabled: picks.ready(),
            on_close: (move |()| close_x.call(())).into(),
            on_secondary: (move |()| {
                if first {
                    close_cancel.call(());
                } else {
                    step.set(index.saturating_sub(1));
                }
            })
            .into(),
            on_primary: (move |()| {
                if last {
                    if let Some(action) = action.clone() {
                        mutation.mutate(action);
                        close_created.call(());
                    }
                } else {
                    query.set(String::new());
                    step.set(index + 1);
                }
            })
            .into(),
        })
    }
}
