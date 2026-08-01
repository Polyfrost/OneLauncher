use std::collections::{HashSet, VecDeque};

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_db::models::ClusterRow;

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::provider::PackageProvider;
use crate::packages::store::PackageStore;
use crate::packages::types::{
	DependencyKind, ProjectDetail, ReleaseType, VersionDependency, VersionDetail, VersionSummary,
};

/// How far the transitive walk goes. Real graphs are two or three deep; the cap
/// is only here so a provider serving a cycle can't spin forever.
const MAX_DEPTH: usize = 4;

/// How many of a project's versions to consider when picking one. The providers
/// already filter by game version and loader, so the answer is near the top.
const VERSION_WINDOW: usize = 50;

#[derive(Debug, Clone)]
pub struct ResolvedDependency {
	pub project: ProjectDetail,
	pub version: VersionDetail,
}

#[derive(Debug, Clone, Default)]
pub struct DependencyResolution {
	/// Dependencies that need installing, ordered breadth-first from the root.
	/// Already-present ones are filtered out.
	pub install: Vec<ResolvedDependency>,
	/// Required dependencies that couldn't be resolved, by whatever id the
	/// provider gave for them. The caller decides how loud to be about these.
	pub unresolved: Vec<String>,
}

impl DependencyResolution {
	pub fn is_empty(&self) -> bool {
		self.install.is_empty() && self.unresolved.is_empty()
	}
}

/// Collects the required dependencies of `root` that `cluster_id` doesn't
/// already have, transitively.
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

	// Seeded with what the cluster already has so an installed library is never
	// fetched twice, plus the root itself against self-referencing graphs.
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

		// A version-pinned dependency only reveals its project once fetched, so
		// the second dedupe check has to happen here rather than up front.
		if !seen.insert(resolved.version.project_id.clone()) {
			continue;
		}

		if depth < MAX_DEPTH {
			for next in resolved.version.dependencies.iter().filter(|d| is_required(d)) {
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

async fn resolve_one(
	provider: &dyn PackageProvider,
	dep: &VersionDependency,
	cluster: &ClusterRow,
	ctx: &ContentCtx,
) -> ContentResult<Option<ResolvedDependency>> {
	// A pinned version wins outright: the author named that exact file, so it is
	// used even when a newer one exists. Both providers ignore the project id
	// argument when the version id is known.
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
			provider.get_version(project_id, &pick.version_id, ctx).await?
		}
	};

	let project = provider.get_project(&version.project_id, ctx).await?;
	Ok(Some(ResolvedDependency { project, version }))
}

/// Picks the newest release of `project_id` that fits the cluster, falling back
/// to the newest prerelease when that's all there is.
///
/// Shared with the browser update check, which asks the same question of an
/// already-installed package: "what would we install for this cluster today?".
pub(crate) async fn pick_version(
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

	// Resource packs, shaders and datapacks declare no loader, so a
	// loader-filtered query comes back empty even though they fit the cluster.
	if candidates.is_empty() && loader_filter.is_some() {
		candidates = provider
			.list_versions(project_id, Some(&cluster.mc_version), None, 0, VERSION_WINDOW, ctx)
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
	// Neither provider promises an order, so sort rather than trust the page.
	candidates.sort_by_key(|version| std::cmp::Reverse(version.published));

	candidates
		.iter()
		.find(|version| matches!(version.release_type, ReleaseType::Release))
		.or_else(|| candidates.first())
		.cloned()
}

/// Mirrors the compatibility check the store applies at install time, so a
/// version that would be rejected there is never picked here.
fn fits_cluster(version: &VersionSummary, cluster: &ClusterRow, loader: GameLoader) -> bool {
	if !version.game_versions.is_empty()
		&& !version
			.game_versions
			.iter()
			.any(|v| cluster.mc_version.contains(v))
	{
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

/// Dependencies only make sense for content the loader reads out of the cluster;
/// a modpack brings its own file list.
pub fn resolves_dependencies(content_type: ContentType) -> bool {
	!matches!(content_type, ContentType::Modpack)
}

#[cfg(test)]
mod tests {
	use chrono::{TimeZone, Utc};

	use super::*;

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
		}
	}

	#[test]
	fn prefers_the_newest_release_over_a_newer_prerelease() {
		let cluster = cluster(GameLoader::Fabric, "1.21.4");
		let candidates = vec![
			summary("old", ReleaseType::Release, 1, vec![GameLoader::Fabric], vec!["1.21.4"]),
			summary("beta", ReleaseType::Beta, 3, vec![GameLoader::Fabric], vec!["1.21.4"]),
			summary("new", ReleaseType::Release, 2, vec![GameLoader::Fabric], vec!["1.21.4"]),
		];

		let pick = choose_version(candidates, &cluster, GameLoader::Fabric);
		assert_eq!(pick.unwrap().version_id, "new");
	}

	#[test]
	fn falls_back_to_a_prerelease_when_no_release_fits() {
		let cluster = cluster(GameLoader::Fabric, "1.21.4");
		let candidates = vec![
			summary("alpha", ReleaseType::Alpha, 1, vec![GameLoader::Fabric], vec!["1.21.4"]),
			summary("beta", ReleaseType::Beta, 2, vec![GameLoader::Fabric], vec!["1.21.4"]),
		];

		let pick = choose_version(candidates, &cluster, GameLoader::Fabric);
		assert_eq!(pick.unwrap().version_id, "beta");
	}

	#[test]
	fn drops_versions_the_store_would_reject() {
		let cluster = cluster(GameLoader::Fabric, "1.21.4");
		let candidates = vec![
			summary("forge", ReleaseType::Release, 3, vec![GameLoader::Forge], vec!["1.21.4"]),
			summary("old-mc", ReleaseType::Release, 2, vec![GameLoader::Fabric], vec!["1.20.1"]),
			summary("fits", ReleaseType::Release, 1, vec![GameLoader::Fabric], vec!["1.21.4"]),
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
}
