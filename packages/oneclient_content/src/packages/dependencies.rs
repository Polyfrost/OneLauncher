use std::collections::{HashSet, VecDeque};

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_db::models::ClusterRow;
use oneclient_events::VersionRange;

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::provider::PackageProvider;
use crate::packages::store::PackageStore;
use crate::packages::types::{
    DependencyKind, ProjectDetail, ReleaseType, SearchFilters, VersionDependency, VersionDetail,
    VersionSummary,
};

/// Caps the transitive walk so a provider serving a cycle can't spin forever
const MAX_DEPTH: usize = 4;

const VERSION_WINDOW: usize = 50;

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
}

impl DependencyResolution {
    pub fn is_empty(&self) -> bool {
        self.install.is_empty() && self.unresolved.is_empty()
    }
}

#[tracing::instrument(level = "debug", skip(root, ctx), fields(project_id = %root.project_id, version_id = %root.version_id))]
pub async fn resolve_required(
    provider_id: ProviderId,
    root: &VersionDetail,
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<DependencyResolution> {
    let mut resolution = DependencyResolution::default();
    if !root.dependencies.iter().any(is_required) {
        return Ok(resolution);
    }

    let provider = ctx.providers.get(provider_id)?;
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;

    // Seeded with installed projects and the root against refetches and self-referencing graphs
    let mut seen = installed_project_ids(provider_id, cluster_id, ctx).await?;
    seen.insert(root.project_id.clone());

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
            continue;
        }

        let resolved = match resolve_one(provider, &dep, &cluster, ctx).await {
            Ok(Some(resolved)) => resolved,
            Ok(None) => {
                tracing::warn!(?dep, "no compatible version for dependency");
                resolution.unresolved.push(dependency_label(&dep));
                continue;
            }
            Err(err) => {
                tracing::warn!(?dep, %err, "failed to resolve dependency");
                resolution.unresolved.push(dependency_label(&dep));
                continue;
            }
        };

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

fn release_number(version: &str, mc_version: &str) -> Option<Vec<u64>> {
    version
        .split(['+', '-', '_', ' '])
        .filter(|token| !token.is_empty())
        .filter(|token| !token.to_ascii_lowercase().starts_with("mc"))
        .filter(|token| *token != mc_version)
        .find_map(numeric_parts)
}

fn compare_parts(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    let len = a.len().max(b.len());
    (0..len)
        .map(|index| a.get(index).unwrap_or(&0).cmp(b.get(index).unwrap_or(&0)))
        .find(|ordering| ordering.is_ne())
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn satisfies(range: &VersionRange, version: &str, mc_version: &str) -> bool {
    let Some(found) = release_number(version, mc_version) else {
        return false;
    };

    let bound =
        |bound: &Option<String>| bound.as_deref().and_then(|bound| release_number(bound, ""));

    if let Some(min) = bound(&range.min) {
        let ordering = compare_parts(&found, &min);
        if ordering.is_lt() || (ordering.is_eq() && !range.min_inclusive) {
            return false;
        }
    }

    if let Some(max) = bound(&range.max) {
        let ordering = compare_parts(&found, &max);
        if ordering.is_gt() || (ordering.is_eq() && !range.max_inclusive) {
            return false;
        }
    }

    true
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

async fn installed_project_ids(
    provider_id: ProviderId,
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<HashSet<String>> {
    Ok(PackageStore::list_linked_artifacts(cluster_id, ctx)
        .await?
        .into_iter()
        .filter(|linked| linked.provider == Some(provider_id))
        .filter_map(|linked| linked.project_id)
        .collect())
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
            assert_eq!(release_number(version, "26.3"), Some(expected), "{version}");
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
    fn search_hits_are_compared_without_punctuation_or_case() {
        assert_eq!(squash("entity_texture_features"), "entitytexturefeatures");
        assert_eq!(squash("Entity-Texture Features"), "entitytexturefeatures");
    }
}
