use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use oneclient_cluster::naming::{MAX_NAME_CHARS, validate_instance_name};
use oneclient_common::domain::GameLoader;
use oneclient_content::packages::ProviderId;
use oneclient_core::clusters::ClusterKind;
use oneclient_core::{BundleArchive, GameVersionKind};

use super::model::*;
use super::package_names;
use crate::hooks::{
    GameVersion, available_bundles, game_versions, java_majors, loader_versions,
    package_meta_batch, query_error, settled_or_loading, use_available_bundles, use_game_versions,
    use_java_majors, use_loader_versions, use_package_meta_batch, use_version_loaders,
    use_versions, versions_metadata,
};
use crate::utils::version_sort_key;

pub struct VersionPicks {
    pub chosen: Option<String>,
    pub list: VersionList,
    pub loaded: bool,
    pub error: Option<String>,
    pub filter: u8,
}

pub struct LoaderPicks {
    pub chosen: Option<GameLoader>,
    pub available: Option<Vec<GameLoader>>,
    pub version: Option<String>,
    pub versions: Arc<[String]>,
}

pub struct BundlePicks {
    pub archives: Arc<[BundleArchive]>,
    pub loaded: bool,
    pub names: HashMap<String, String>,
    pub declined: HashSet<String>,
}

impl BundlePicks {
    pub fn taken(&self) -> Vec<String> {
        self.names_of_taken().collect()
    }

    pub fn taken_count(&self) -> usize {
        self.names_of_taken().count()
    }

    fn names_of_taken(&self) -> impl Iterator<Item = String> + '_ {
        self.archives
            .iter()
            .map(|archive| archive.manifest.name.clone())
            .filter(|name| !self.declined.contains(name))
    }
}

pub struct Picks {
    pub steps: &'static [Step],
    pub index: usize,
    pub step: Step,
    pub choice: TypeChoice,
    pub modpack_origin: ModpackOrigin,
    pub kind: ClusterKind,
    pub name: String,
    pub suggested: String,
    pub versions: VersionPicks,
    pub loader: LoaderPicks,
    pub bundles: BundlePicks,
}

impl Picks {
    pub fn loader_label(&self) -> String {
        match (self.loader.chosen, self.loader.version.as_deref()) {
            (None | Some(GameLoader::Vanilla), _) => "Vanilla".to_string(),
            (Some(loader), Some(version)) => format!("{loader} {version}"),
            (Some(loader), None) => loader.to_string(),
        }
    }
}

fn matches_filter(kind: GameVersionKind, filter: u8) -> bool {
    filter & kind_bit(kind) != 0
}

struct OneClientVersion {
    id: String,
    loader: GameLoader,
}

struct Catalogue {
    oneclient: Vec<OneClientVersion>,
    oneclient_settled: bool,
    vanilla: Option<Arc<[GameVersion]>>,
    java: HashMap<String, u32>,
    error: Option<String>,
}

impl Catalogue {
    fn loader_for(&self, version: &str) -> Option<GameLoader> {
        self.oneclient
            .iter()
            .find(|entry| entry.id == version)
            .map(|entry| entry.loader)
    }

    fn lists(&self, version: &str) -> bool {
        self.oneclient.iter().any(|entry| entry.id == version)
    }
}

fn use_catalogue(choice: TypeChoice) -> Catalogue {
    let oneclient_query = use_versions();
    let vanilla_query = use_game_versions();

    let metadata = versions_metadata(&oneclient_query);
    let mut oneclient: Vec<OneClientVersion> = metadata
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            Some(OneClientVersion {
                id: oneclient_common::version::format_mc_version(
                    entry.major_version,
                    entry.minor_version?,
                    entry.patch_version,
                ),
                loader: entry.loader.as_deref()?.parse().ok()?,
            })
        })
        .collect();
    oneclient.sort_by_key(|entry| std::cmp::Reverse(version_sort_key(&entry.id)));

    let java_query = use_java_majors(if choice == TypeChoice::OneClient {
        oneclient.iter().map(|entry| entry.id.clone()).collect()
    } else {
        Vec::new()
    });

    let error = if choice == TypeChoice::OneClient {
        query_error(&oneclient_query)
    } else {
        query_error(&vanilla_query)
    };

    Catalogue {
        oneclient,
        oneclient_settled: metadata.is_some(),
        vanilla: game_versions(&vanilla_query),
        java: java_majors(&java_query),
        error,
    }
}

