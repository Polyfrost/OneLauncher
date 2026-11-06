use std::collections::{HashMap, HashSet, VecDeque};

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::models::ClusterRow;
use oneclient_events::VersionRange;

use crate::bundles::BundlesManager;
use crate::bundles::install::resolve_bundle_version;
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::local_manifest::read_jar_mod_id;
use crate::packages::metadata_cache::with_recorded_dependencies;
use crate::packages::provider::PackageProvider;
use crate::packages::store::{PackageStore, artifact_absolute_path};
use crate::packages::types::{
    DependencyKind, ProjectDetail, ReleaseType, SearchFilters, VersionDependency, VersionDetail,
    VersionSummary,
};
use crate::packages::updates::{BundlePins, cluster_bundle_pins, unfit_pins};
use crate::packages::{cached_project_detail, get_version_cached};

/// Caps the transitive walk so a provider serving a cycle can't spin forever
const MAX_DEPTH: usize = 4;

const VERSION_WINDOW: usize = 50;

const PRERELEASE_TAGS: [&str; 6] = ["alpha", "beta", "rc", "pre", "snapshot", "dev"];

#[derive(Debug, Clone)]
pub struct ResolvedDependency {
    pub project: ProjectDetail,
    pub version: VersionDetail,
}

#[derive(Debug, Clone, Default)]
pub struct DependencyResolution {
    /// Ordered breadth-first from the root
    /// already-present ones are filtered out
    pub install: Vec<ResolvedDependency>,
    pub unresolved: Vec<String>,
    pub exact: Vec<(String, String)>,
    pub switched_off: Vec<String>,
}

impl DependencyResolution {
    pub fn is_empty(&self) -> bool {
        self.install.is_empty() && self.unresolved.is_empty()
    }
}

#[tracing::instrument(level = "debug", skip(project, root, bundles, ctx), fields(project_id = %root.project_id, version_id = %root.version_id))]
pub async fn resolve_required(
    provider_id: ProviderId,
    project: &ProjectDetail,
    root: &VersionDetail,
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<DependencyResolution> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let mut pins = cluster_bundle_pins(&cluster, bundles, ctx)
        .await
        .unwrap_or_else(|err| {
            tracing::debug!(%err, "resolving dependencies with no readable pins");
            BundlePins::new()
        });

    loop {
        let resolution =
            resolve_required_or_unpinned(provider_id, root, cluster_id, &mut pins, ctx).await?;
        let unfit = unfit_pins(
            provider_id,
            project,
            root,
            &resolution.install,
            &pins,
            &cluster,
            ctx,
        )
        .await
        .unwrap_or_else(|err| {
            tracing::debug!(%err, "could not check a dependency's pin");
            Vec::new()
        });
        if unfit.is_empty() {
            return Ok(resolution);
        }
        for pin in unfit {
            pins.remove(&pin);
        }
    }
}

pub(crate) async fn resolve_required_or_unpinned(
    provider_id: ProviderId,
    root: &VersionDetail,
    cluster_id: i64,
    pins: &mut BundlePins,
    ctx: &ContentCtx,
) -> ContentResult<DependencyResolution> {
    loop {
        let resolution = resolve_required_pinned(provider_id, root, cluster_id, pins, ctx).await?;
        let before = pins.len();
        for project_id in &resolution.unresolved {
            pins.remove(&(provider_id, project_id.clone()));
        }
        if pins.len() == before {
            return Ok(resolution);
        }
    }
}

