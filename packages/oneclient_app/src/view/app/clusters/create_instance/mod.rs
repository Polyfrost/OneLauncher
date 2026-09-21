use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_content::packages::ProviderId;
use oneclient_core::clusters::ClusterKind;
use oneclient_core::{BundleArchive, BundleFile, BundleFileKind, GameVersionKind};

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{
    ClusterAction, available_bundles, game_versions, java_majors, loader_game_versions,
    loader_versions, package_meta_batch, use_available_bundles, use_cluster_mutation,
    use_game_versions, use_java_majors, use_loader_game_versions, use_loader_versions,
    use_package_meta_batch, use_version_loaders, use_versions, version_loaders, versions_metadata,
};
use crate::theme::colors;
use crate::ui::border_all_color;

mod rail;
mod steps;

const DIALOG_WIDTH: f32 = 1020.;
const DIALOG_HEIGHT: f32 = 660.;
const RAIL_WIDTH: f32 = 368.;
const PANE_PADDING: f32 = 24.;

const FILTERS: [&str; 5] = ["Releases", "Snapshots", "Beta", "Alpha", "All versions"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TypeChoice {
    OneClient,
    Scratch,
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
    const ALL: [Self; 5] = [
        Self::Fabric,
        Self::Forge,
        Self::NeoForge,
        Self::Quilt,
        Self::Vanilla,
    ];

    fn primary(self) -> GameLoader {
        match self {
            Self::Fabric => GameLoader::Fabric,
            Self::Forge => GameLoader::Forge,
            Self::NeoForge => GameLoader::NeoForge,
            Self::Quilt => GameLoader::Quilt,
            Self::Vanilla => GameLoader::Vanilla,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
            Self::Quilt => "Quilt",
            Self::Vanilla => "No loader",
        }
    }

    fn blurb(self) -> &'static str {
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
    fn label(self) -> &'static str {
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

#[derive(Clone, Copy)]
pub struct Wizard {
    step: State<usize>,
    choice: State<Option<TypeChoice>>,
    version: State<Option<String>>,
    filter: State<usize>,
    query: State<String>,
    loader: State<LoaderChoice>,
    loader_version: State<Option<String>>,
    declined: State<Option<HashSet<String>>>,
    name: State<String>,
    name_touched: State<bool>,
    description: State<String>,
    tags: State<Vec<String>>,
    tag_draft: State<String>,
    tag_open: State<bool>,
    cover: State<Option<PathBuf>>,
}

pub struct Picks {
    steps: Vec<Step>,
    index: usize,
    step: Step,
    choice: Option<TypeChoice>,
    kind: ClusterKind,
    version: Option<String>,
    loader: Option<GameLoader>,
    loader_version: Option<String>,
    loader_versions: Vec<String>,
    versions: Vec<VersionRow>,
    versions_loaded: bool,
    archives: Vec<BundleArchive>,
    bundles_loaded: bool,
    package_names: HashMap<String, String>,
    declined: HashSet<String>,
    name: String,
    suggested: String,
}

impl Picks {
    fn loader_label(&self) -> String {
        match (self.loader, self.loader_version.as_deref()) {
            (None | Some(GameLoader::Vanilla), _) => "No loader".to_string(),
            (Some(loader), Some(version)) => format!("{loader} {version}"),
            (Some(loader), None) => loader.to_string(),
        }
    }

    fn taken_bundles(&self) -> Vec<String> {
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

    let versions: Vec<VersionRow> = if choice == Some(TypeChoice::OneClient) {
        oneclient
            .iter()
            .filter(|(id, _)| needle.is_empty() || id.to_lowercase().contains(&needle))
            .map(|(id, loader)| VersionRow {
                badge: java.get(id).map(|major| format!("Java {major}")),
                id: id.clone(),
                meta: loader.to_string(),
                date: String::new(),
            })
            .collect()
    } else {
        all.iter()
            .filter(|info| matches_filter(info.kind, filter))
            .filter(|info| match &allowed {
                Some(Some(ids)) => ids.contains(&info.id),
                _ => true,
            })
            .filter(|info| needle.is_empty() || info.id.to_lowercase().contains(&needle))
            .map(|info| VersionRow {
                id: info.id.clone(),
                meta: String::new(),
                badge: (!info.kind.is_release()).then(|| info.kind.label().to_string()),
                date: info.released.format("%d %b %Y").to_string(),
            })
            .collect()
    };

    let versions_loaded = if choice == Some(TypeChoice::OneClient) {
        catalogue_loaded
    } else {
        !all.is_empty() && allowed.is_some()
    };

    let default_version = if choice == Some(TypeChoice::OneClient) {
        versions.first()
    } else {
        versions
            .iter()
            .find(|row| row.badge.is_none())
            .or_else(|| versions.first())
    }
    .map(|row| row.id.clone());
    let version = (w.version.read().clone())
        .filter(|chosen| versions.iter().any(|row| &row.id == chosen))
        .or(default_version);

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
    let names = package_names(
        &archives,
        &package_meta_batch(&modrinth_query),
        &package_meta_batch(&curseforge_query),
    );

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

    let typed = w.name.read().trim().to_string();
    let name = if *w.name_touched.read() && !typed.is_empty() {
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
        declined: w.declined.read().clone().unwrap_or_else(|| {
            bundles
                .iter()
                .flatten()
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

pub fn file_provider(file: &BundleFile) -> ProviderId {
    match &file.kind {
        BundleFileKind::Managed { provider, .. } => *provider,
        BundleFileKind::External(_) => ProviderId::Local,
    }
}

pub fn tidy_file_name(raw: &str) -> String {
    let stem = raw
        .rsplit_once('.')
        .filter(|(head, ext)| !head.is_empty() && ext.len() <= 8)
        .map_or(raw, |(head, _)| head);

    let bytes = stem.as_bytes();
    let cut = bytes.iter().enumerate().position(|(index, byte)| {
        matches!(byte, b'-' | b'_' | b'+') && bytes.get(index + 1).is_some_and(u8::is_ascii_digit)
    });

    match cut {
        Some(0) | None => stem.to_string(),
        Some(cut) => stem[..cut].to_string(),
    }
}

fn package_names(
    archives: &[BundleArchive],
    modrinth: &HashMap<String, oneclient_content::packages::CachedPackageMeta>,
    curseforge: &HashMap<String, oneclient_content::packages::CachedPackageMeta>,
) -> HashMap<String, String> {
    let mut names = HashMap::new();

    for file in archives.iter().flat_map(|archive| &archive.manifest.files) {
        let package_id = file.kind.package_id();
        let meta = match file_provider(file) {
            ProviderId::Modrinth => modrinth.get(&package_id),
            ProviderId::CurseForge => curseforge.get(&package_id),
            ProviderId::Local => None,
        };

        let name = meta
            .map(|meta| meta.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| tidy_file_name(&file.display_name()));

        names.insert(package_id, name);
    }

    names
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

fn header(picks: &Picks, on_close: EventHandler<()>) -> Element {
    let (title, sub) = heading(picks);

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Start)
        .spacing(16.)
        .padding(Gaps::new(22., PANE_PADDING, 16., PANE_PADDING))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(5.)
                .child(
                    label()
                        .text(format!("Step {} of {}", picks.index + 1, picks.steps.len()))
                        .font_size(11.)
                        .font_weight(FontWeight::MEDIUM)
                        .letter_spacing(1.6)
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(title)
                        .font_size(20.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(sub)
                        .font_size(13.)
                        .line_height(1.35)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Button::new()
                .ghost()
                .icon()
                .alt("Close")
                .on_press(move |_| on_close.call(()))
                .child(Icon::new(IconType::XClose).size(16.)),
        )
        .into_element()
}

fn footer(
    mut wizard: Wizard,
    picks: &Picks,
    on_cancel: EventHandler<()>,
    on_create: EventHandler<()>,
) -> Element {
    let ready = picks.ready();
    let last = picks.step == Step::Customize;
    let first = picks.index == 0;
    let index = picks.index;

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(20.)
        .padding(Gaps::new(16., PANE_PADDING, 16., PANE_PADDING))
        .border(
            Border::new()
                .fill(colors::component_border())
                .width(BorderWidth {
                    top: 1.,
                    right: 0.,
                    bottom: 0.,
                    left: 0.,
                }),
        )
        .child(
            label()
                .text(footer_note(picks))
                .width(Size::flex(1.0))
                .font_size(12.)
                .line_height(1.35)
                .max_lines(2)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .horizontal()
                .spacing(10.)
                .cross_align(Alignment::Center)
                .child(
                    Button::new()
                        .ghost()
                        .on_press(move |_| {
                            if first {
                                on_cancel.call(());
                            } else {
                                wizard.step.set(index.saturating_sub(1));
                            }
                        })
                        .text(if first { "Cancel" } else { "Back" }),
                )
                .child(
                    Button::new()
                        .primary()
                        .enabled(ready)
                        .on_press(move |_| {
                            if !ready {
                                return;
                            }
                            if last {
                                on_create.call(());
                            } else {
                                wizard.query.set(String::new());
                                wizard.step.set(index + 1);
                            }
                        })
                        .text(if last { "Create instance" } else { "Next" }),
                ),
        )
        .into_element()
}

fn create_action(wizard: Wizard, picks: &Picks) -> Option<ClusterAction> {
    let mc_version = picks.version.clone()?;
    let mc_loader = picks.loader?;
    let description = wizard.description.read().trim().to_string();

    Some(ClusterAction::CreateInstance {
        kind: picks.kind,
        name: picks.name.trim().to_string(),
        mc_version,
        mc_loader,
        mc_loader_version: picks.loader_version.clone(),
        description: (!description.is_empty()).then_some(description),
        tags: wizard.tags.read().clone(),
        cover_source: wizard.cover.read().clone(),
        bundles: (picks.kind == ClusterKind::OneClient).then(|| picks.taken_bundles()),
    })
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
            name: use_state(String::new),
            name_touched: use_state(|| false),
            description: use_state(String::new),
            tags: use_state(Vec::new),
            tag_draft: use_state(String::new),
            tag_open: use_state(|| false),
            cover: use_state(|| None::<PathBuf>),
        };

        let picks = resolve(wizard);
        let action = create_action(wizard, &picks);

        let close_scrim = self.on_close.clone();
        let close_x = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_created = self.on_close.clone();

        OverlayPopup::new()
            .on_close(move |()| close_scrim.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::px(DIALOG_WIDTH))
                            .height(Size::px(DIALOG_HEIGHT))
                            .max_width(Size::window_percent(94.))
                            .max_height(Size::window_percent(92.))
                            .content(Content::Flex)
                            .corner_radius(CornerRadius::new_all(16.))
                            .overflow(Overflow::Clip)
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .child(rail::rail(wizard, &picks))
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::flex(1.0))
                                    .height(Size::fill())
                                    .content(Content::Flex)
                                    .child(header(&picks, (move |()| close_x.call(())).into()))
                                    .child(
                                        ScrollArea::new()
                                            .width(Size::fill())
                                            .height(Size::flex(1.0))
                                            .padding(Gaps::new(0., PANE_PADDING, 8., PANE_PADDING))
                                            .scrollbar_gutter(true)
                                            .child(steps::body(wizard, &picks)),
                                    )
                                    .child(footer(
                                        wizard,
                                        &picks,
                                        (move |()| close_cancel.call(())).into(),
                                        (move |()| {
                                            if let Some(action) = action.clone() {
                                                mutation.mutate(action);
                                                close_created.call(());
                                            }
                                        })
                                        .into(),
                                    )),
                            ),
                    ),
            )
            .into_element()
    }
}

#[cfg(test)]
mod tests {
    use super::tidy_file_name;

    #[test]
    fn a_versioned_jar_keeps_only_its_name() {
        assert_eq!(
            tidy_file_name("sodium-fabric-0.5.11+mc1.20.1.jar"),
            "sodium-fabric"
        );
        assert_eq!(tidy_file_name("EvergreenHUD-3.0.0.jar"), "EvergreenHUD");
        assert_eq!(tidy_file_name("Sodium-13.303x012"), "Sodium");
    }

    #[test]
    fn a_name_without_a_version_survives_whole() {
        assert_eq!(tidy_file_name("OneConfig.jar"), "OneConfig");
        assert_eq!(tidy_file_name("PolyBlur"), "PolyBlur");
    }

    #[test]
    fn a_leading_separator_is_not_a_version_cut() {
        assert_eq!(tidy_file_name("-1abc"), "-1abc");
    }

    #[test]
    fn a_dotted_name_is_not_mistaken_for_an_extension() {
        assert_eq!(
            tidy_file_name("com.example.longextension"),
            "com.example.longextension"
        );
    }
}