struct BundleContext {
    archives: Arc<[BundleArchive]>,
    settled: bool,
    names: HashMap<String, String>,
}

fn use_bundle_context(
    version: Option<&String>,
    loader: Option<GameLoader>,
    takes_bundles: bool,
    wants_names: bool,
) -> BundleContext {
    let key = match (takes_bundles, version) {
        (true, Some(version)) => version.clone(),
        _ => String::new(),
    };

    let query = use_available_bundles(key, loader.unwrap_or(GameLoader::Vanilla));
    let bundles = available_bundles(&query);
    let archives = bundles.clone().unwrap_or_else(|| Arc::from([]));

    let mut modrinth_ids = Vec::new();
    let mut curseforge_ids = Vec::new();
    for file in archives.iter().flat_map(|archive| &archive.manifest.files) {
        match file.kind.metadata_provider() {
            ProviderId::Modrinth => modrinth_ids.push(file.kind.package_id()),
            ProviderId::CurseForge => curseforge_ids.push(file.kind.package_id()),
            ProviderId::Local => {}
        }
    }

    let modrinth = use_package_meta_batch(ProviderId::Modrinth, modrinth_ids);
    let curseforge = use_package_meta_batch(ProviderId::CurseForge, curseforge_ids);

    BundleContext {
        names: if wants_names {
            package_names(
                &archives,
                &package_meta_batch(&modrinth),
                &package_meta_batch(&curseforge),
            )
        } else {
            HashMap::new()
        },
        archives,
        settled: bundles.is_some(),
    }
}

fn build_version_list(
    choice: TypeChoice,
    catalogue: &Catalogue,
    filter: u8,
    needle: &str,
) -> VersionList {
    let matches_needle = |id: &str| needle.is_empty() || id.to_lowercase().contains(needle);

    if choice == TypeChoice::OneClient {
        return VersionList::Curated(
            catalogue
                .oneclient
                .iter()
                .filter(|entry| matches_needle(&entry.id))
                .map(|entry| VersionRow {
                    badge: catalogue
                        .java
                        .get(&entry.id)
                        .map(|major| format!("Java {major}")),
                    id: entry.id.clone(),
                    meta: entry.loader.to_string(),
                    date: String::new(),
                })
                .collect(),
        );
    }

    let all = catalogue.vanilla.clone().unwrap_or_else(|| Arc::from([]));
    let visible = all
        .iter()
        .enumerate()
        .filter(|(_, entry)| matches_filter(entry.kind, filter))
        .filter(|(_, entry)| matches_needle(&entry.id))
        .map(|(index, _)| index as u32)
        .collect();

    VersionList::Catalogue { all, visible }
}

fn default_version(choice: TypeChoice, catalogue: &Catalogue, filter: u8) -> Option<String> {
    if choice == TypeChoice::OneClient {
        return catalogue.oneclient.first().map(|entry| entry.id.clone());
    }

    let all = catalogue.vanilla.as_deref().unwrap_or(&[]);
    let mut scoped = all
        .iter()
        .filter(|entry| matches_filter(entry.kind, filter));
    let first = scoped.clone().next();
    scoped
        .find(|entry| entry.kind.is_release())
        .or(first)
        .map(|entry| entry.id.clone())
}