pub(crate) async fn resolve_required_pinned(
    provider_id: ProviderId,
    root: &VersionDetail,
    cluster_id: i64,
    pins: &BundlePins,
    ctx: &ContentCtx,
) -> ContentResult<DependencyResolution> {
    let mut resolution = DependencyResolution::default();
    if !root.dependencies.iter().any(is_required) {
        return Ok(resolution);
    }

    let provider = ctx.providers.get(provider_id)?;
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;

    let releases: Vec<_> = artifact_dao::list_cluster_releases(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .filter(|release| release.provider == provider_id as i64)
        .collect();
    let running: HashSet<&str> = releases
        .iter()
        .filter(|release| release.enabled != 0)
        .map(|release| release.project_id.as_str())
        .collect();
    let switched_off: HashMap<&str, &str> = releases
        .iter()
        .filter(|release| !running.contains(release.project_id.as_str()))
        .map(|release| (release.project_id.as_str(), release.display_name.as_str()))
        .collect();

    // Seeded with installed projects and the root against refetches and self-referencing graphs
    let mut seen: HashSet<String> = releases
        .iter()
        .map(|release| release.project_id.clone())
        .collect();
    seen.insert(root.project_id.clone());

    // Read only once something fails to resolve, it opens every jar in the cluster
    let mut installed_mod_ids: Option<HashSet<String>> = None;

    let mut queue: VecDeque<(VersionDependency, usize)> = root
        .dependencies
        .iter()
        .filter(|dep| is_required(dep))
        .map(|dep| (dep.clone(), 1))
        .collect();

    while let Some((dep, depth)) = queue.pop_front() {
        if let Some(project_id) = &dep.project_id
            && seen.contains(project_id)
        {
            if let Some(version_id) = dep.version_id {
                resolution.exact.push((project_id.clone(), version_id));
            } else if let Some(name) = switched_off.get(project_id.as_str()) {
                resolution.switched_off.push((*name).to_string());
            }
            continue;
        }

        let resolved = match resolve_dependency(provider, &dep, &cluster, pins, ctx).await {
            Ok(Some(resolved)) => resolved,
            Ok(None) => {
                if !installed_under_mod_id(provider, &dep, cluster_id, &mut installed_mod_ids, ctx)
                    .await
                {
                    tracing::warn!(?dep, "no compatible version for dependency");
                    resolution.unresolved.push(dependency_label(&dep));
                }
                continue;
            }
            Err(err) => {
                if !installed_under_mod_id(provider, &dep, cluster_id, &mut installed_mod_ids, ctx)
                    .await
                {
                    tracing::warn!(?dep, %err, "failed to resolve dependency");
                    resolution.unresolved.push(dependency_label(&dep));
                }
                continue;
            }
        };

        if let Some(version_id) = &dep.version_id {
            resolution
                .exact
                .push((resolved.version.project_id.clone(), version_id.clone()));
        }

        // A version-pinned dependency only reveals its project once fetched so dedupe again here
        if !seen.insert(resolved.version.project_id.clone()) {
            continue;
        }

        if depth < MAX_DEPTH {
            for next in resolved
                .version
                .dependencies
                .iter()
                .filter(|d| is_required(d))
            {
                queue.push_back((next.clone(), depth + 1));
            }
        }

        resolution.install.push(resolved);
    }

    Ok(resolution)
}

fn is_required(dep: &VersionDependency) -> bool {
    dep.kind == DependencyKind::Required
}

fn dependency_label(dep: &VersionDependency) -> String {
    dep.project_id
        .clone()
        .or_else(|| dep.version_id.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

async fn resolve_dependency(
    provider: &dyn PackageProvider,
    dep: &VersionDependency,
    cluster: &ClusterRow,
    pins: &BundlePins,
    ctx: &ContentCtx,
) -> ContentResult<Option<ResolvedDependency>> {
    let (project_id, version) = match (&dep.project_id, &dep.version_id) {
        (project_id, Some(version_id)) => {
            let project_id = match project_id {
                Some(project_id) => project_id.clone(),
                None => {
                    artifact_dao::get_release_by_version(&ctx.db, provider.id() as i64, version_id)
                        .await?
                        .map(|release| release.project_id)
                        .unwrap_or_default()
                }
            };
            let version = get_version_cached(ctx, provider.id(), &project_id, version_id).await?;
            (version.project_id.clone(), version)
        }
        (Some(project_id), None) => {
            let Some(pin) = pins.get(&(provider.id(), project_id.clone())) else {
                return resolve_one(provider, dep, cluster, ctx).await;
            };
            let version =
                resolve_bundle_version(provider.id(), project_id, &pin.version_id, &pin.sha1, ctx)
                    .await?;
            (project_id.clone(), version)
        }
        (None, None) => return Ok(None),
    };

    let version = with_recorded_dependencies(ctx, provider.id(), version).await?;
    let project = dependency_project(provider, &project_id, &version, ctx).await?;
    Ok(Some(ResolvedDependency { project, version }))
}

async fn dependency_project(
    provider: &dyn PackageProvider,
    project_id: &str,
    version: &VersionDetail,
    ctx: &ContentCtx,
) -> ContentResult<ProjectDetail> {
    let artifact = match version.primary_file() {
        Some(file) => {
            artifact_dao::get_artifact_by_hash(&ctx.db, &polyio::normalize_hash(&file.sha1)).await?
        }
        None => None,
    };

    match artifact.and_then(|artifact| ContentType::from_repr(artifact.content_type as u8)) {
        Some(content_type) => {
            Ok(cached_project_detail(ctx, provider.id(), project_id, content_type).await)
        }
        None => provider.get_project(project_id, ctx).await,
    }
}

pub(crate) async fn resolve_one(
    provider: &dyn PackageProvider,
    dep: &VersionDependency,
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> ContentResult<Option<ResolvedDependency>> {
    // A pinned version wins even when newer exists
    // providers ignore the project id once the version id is known
    let version = match &dep.version_id {
        Some(version_id) => {
            provider
                .get_version(
                    dep.project_id.as_deref().unwrap_or_default(),
                    version_id,
                    ctx,
                )
                .await?
        }
        None => {
            let Some(project_id) = dep.project_id.as_deref() else {
                return Ok(None);
            };
            let Some(pick) = pick_version(provider, project_id, cluster, ctx).await? else {
                return Ok(None);
            };
            provider
                .get_version(project_id, &pick.version_id, ctx)
                .await?
        }
    };

    let project = provider.get_project(&version.project_id, ctx).await?;
    Ok(Some(ResolvedDependency { project, version }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModInstallTarget {
    pub project_id: String,
    pub version_id: String,
    pub name: String,
}

const MOD_SEARCH_LIMIT: usize = 10;

pub async fn find_mod_for_cluster(
    provider: &dyn PackageProvider,
    mod_id: &str,
    name: Option<&str>,
    range: Option<&VersionRange>,
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<Option<ModInstallTarget>> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;

    let project = match provider.get_project(mod_id, ctx).await {
        Ok(project) if project.content_type == ContentType::Mod => Some((project.id, project.name)),
        Ok(project) => {
            tracing::debug!(
                mod_id,
                content_type = ?project.content_type,
                "the mod id names a project that is not a mod, searching instead"
            );
            search_mod(provider, mod_id, name, &cluster, ctx).await?
        }
        Err(err) => {
            tracing::debug!(
                mod_id,
                "no project under the mod id, searching instead: {err}"
            );
            search_mod(provider, mod_id, name, &cluster, ctx).await?
        }
    };

    let Some((project_id, name)) = project else {
        return Ok(None);
    };

    let Some(version) = pick_mod_version(provider, &project_id, range, &cluster, ctx).await? else {
        return Ok(None);
    };

    Ok(Some(ModInstallTarget {
        project_id,
        version_id: version.version_id,
        name,
    }))
}

async fn pick_mod_version(
    provider: &dyn PackageProvider,
    project_id: &str,
    range: Option<&VersionRange>,
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> ContentResult<Option<VersionSummary>> {
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Vanilla);
    if !loader.is_modded() {
        return Ok(None);
    }

    let mut candidates = provider
        .list_versions(
            project_id,
            Some(&cluster.mc_version),
            Some(loader),
            0,
            VERSION_WINDOW,
            ctx,
        )
        .await?
        .items;

    candidates.retain(|version| {
        !version.loaders.is_empty()
            && range
                .is_none_or(|range| satisfies(range, &version.version_number, &cluster.mc_version))
    });

    Ok(choose_version(candidates, cluster, loader))
}

fn numeric_parts(text: &str) -> Option<Vec<u64>> {
    let text = text.trim().trim_start_matches(['v', 'V']);
    let parts: Option<Vec<u64>> = text.split('.').map(|part| part.parse().ok()).collect();

    parts.filter(|parts| !parts.is_empty())
}

fn prerelease_identifiers(token: &str) -> Option<Vec<String>> {
    let lowered = token.to_ascii_lowercase();
    PRERELEASE_TAGS
        .iter()
        .any(|tag| lowered.starts_with(tag))
        .then(|| token.split('.').map(str::to_owned).collect())
}

#[derive(Debug, PartialEq, Eq)]
struct Release {
    core: Vec<u64>,
    prerelease: Option<Vec<String>>,
}

fn release_number(version: &str, mc_version: &str) -> Option<Release> {
    let mut tokens = version
        .split(['+', '-', '_', ' '])
        .filter(|token| !token.is_empty())
        .filter(|token| !token.to_ascii_lowercase().starts_with("mc"))
        .filter(|token| *token != mc_version);

    let core = tokens.by_ref().find_map(numeric_parts)?;
    let prerelease = match tokens.next() {
        Some(token) => prerelease_identifiers(token),
        None => version.ends_with('-').then(Vec::new),
    };

    Some(Release { core, prerelease })
}

fn compare_parts(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    let len = a.len().max(b.len());
    (0..len)
        .map(|index| a.get(index).unwrap_or(&0).cmp(b.get(index).unwrap_or(&0)))
        .find(|ordering| ordering.is_ne())
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn compare_prereleases(a: &[String], b: &[String]) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    a.iter()
        .zip(b)
        .map(|(a, b)| match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(a), Ok(b)) => a.cmp(&b),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => a.cmp(b),
        })
        .find(|ordering| ordering.is_ne())
        .unwrap_or_else(|| a.len().cmp(&b.len()))
}

fn compare_releases(a: &Release, b: &Release) -> std::cmp::Ordering {
    compare_parts(&a.core, &b.core).then_with(|| match (&a.prerelease, &b.prerelease) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(a), Some(b)) => compare_prereleases(a, b),
    })
}

fn semver_release(version: &str) -> Option<Release> {
    let version = version
        .split_once('+')
        .map_or(version, |(version, _)| version);
    let (core, prerelease) = match version.split_once('-') {
        Some((core, "")) => (core, Some(Vec::new())),
        Some((core, tag)) => (core, Some(tag.split('.').map(str::to_owned).collect())),
        None => (version, None),
    };

    let identifier = |part: &String| {
        !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    if !core.starts_with(|c: char| c.is_ascii_digit())
        || !prerelease.iter().flatten().all(identifier)
    {
        return None;
    }

    Some(Release {
        core: numeric_parts(core)?,
        prerelease,
    })
}

pub(crate) fn compare_semver(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    Some(compare_releases(&semver_release(a)?, &semver_release(b)?))
}

fn satisfies(range: &VersionRange, version: &str, mc_version: &str) -> bool {
    release_number(version, mc_version)
        .is_some_and(|found| within(range, &found, |bound| release_number(bound, "")))
}

fn within(range: &VersionRange, found: &Release, read: impl Fn(&str) -> Option<Release>) -> bool {
    let bound = |bound: &Option<String>| bound.as_deref().and_then(&read);

    if let Some(min) = bound(&range.min) {
        let ordering = compare_releases(found, &min);
        if ordering.is_lt() || (ordering.is_eq() && !range.min_inclusive) {
            return false;
        }
    }

    if let Some(max) = bound(&range.max) {
        let ordering = compare_releases(found, &max);
        if ordering.is_gt() || (ordering.is_eq() && !range.max_inclusive) {
            return false;
        }
    }

    true
}

pub(crate) fn rules_out(ranges: &[VersionRange], version: &str, mc_version: &str) -> bool {
    release_number(version, mc_version).is_some()
        && !ranges
            .iter()
            .any(|range| satisfies(range, version, mc_version))
}

fn semver_within(range: &VersionRange, version: &str) -> bool {
    use std::cmp::Ordering;

    let holds = |bound: &Option<String>, inclusive: bool, beyond: Ordering| {
        let Some(bound) = bound else {
            return true;
        };
        match compare_semver(version, bound) {
            Some(ordering) => ordering != beyond && (ordering.is_ne() || inclusive),
            None => inclusive && version == bound,
        }
    };

    holds(&range.min, range.min_inclusive, Ordering::Less)
        && holds(&range.max, range.max_inclusive, Ordering::Greater)
}

pub(crate) fn semver_rules_out(ranges: &[VersionRange], version: &str) -> bool {
    !ranges.iter().any(|range| semver_within(range, version))
}

pub(crate) fn number_among(ranges: &[VersionRange], version: &str, mc_version: &str) -> bool {
    ranges
        .iter()
        .any(|range| satisfies(range, version, mc_version))
}

const MAVEN_QUALIFIERS: [&str; 7] = ["alpha", "beta", "milestone", "rc", "snapshot", "", "sp"];
const MAVEN_RELEASE: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
enum MavenItem {
    Number(u128),
    Text(String),
    List(Vec<MavenItem>),
}

impl MavenItem {
    fn read(text: &str, digits: bool, before_digit: bool) -> Self {
        if digits {
            return Self::Number(text.parse().unwrap_or(u128::MAX));
        }
        Self::Text(
            match text {
                "a" if before_digit => "alpha",
                "b" if before_digit => "beta",
                "m" if before_digit => "milestone",
                "ga" | "final" | "release" => "",
                "cr" => "rc",
                other => other,
            }
            .to_string(),
        )
    }

    fn is_null(&self) -> bool {
        self.against_nothing().is_eq() && !matches!(self, Self::List(items) if !items.is_empty())
    }

    fn against_nothing(&self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match self {
            Self::Number(number) => number.cmp(&0),
            Self::Text(text) => maven_rank(text).0.cmp(&MAVEN_RELEASE),
            Self::List(items) => items
                .iter()
                .map(Self::against_nothing)
                .find(|ordering| ordering.is_ne())
                .unwrap_or(Ordering::Equal),
        }
    }
}

fn maven_rank(text: &str) -> (usize, &str) {
    MAVEN_QUALIFIERS
        .iter()
        .position(|known| *known == text)
        .map_or((MAVEN_QUALIFIERS.len(), text), |rank| (rank, ""))
}

fn open_qualifier(lists: &mut Vec<Vec<MavenItem>>) {
    if lists.last().is_some_and(|list| !list.is_empty()) {
        lists.push(Vec::new());
    }
}

fn maven_items(version: &str) -> Vec<MavenItem> {
    let version = version.trim().to_ascii_lowercase();
    let mut lists = vec![Vec::new()];
    let mut start = 0;
    let mut digits = false;

    for (index, c) in version.char_indices() {
        if matches!(c, '.' | '-') {
            let text = &version[start..index];
            lists.last_mut().unwrap().push(if text.is_empty() {
                MavenItem::Number(0)
            } else {
                MavenItem::read(text, digits, false)
            });
            start = index + 1;
            if c == '-' {
                lists.push(Vec::new());
            }
            continue;
        }

        let digit = c.is_ascii_digit();
        if digit != digits && index > start {
            if digit {
                open_qualifier(&mut lists);
            }
            let item = MavenItem::read(&version[start..index], digits, digit);
            lists.last_mut().unwrap().push(item);
            lists.push(Vec::new());
            start = index;
        }
        digits = digit;
    }
    if start < version.len() {
        if !digits {
            open_qualifier(&mut lists);
        }
        let item = MavenItem::read(&version[start..], digits, false);
        lists.last_mut().unwrap().push(item);
    }

    lists.into_iter().rev().fold(Vec::new(), |inner, mut list| {
        list.push(MavenItem::List(inner));
        for index in (0..list.len()).rev() {
            if list[index].is_null() {
                list.remove(index);
            } else if !matches!(list[index], MavenItem::List(_)) {
                break;
            }
        }
        list
    })
}

fn compare_maven_items(a: &[MavenItem], b: &[MavenItem]) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    (0..a.len().max(b.len()))
        .map(|index| match (a.get(index), b.get(index)) {
            (Some(a), Some(b)) => match (a, b) {
                (MavenItem::Number(a), MavenItem::Number(b)) => a.cmp(b),
                (MavenItem::Text(a), MavenItem::Text(b)) => maven_rank(a).cmp(&maven_rank(b)),
                (MavenItem::List(a), MavenItem::List(b)) => compare_maven_items(a, b),
                (MavenItem::Number(_), _) | (MavenItem::List(_), MavenItem::Text(_)) => {
                    Ordering::Greater
                }
                (MavenItem::Text(_) | MavenItem::List(_), _) => Ordering::Less,
            },
            (Some(a), None) => a.against_nothing(),
            (None, Some(b)) => b.against_nothing().reverse(),
            (None, None) => Ordering::Equal,
        })
        .find(|ordering| ordering.is_ne())
        .unwrap_or(Ordering::Equal)
}

pub(crate) fn maven_fits(ranges: &[VersionRange], version: &str) -> Option<bool> {
    use std::cmp::Ordering;

    if version.contains("${") || !version.contains(|c: char| c.is_ascii_digit()) {
        return None;
    }

    let found = maven_items(version);
    let holds = |bound: &Option<String>, inclusive: bool, beyond: Ordering| {
        bound.as_deref().is_none_or(|bound| {
            let ordering = compare_maven_items(&found, &maven_items(bound));
            ordering != beyond && (ordering.is_ne() || inclusive)
        })
    };

    Some(ranges.iter().any(|range| {
        holds(&range.min, range.min_inclusive, Ordering::Less)
            && holds(&range.max, range.max_inclusive, Ordering::Greater)
    }))
}

async fn search_mod(
    provider: &dyn PackageProvider,
    mod_id: &str,
    name: Option<&str>,
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> ContentResult<Option<(String, String)>> {
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Vanilla);
    let query = name.unwrap_or(mod_id).replace(['_', '-'], " ");

    let hits = provider
        .search(
            &SearchFilters {
                query: Some(query),
                limit: Some(MOD_SEARCH_LIMIT),
                content_type: Some(ContentType::Mod),
                game_versions: Some(vec![cluster.mc_version.clone()]),
                loaders: loader.is_modded().then(|| vec![loader]),
                ..SearchFilters::default()
            },
            ctx,
        )
        .await?
        .items;

    let wanted: Vec<String> = std::iter::once(mod_id)
        .chain(name)
        .map(squash)
        .filter(|key| !key.is_empty())
        .collect();

    Ok(hits
        .into_iter()
        .find(|hit| wanted.contains(&squash(&hit.slug)) || wanted.contains(&squash(&hit.name)))
        .map(|hit| (hit.id, hit.name)))
}

fn squash(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Falls back to the newest prerelease when no release fits the cluster
pub async fn pick_version(
    provider: &dyn PackageProvider,
    project_id: &str,
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> ContentResult<Option<VersionSummary>> {
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Vanilla);
    let loader_filter = loader.is_modded().then_some(loader);

    let mut candidates = provider
        .list_versions(
            project_id,
            Some(&cluster.mc_version),
            loader_filter,
            0,
            VERSION_WINDOW,
            ctx,
        )
        .await?
        .items;

    // Resource packs shaders and datapacks declare no loader so a loader-filtered query is empty
    if candidates.is_empty() && loader_filter.is_some() {
        candidates = provider
            .list_versions(
                project_id,
                Some(&cluster.mc_version),
                None,
                0,
                VERSION_WINDOW,
                ctx,
            )
            .await?
            .items;
    }

    Ok(choose_version(candidates, cluster, loader))
}

fn choose_version(
    mut candidates: Vec<VersionSummary>,
    cluster: &ClusterRow,
    loader: GameLoader,
) -> Option<VersionSummary> {
    candidates.retain(|version| fits_cluster(version, cluster, loader));
    // Neither provider promises an order so sort rather than trust the page
    candidates.sort_by_key(|version| std::cmp::Reverse(version.published));

    candidates
        .iter()
        .find(|version| matches!(version.release_type, ReleaseType::Release))
        .or_else(|| candidates.first())
        .cloned()
}

#[must_use]
pub(crate) fn base_game_version(version: &str) -> &str {
    version
        .split_once('-')
        .filter(|(base, _)| base.contains('.'))
        .map_or(version, |(base, _)| base)
}

#[must_use]
pub(crate) fn supports_game_version(game_versions: &[String], mc_version: &str) -> bool {
    let wanted = base_game_version(mc_version);
    game_versions.is_empty() || game_versions.iter().any(|v| base_game_version(v) == wanted)
}

fn fits_cluster(version: &VersionSummary, cluster: &ClusterRow, loader: GameLoader) -> bool {
    if !supports_game_version(&version.game_versions, &cluster.mc_version) {
        return false;
    }

    version.loaders.is_empty() || version.loaders.iter().any(|l| loader.compatible_with(*l))
}

/// Bundles can ship a library from outside the provider, like Fabric Language Kotlin on 1.8.9
/// which comes from GitHub because Modrinth has no build for it. The project id check can't see
/// that copy, so a dependency the provider can't serve is also matched by its slug against the
/// mod ids of the jars already in the cluster
async fn installed_under_mod_id(
    provider: &dyn PackageProvider,
    dep: &VersionDependency,
    cluster_id: i64,
    installed: &mut Option<HashSet<String>>,
    ctx: &ContentCtx,
) -> bool {
    let Some(project_id) = &dep.project_id else {
        return false;
    };
    let Ok(project) = provider.get_project(project_id, ctx).await else {
        return false;
    };

    if installed.is_none() {
        *installed = Some(installed_mod_ids(cluster_id, ctx).await);
    }
    installed
        .as_ref()
        .is_some_and(|ids| ids.contains(&project.slug))
}

async fn installed_mod_ids(cluster_id: i64, ctx: &ContentCtx) -> HashSet<String> {
    let Ok(linked) = PackageStore::list_linked_artifacts(cluster_id, ctx).await else {
        return HashSet::new();
    };

    let mut ids = HashSet::new();
    for artifact in linked
        .iter()
        .filter(|a| a.enabled && a.content_type == ContentType::Mod)
    {
        let Ok(Some(row)) = artifact_dao::get_artifact_by_hash(&ctx.db, &artifact.hash).await
        else {
            continue;
        };
        let Ok(jar) = artifact_absolute_path(&row.path) else {
            continue;
        };
        if let Some(id) = read_jar_mod_id(&jar).await {
            ids.insert(id.id);
        }
    }
    ids
}

/// False for modpacks they bring their own file list
pub fn resolves_dependencies(content_type: ContentType) -> bool {
    !matches!(content_type, ContentType::Modpack)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    #[test]
    fn a_version_is_offered_only_for_the_game_version_it_names() {
        let offered = |stated: &[&str], mc_version: &str| {
            supports_game_version(
                &stated.iter().map(|v| (*v).to_string()).collect::<Vec<_>>(),
                mc_version,
            )
        };

        assert!(offered(&[], "26.3"), "an undescribed version is offered");
        assert!(offered(&["26.3"], "26.3"));
        assert!(
            offered(&["26.3-rc-1"], "26.3"),
            "a pre-release tag names the same game version"
        );
        assert!(offered(&["26.3"], "26.3-snapshot-10"));
        assert!(
            !offered(&["26.1"], "26.1.2"),
            "strict where the launch-time check is lenient: #830 stopped 26.1 \
			 builds being offered to a 26.1.2 cluster"
        );
        assert!(!offered(&["1.21.11"], "26.1.2"));
        assert!(
            !offered(&["rd-132211"], "rd-160052"),
            "pre-classic names the build after the dash, so there is no tag to strip"
        );
    }

    fn summary(
        version_id: &str,
        release_type: ReleaseType,
        day: u32,
        loaders: Vec<GameLoader>,
        game_versions: Vec<&str>,
    ) -> VersionSummary {
        VersionSummary {
            version_id: version_id.into(),
            project_id: "p".into(),
            name: version_id.into(),
            version_number: version_id.into(),
            published: Utc.with_ymd_and_hms(2025, 1, day, 0, 0, 0).unwrap(),
            release_type,
            game_versions: game_versions.into_iter().map(Into::into).collect(),
            loaders,
            downloads: 0,
            file_size: 0,
            dependencies: Vec::new(),
        }
    }

    fn cluster(loader: GameLoader, mc_version: &str) -> ClusterRow {
        ClusterRow {
            id: 1,
            name: "c".into(),
            folder_name: "c".into(),
            setting_profile_name: None,
            mc_version: mc_version.into(),
            mc_loader: loader as i64,
            stage: 0,
            mc_loader_version: None,
            created_at: None,
            last_played: None,
            overall_played: None,
            linked_modpack_hash: None,
            kind: 0,
            user_created: 0,
            description: None,
            tags: "[]".into(),
            cover_path: None,
        }
    }

    #[test]
    fn prefers_the_newest_release_over_a_newer_prerelease() {
        let cluster = cluster(GameLoader::Fabric, "1.21.4");
        let candidates = vec![
            summary(
                "old",
                ReleaseType::Release,
                1,
                vec![GameLoader::Fabric],
                vec!["1.21.4"],
            ),
            summary(
                "beta",
                ReleaseType::Beta,
                3,
                vec![GameLoader::Fabric],
                vec!["1.21.4"],
            ),
            summary(
                "new",
                ReleaseType::Release,
                2,
                vec![GameLoader::Fabric],
                vec!["1.21.4"],
            ),
        ];

        let pick = choose_version(candidates, &cluster, GameLoader::Fabric);
        assert_eq!(pick.unwrap().version_id, "new");
    }

    #[test]
    fn falls_back_to_a_prerelease_when_no_release_fits() {
        let cluster = cluster(GameLoader::Fabric, "1.21.4");
        let candidates = vec![
            summary(
                "alpha",
                ReleaseType::Alpha,
                1,
                vec![GameLoader::Fabric],
                vec!["1.21.4"],
            ),
            summary(
                "beta",
                ReleaseType::Beta,
                2,
                vec![GameLoader::Fabric],
                vec!["1.21.4"],
            ),
        ];

        let pick = choose_version(candidates, &cluster, GameLoader::Fabric);
        assert_eq!(pick.unwrap().version_id, "beta");
    }

    #[test]
    fn drops_versions_the_store_would_reject() {
        let cluster = cluster(GameLoader::Fabric, "1.21.4");
        let candidates = vec![
            summary(
                "forge",
                ReleaseType::Release,
                3,
                vec![GameLoader::Forge],
                vec!["1.21.4"],
            ),
            summary(
                "old-mc",
                ReleaseType::Release,
                2,
                vec![GameLoader::Fabric],
                vec!["1.20.1"],
            ),
            summary(
                "fits",
                ReleaseType::Release,
                1,
                vec![GameLoader::Fabric],
                vec!["1.21.4"],
            ),
        ];

        let pick = choose_version(candidates, &cluster, GameLoader::Fabric);
        assert_eq!(pick.unwrap().version_id, "fits");
    }

    #[test]
    fn keeps_loaderless_content_for_a_modded_cluster() {
        let cluster = cluster(GameLoader::Fabric, "1.21.4");
        let candidates = vec![summary(
            "pack",
            ReleaseType::Release,
            1,
            Vec::new(),
            vec!["1.21.4"],
        )];

        let pick = choose_version(candidates, &cluster, GameLoader::Fabric);
        assert_eq!(pick.unwrap().version_id, "pack");
    }

    #[test]
    fn no_compatible_version_resolves_to_nothing() {
        let cluster = cluster(GameLoader::Fabric, "1.21.4");
        let candidates = vec![summary(
            "forge",
            ReleaseType::Release,
            1,
            vec![GameLoader::Forge],
            vec!["1.21.4"],
        )];

        assert!(choose_version(candidates, &cluster, GameLoader::Fabric).is_none());
    }

    #[test]
    fn release_numbers_are_read_past_minecraft_and_loader_tags() {
        for (version, expected) in [
            ("mc1.21.1-0.6.0-fabric", vec![0, 6, 0]),
            ("0.120.0+26.3", vec![0, 120, 0]),
            ("1.8.0+26.3-fabric", vec![1, 8, 0]),
            ("26.3.0.0.9", vec![26, 3, 0, 0, 9]),
            ("v2.1", vec![2, 1]),
        ] {
            assert_eq!(
                release_number(version, "26.3").map(|release| release.core),
                Some(expected),
                "{version}"
            );
        }

        assert_eq!(release_number("beta", "26.3"), None);
    }

    #[test]
    fn versions_are_checked_against_the_wanted_range() {
        let at_least = VersionRange::at_least("0.5.4");
        assert!(satisfies(&at_least, "mc1.21.1-0.6.0-fabric", "1.21.1"));
        assert!(!satisfies(&at_least, "mc1.21.1-0.5.3-fabric", "1.21.1"));

        let terrablender = VersionRange::at_least("26.3.0.0.6");
        assert!(satisfies(&terrablender, "26.3.0.0.9", "26.3"));
        assert!(!satisfies(&terrablender, "26.3.0.0.5", "26.3"));

        let older = VersionRange {
            max: Some("2.0".to_string()),
            ..VersionRange::default()
        };
        assert!(satisfies(&older, "1.9.9", "26.3"));
        assert!(!satisfies(&older, "2.0", "26.3"));

        let exact = VersionRange::exactly("1.2.0");
        assert!(satisfies(&exact, "1.2", "26.3"));
        assert!(!satisfies(&exact, "1.2.1", "26.3"));
    }

    #[test]
    fn an_unreadable_version_never_satisfies_a_range() {
        assert!(!satisfies(&VersionRange::at_least("1.0"), "beta", "26.3"));
    }

    #[test]
    fn a_prerelease_sorts_below_its_release() {
        let at_least = VersionRange::at_least("1.0.0");
        assert!(!satisfies(&at_least, "1.0.0-beta.1", "26.3"));
        assert!(!satisfies(&at_least, "mc26.3-1.0.0-rc.2+fabric", "26.3"));
        assert!(satisfies(&at_least, "1.0.0", "26.3"));
        assert!(satisfies(&at_least, "1.0.0-fabric", "26.3"));
        assert!(satisfies(&at_least, "1.0.1-beta.1", "26.3"));

        let beta = VersionRange::at_least("1.0.0-beta.2");
        assert!(satisfies(&beta, "1.0.0-beta.10", "26.3"));
        assert!(satisfies(&beta, "1.0.0-rc.1", "26.3"));
        assert!(!satisfies(&beta, "1.0.0-beta.1", "26.3"));
        assert!(!satisfies(&beta, "1.0.0-beta", "26.3"));

        assert!(satisfies(
            &VersionRange::at_least("1.0.0-"),
            "1.0.0-alpha",
            "26.3"
        ));
    }

    #[test]
    fn a_declared_version_reads_the_way_fabric_does() {
        let at_least = [VersionRange::at_least("1.0")];

        assert!(semver_rules_out(&at_least, "1.0-fabric"));
        assert!(!semver_rules_out(&at_least, "1.0+beta"));
        assert!(!semver_rules_out(&at_least, "1.0.0+1.21.1"));
        assert!(!semver_rules_out(&at_least, "1.1-fabric"));

        assert!(semver_rules_out(&at_least, "v2.0"));
        assert!(semver_rules_out(&at_least, "not.a.version"));
        assert!(semver_rules_out(&at_least, "2.0-beta_1"));
        assert!(!semver_rules_out(&[VersionRange::default()], "v2.0"));

        let text = [VersionRange::at_least("v2.0")];
        assert!(!semver_rules_out(&text, "v2.0"));
        assert!(semver_rules_out(&text, "2.0"));

        let suffixed = [VersionRange::at_least("1.0-fabric")];
        assert!(!semver_rules_out(&suffixed, "1.0-fabric"));
        assert!(!semver_rules_out(&suffixed, "1.0"));
        assert!(semver_rules_out(&suffixed, "1.0-alpha"));

        let below_two = [VersionRange {
            max: Some("2-".to_string()),
            ..VersionRange::default()
        }];
        assert!(!semver_rules_out(&below_two, "1.9"));
        assert!(semver_rules_out(&below_two, "2.0-alpha"));
    }

    #[test]
    fn a_forge_version_follows_mavens_order() {
        let fits = |spec: &[VersionRange], version| maven_fits(spec, version).unwrap();
        let at_least = |min| [VersionRange::at_least(min)];

        assert!(!fits(&at_least("1.0"), "1.0-m1"));
        assert!(!fits(&at_least("1.0"), "1.0-alpha2"));
        assert!(fits(&at_least("1.0-rc2"), "1.0-rc10"));
        assert!(!fits(&at_least("1.0-rc2"), "1.0-rc1"));
        assert!(fits(&at_least("1.0"), "1.0-neoforge"));
        assert!(fits(&at_least("1.0"), "1.0.0.final"));
        assert!(fits(&at_least("1.0-cr1"), "1.0-rc1"));
        assert!(fits(&at_least("1.20.1-47.2.0"), "1.20.1-47.2.20"));
        assert!(fits(&at_least("1.0-beta"), "1.0-rc1"));
        assert!(!fits(&at_least("1.0"), "1.0-snapshot"));
        assert!(fits(&at_least("1.0"), "1.0-sp1"));
        assert!(!fits(&at_least("2.0"), "v3.0"), "text sorts below numbers");
        assert!(!fits(&at_least("1.0-rc3"), "1.0.RC2"));
        assert!(fits(&at_least("1.0-rc2"), "1.0.RC3"));
        assert!(fits(&[VersionRange::exactly("1.0-rc2")], "1.0.RC2"));
        assert!(fits(&[VersionRange::exactly("1.0-rc")], "1.0.rc"));
        assert!(!fits(&at_least("1.0.0-x2"), "1.0.0.x1"));

        let below_two = [VersionRange {
            max: Some("2.0".to_string()),
            ..VersionRange::default()
        }];
        assert!(fits(&below_two, "2.0-rc1"));
        assert!(!fits(&below_two, "2.0"));
        assert!(fits(&[VersionRange::exactly("1.2")], "1.2.0"));

        assert_eq!(maven_fits(&at_least("1.0"), "${file.jarVersion}"), None);
        assert_eq!(maven_fits(&at_least("1.0"), "@VERSION@"), None);
        assert_eq!(maven_fits(&at_least("1.0"), ""), None);
    }

    #[test]
    fn only_a_readable_version_outside_every_range_is_ruled_out() {
        let ranges = [VersionRange::at_least("2.0"), VersionRange::exactly("1.0")];

        assert!(rules_out(&ranges, "1.5", "26.3"));
        assert!(!rules_out(&ranges, "1.0", "26.3"));
        assert!(!rules_out(&ranges, "2.1", "26.3"));
        assert!(!rules_out(&ranges, "beta", "26.3"));
    }

    #[test]
    fn search_hits_are_compared_without_punctuation_or_case() {
        assert_eq!(squash("entity_texture_features"), "entitytexturefeatures");
        assert_eq!(squash("Entity-Texture Features"), "entitytexturefeatures");
    }
}
