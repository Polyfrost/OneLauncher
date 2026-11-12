use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use oneclient_cluster::naming::{MAX_NAME_CHARS, validate_instance_name};
use oneclient_common::domain::GameLoader;
use oneclient_content::packages::ProviderId;
use oneclient_core::clusters::{ClusterKind, PackChoices};
use oneclient_core::{
    BundleArchive, ExternalInstance, GameVersionKind, ImportAlternative, ScreenedInstance,
};

use super::model::*;
use super::package_names;
use crate::hooks::{
    AlternativePick, ExternalImportJob, GameVersion, available_bundles, game_versions,
    import_screening, java_majors, loader_versions, package_meta_batch, query_error,
    settled_or_loading, use_available_bundles, use_game_versions, use_import_screening,
    use_java_majors, use_loader_versions, use_modpack_header, use_package_meta_batch,
    use_version_loaders, use_versions, versions_metadata,
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

#[derive(Clone)]
pub struct BundleTarget {
    pub key: PathBuf,
    pub name: String,
    pub mc_version: String,
    pub loader: GameLoader,
}

pub struct Picks {
    pub steps: &'static [Step],
    pub index: usize,
    pub step: Step,
    pub choice: TypeChoice,
    pub modpack_origin: ModpackOrigin,
    pub import_source: ImportSource,
    pub share_code: String,
    pub import_count: usize,
    pub import_mode: ImportMode,
    pub import_screening: Option<Vec<ScreenedInstance>>,
    pub import_screening_error: Option<String>,
    pub improving: bool,
    pub improve_targets: Option<Vec<BundleTarget>>,
    pub bundle_page: usize,
    pub finishing: bool,
    pub modpack_file: Option<PathBuf>,
    pub folder: FolderChoice,
    pub kind: ClusterKind,
    pub name: String,
    pub suggested: String,
    pub versions: VersionPicks,
    pub loader: LoaderPicks,
    pub bundles: BundlePicks,
}

impl Picks {
    pub fn flagged_count(&self) -> usize {
        self.import_screening
            .iter()
            .flatten()
            .map(|screened| screened.flagged.len())
            .sum()
    }

    pub fn unpublished_count(&self) -> usize {
        self.import_screening
            .iter()
            .flatten()
            .map(|screened| screened.unpublished.len())
            .sum()
    }

    pub fn unconfirmed_count(&self) -> usize {
        self.import_screening
            .iter()
            .flatten()
            .map(|screened| screened.unconfirmed)
            .sum()
    }

    pub fn bundle_target(&self) -> Option<&BundleTarget> {
        if !self.improving || self.step != Step::Bundles {
            return None;
        }
        self.improve_targets.as_ref()?.get(self.bundle_page)
    }

    pub fn last_bundle_page(&self) -> bool {
        self.improve_targets
            .as_ref()
            .is_some_and(|targets| self.bundle_page + 1 >= targets.len())
    }

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

    fn offers(&self, version: &str, loader: GameLoader) -> bool {
        self.oneclient
            .iter()
            .any(|entry| entry.id == version && entry.loader == loader)
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
        oneclient
            .iter()
            .map(|entry| (entry.id.clone(), entry.loader))
            .collect()
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
        oneclient_settled: metadata.is_some() || query_error(&oneclient_query).is_some(),
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
        (TypeChoice::Scratch, Some(_)) | (TypeChoice::Modpack | TypeChoice::Import, _) => {
            ClusterKind::Modded
        }
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
    let import_mode = *w.import_mode.read();
    let import_source = *w.import_source.read();
    let modpack_origin = *w.modpack_origin.read();
    let steps = step_order(choice, import_source, import_mode, modpack_origin);
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
        TypeChoice::Scratch | TypeChoice::Modpack | TypeChoice::Import => {
            w.loader
                .read()
                .resolve_exact(available.as_deref().unwrap_or(&[]), w.exact_loader)
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

    let improving = improves(steps, import_mode);
    let pack = choice == TypeChoice::Modpack;
    let modpack_file = w.modpack_file.read().clone();
    let chosen = w.import_chosen.read().clone();
    let header = use_modpack_header(modpack_file.clone().filter(|_| improving && pack));
    let improve_targets: Option<Vec<BundleTarget>> = if !improving || !catalogue.oneclient_settled {
        None
    } else if pack {
        header.map(|header| {
            header
                .filter(|(version, loader)| catalogue.offers(version, *loader))
                .zip(modpack_file.clone())
                .map(|((mc_version, loader), key)| BundleTarget {
                    name: key
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    key,
                    mc_version,
                    loader,
                })
                .into_iter()
                .collect()
        })
    } else {
        Some(
            chosen
                .iter()
                .filter(|instance| catalogue.offers(&instance.mc_version, instance.loader))
                .map(|instance| BundleTarget {
                    key: instance.game_dir.clone(),
                    name: instance.name.clone(),
                    mc_version: instance.mc_version.clone(),
                    loader: instance.loader,
                })
                .collect(),
        )
    };
    let bundle_page = *w.bundle_page.read();
    let target = improve_targets
        .as_ref()
        .filter(|_| step == Step::Bundles)
        .and_then(|targets| targets.get(bundle_page));

    let screening = improving && (step == Step::Bundles || *w.finishing.read());
    let screening_query = use_import_screening(
        if screening && !pack {
            chosen
        } else {
            Vec::new()
        },
        modpack_file.clone().filter(|_| screening && pack),
    );
    let (import_screening, import_screening_error) = if screening {
        (
            import_screening(&screening_query),
            query_error(&screening_query),
        )
    } else {
        (None, None)
    };

    let bundles = match target {
        Some(target) => {
            use_bundle_context(Some(&target.mc_version), Some(target.loader), true, true)
        }
        None => use_bundle_context(version.as_ref(), loader, oneclient, step == Step::Bundles),
    };

    let kind = kind_for(choice, loader);
    let suggested = suggested_name(version.as_ref(), kind, loader);
    let chosen_declined = match target {
        Some(target) => w.import_declined.read().get(&target.key).cloned(),
        None => w.declined.read().clone(),
    };
    let declined = chosen_declined.unwrap_or_else(|| {
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
        modpack_origin,
        import_source,
        share_code: w.share_code.read().clone(),
        import_count: w.import_chosen.read().len(),
        import_mode,
        import_screening,
        import_screening_error,
        improving,
        improve_targets,
        bundle_page,
        finishing: *w.finishing.read(),
        modpack_file,
        folder: w.folder.read().clone(),
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

pub fn import_jobs(
    screening: &[ScreenedInstance],
    chosen: &[ExternalInstance],
    bundles: &HashMap<PathBuf, Vec<String>>,
) -> Vec<ExternalImportJob> {
    chosen
        .iter()
        .map(|instance| {
            let mut job = ExternalImportJob::as_is(instance.clone());
            let screened = screening
                .iter()
                .find(|screened| screened.game_dir == instance.game_dir);
            job.bundles = bundles.get(&instance.game_dir).cloned();
            job.reviewed = screened
                .filter(|screened| screened.unconfirmed == 0)
                .map(|screened| (screened.mods_stamp, screened.unpublished.clone()));
            let mut picks = Vec::new();
            if let Some(screened) = screened {
                apply_replacements(screened, &mut job.choices.skip, &mut picks);
            }
            job.alternatives = picks
                .into_iter()
                .map(|(provider, project_id, chosen)| AlternativePick {
                    name: chosen.map_or_else(|| project_id.clone(), |a| a.resolved.name.clone()),
                    provider,
                    project_id,
                })
                .collect();
            job
        })
        .collect()
}

pub fn pack_choices(screening: &[ScreenedInstance], bundles: Option<Vec<String>>) -> PackChoices {
    let mut choices = PackChoices::default();
    let mut picks = Vec::new();
    for screened in screening {
        apply_replacements(screened, &mut choices.skipped, &mut picks);
    }
    choices.alternatives = picks
        .into_iter()
        .filter_map(|(_, _, chosen)| chosen.map(|a| a.resolved.clone()))
        .collect();
    choices.bundles = bundles;
    choices
}

type Pick<'a> = (ProviderId, String, Option<&'a ImportAlternative>);

fn apply_replacements<'a>(
    screened: &'a ScreenedInstance,
    skip: &mut HashSet<String>,
    picks: &mut Vec<Pick<'a>>,
) {
    for flagged in &screened.flagged {
        let Some(chosen) = default_replacement(flagged) else {
            continue;
        };
        skip.insert(flagged.hash.clone());
        let provider = chosen.resolved.provider;
        let project_id = &chosen.resolved.project_id;
        let picked = picks
            .iter()
            .any(|(p, id, _)| *p == provider && id == project_id);
        if !picked && !chosen.already_installed {
            picks.push((provider, project_id.clone(), Some(chosen)));
        }
    }
}

#[cfg(test)]
mod tests {
    use oneclient_content::packages::ResolvedAlternative;
    use oneclient_core::{FlaggedImportMod, ImportAlternative};

    use super::*;

    fn alternative(project_id: &str, has_build: bool, already_installed: bool) -> ImportAlternative {
        ImportAlternative {
            resolved: ResolvedAlternative {
                provider: ProviderId::Modrinth,
                project_id: project_id.to_string(),
                name: project_id.to_string(),
                version_id: has_build.then(|| "v1".to_string()),
                version_number: has_build.then(|| "1.0".to_string()),
                icon_url: None,
            },
            already_installed,
        }
    }

    fn flagged(hash: &str, alternatives: Vec<ImportAlternative>) -> FlaggedImportMod {
        FlaggedImportMod {
            hash: hash.to_string(),
            file_name: hash.to_string(),
            name: hash.to_string(),
            enabled: true,
            explanation: None,
            alternatives,
        }
    }

    #[test]
    fn flagged_mods_are_replaced_automatically_and_kept_without_a_build() {
        let screened = ScreenedInstance {
            game_dir: PathBuf::from("pack.mrpack"),
            name: "Pack".to_string(),
            mc_version: "1.21.1".to_string(),
            flagged: vec![
                flagged("no-build", vec![alternative("missing", false, false)]),
                flagged("first", vec![alternative("better", true, false)]),
                flagged("second", vec![alternative("better", true, false)]),
                flagged("shipped", vec![alternative("in-pack", false, true)]),
            ],
            unpublished: Vec::new(),
            unconfirmed: 0,
            mods_stamp: 0,
        };

        let choices = pack_choices(&[screened], Some(vec!["Core".to_string()]));

        assert_eq!(
            choices.skipped,
            HashSet::from(["first", "second", "shipped"].map(String::from))
        );
        let installed: Vec<&str> = choices
            .alternatives
            .iter()
            .map(|a| a.project_id.as_str())
            .collect();
        assert_eq!(installed, ["better"]);
        assert_eq!(choices.bundles, Some(vec!["Core".to_string()]));
    }

    fn instance(dir: &str) -> ExternalInstance {
        ExternalInstance {
            launcher: oneclient_core::ExternalLauncher::Prism,
            id: dir.to_string(),
            name: dir.to_string(),
            game_dir: PathBuf::from(dir),
            mc_version: "1.21.1".to_string(),
            loader: GameLoader::Fabric,
            loader_version: None,
            icon: None,
            notes: None,
            groups: Vec::new(),
            played_secs: 0,
            settings: Default::default(),
            linked_pack: None,
            unsupported_loader: None,
            skipped_parts: Vec::new(),
        }
    }

    fn screened(dir: &str, unconfirmed: usize) -> ScreenedInstance {
        ScreenedInstance {
            game_dir: PathBuf::from(dir),
            name: dir.to_string(),
            mc_version: "1.21.1".to_string(),
            flagged: vec![flagged(dir, vec![alternative("better", true, false)])],
            unpublished: Vec::new(),
            unconfirmed,
            mods_stamp: 7,
        }
    }

    #[test]
    fn each_import_gets_its_own_bundles_and_only_a_confirmed_check_is_reused() {
        let screening = [screened("a", 0), screened("b", 2)];
        let bundles = HashMap::from([(PathBuf::from("a"), vec!["Core".to_string()])]);

        let jobs = import_jobs(&screening, &[instance("a"), instance("b")], &bundles);

        assert_eq!(jobs[0].bundles, Some(vec!["Core".to_string()]));
        assert_eq!(jobs[1].bundles, None);
        assert_eq!(jobs[0].reviewed, Some((7, Vec::new())));
        assert_eq!(jobs[1].reviewed, None);
        assert!(jobs[0].choices.skip.contains("a"));
        assert!(jobs[1].choices.skip.contains("b"));
    }
}