fn kind_for(choice: TypeChoice, loader: Option<GameLoader>) -> ClusterKind {
    match (choice, loader) {
        (TypeChoice::OneClient, _) => ClusterKind::OneClient,
        (TypeChoice::Scratch, Some(GameLoader::Vanilla) | None) => ClusterKind::Vanilla,
        (TypeChoice::Scratch, Some(_)) | (TypeChoice::Modpack, _) => ClusterKind::Modded,
    }
}

fn suggested_name(
    version: Option<&String>,
    kind: ClusterKind,
    loader: Option<GameLoader>,
) -> String {
    let Some(version) = version else {
        return String::new();
    };

    let full = match (kind, loader) {
        (ClusterKind::Vanilla, _) => format!("{version} Vanilla"),
        (_, Some(loader)) => format!("{version} {loader}"),
        (_, None) => version.clone(),
    };
    if validate_instance_name(&full).is_ok() {
        return full;
    }

    version
        .chars()
        .take(MAX_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn resolve(w: Wizard) -> Picks {
    let choice = *w.choice.read();
    let steps = step_order(choice);
    let index = (*w.step.read()).min(steps.len().saturating_sub(1));
    let step = steps[index];

    let catalogue = use_catalogue(choice);

    let filter = *w.filter.read();
    let needle = w.query.read().trim().to_lowercase();
    let versions = build_version_list(choice, &catalogue, filter, &needle);

    let oneclient = choice == TypeChoice::OneClient;
    let versions_settled = if oneclient {
        catalogue.oneclient_settled
    } else {
        catalogue.vanilla.is_some()
    };
    let versions_error = catalogue.error.clone();

    let still_offered = |chosen: &String| {
        if oneclient {
            catalogue.lists(chosen)
        } else {
            catalogue
                .vanilla
                .iter()
                .flat_map(|all| all.iter())
                .any(|entry: &GameVersion| {
                    &entry.id == chosen && matches_filter(entry.kind, filter)
                })
        }
    };
    let version = (w.version.read().clone())
        .filter(still_offered)
        .or_else(|| default_version(choice, &catalogue, filter));

    let available_query = use_version_loaders(version.clone().unwrap_or_default());
    let available = settled_or_loading(&available_query);
    let loader = match choice {
        TypeChoice::OneClient => version.as_ref().and_then(|id| catalogue.loader_for(id)),
        TypeChoice::Scratch | TypeChoice::Modpack => {
            w.loader.read().resolve(available.as_deref().unwrap_or(&[]))
        }
    };

    let loader_versions = loader_versions(&use_loader_versions(
        version.clone().unwrap_or_default(),
        loader.unwrap_or(GameLoader::Vanilla),
    ));
    let loader_version = match (choice, loader) {
        (TypeChoice::Scratch, Some(loader)) if loader != GameLoader::Vanilla => {
            (w.loader_version.read().clone())
                .filter(|chosen| loader_versions.contains(chosen))
                .or_else(|| loader_versions.first().cloned())
        }
        _ => None,
    };

    let bundles = use_bundle_context(version.as_ref(), loader, oneclient, step == Step::Bundles);

    let kind = kind_for(choice, loader);
    let suggested = suggested_name(version.as_ref(), kind, loader);
    let declined = w.declined.read().clone().unwrap_or_else(|| {
        bundles
            .archives
            .iter()
            .filter(|archive| !archive.manifest.enabled)
            .map(|archive| archive.manifest.name.clone())
            .collect()
    });

    Picks {
        steps,
        index,
        step,
        choice,
        modpack_origin: *w.modpack_origin.read(),
        kind,
        name: w.details.effective_name(&suggested),
        suggested,
        versions: VersionPicks {
            chosen: version,
            list: versions,
            loaded: versions_settled,
            error: versions_error,
            filter,
        },
        loader: LoaderPicks {
            chosen: loader,
            available,
            version: loader_version,
            versions: loader_versions,
        },
        bundles: BundlePicks {
            archives: bundles.archives,
            loaded: bundles.settled,
            names: bundles.names,
            declined,
        },
    }
}
