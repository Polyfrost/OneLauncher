use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use tokio::sync::OnceCell;

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::{ClusterRow, SeenStatus};
use oneclient_events::GroupedProgressChild;

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::dependencies::{ResolvedDependency, resolve_one, resolves_dependencies, supports_game_version};
use crate::packages::store::PackageStore;
use crate::packages::types::{
	DependencyKind, InstalledPackage, ProjectDetail, VersionDependency, VersionDetail,
};
use crate::packages::updates::{Candidate, browser_installed};

pub const MIGRATED_CONTENT_TYPES: [ContentType; 3] =
	[ContentType::Mod, ContentType::ResourcePack, ContentType::Shader];

const DEPENDENCY_DEPTH: usize = 4;

const SOURCE_CONCURRENCY: usize = 3;
const DEPENDENCY_CONCURRENCY: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseMigrationPackage {
	pub source_hash: String,
	pub provider: ProviderId,
	pub project_id: String,
	pub content_type: ContentType,
	pub display_name: String,
	pub version_id: String,
	pub version_name: String,
	pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkipReason {
	NoCompatibleVersion,
	ProviderUnavailable,
	MissingDependency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseMigrationSkip {
	pub provider: ProviderId,
	pub project_id: String,
	pub content_type: ContentType,
	pub display_name: String,
	pub reason: SkipReason,
}

#[derive(Debug, Clone)]
pub struct ReleaseMigrationDependency {
	pub provider: ProviderId,
	pub project: ProjectDetail,
	pub version: VersionDetail,
	pub required_by: Vec<String>,
}

impl ReleaseMigrationDependency {
	#[must_use]
	pub fn version_name(&self) -> String {
		version_label(&self.version)
	}

	#[must_use]
	pub fn size(&self) -> u64 {
		self.version.primary_file().map_or(0, |file| file.size)
	}

	#[must_use]
	pub fn is_needed_by(&self, selected_hashes: &HashSet<String>) -> bool {
		self.required_by.iter().any(|hash| selected_hashes.contains(hash))
	}
}

#[derive(Debug, Clone, Default)]
pub struct ReleaseMigrationPlan {
	pub source_cluster_id: i64,
	pub target_cluster_id: i64,
	pub packages: Vec<ReleaseMigrationPackage>,
	pub dependencies: Vec<ReleaseMigrationDependency>,
	pub unavailable: Vec<ReleaseMigrationSkip>,
	pub unreachable: bool,
}

impl ReleaseMigrationPlan {
	#[must_use]
	pub fn offered(&self, content_type: ContentType) -> usize {
		self.packages
			.iter()
			.filter(|package| package.content_type == content_type)
			.count()
	}
}

fn version_label(version: &VersionDetail) -> String {
	if version.version_number.is_empty() {
		version.name.clone()
	} else {
		version.version_number.clone()
	}
}

async fn migratable_candidates(cluster_id: i64, ctx: &ContentCtx) -> ContentResult<Vec<Candidate>> {
	let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;
	let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
	let bundle_projects: HashSet<String> =
		tracked.iter().filter_map(|row| row.package_id.clone()).collect();
	let bundle_hashes: HashSet<String> = tracked.into_iter().map(|row| row.hash).collect();

	Ok(browser_installed(&linked, &bundle_hashes, &bundle_projects)
		.into_iter()
		.filter(|candidate| MIGRATED_CONTENT_TYPES.contains(&candidate.content_type))
		.collect())
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn has_migratable_packages(cluster_id: i64, ctx: &ContentCtx) -> ContentResult<bool> {
	Ok(!migratable_candidates(cluster_id, ctx).await?.is_empty())
}

async fn target_projects(target_cluster_id: i64, ctx: &ContentCtx) -> ContentResult<HashSet<String>> {
	let mut projects: HashSet<String> = PackageStore::list_linked_artifacts(target_cluster_id, ctx)
		.await?
		.into_iter()
		.filter_map(|linked| linked.project_id)
		.collect();
	projects.extend(
		bundle_dao::list_bundle_tracked(&ctx.db, target_cluster_id)
			.await?
			.into_iter()
			.filter_map(|row| row.package_id),
	);
	Ok(projects)
}

fn fits_target(version: &VersionDetail, content_type: ContentType, mc_version: &str, loader: GameLoader) -> bool {
	if version.primary_file().is_none() || !supports_game_version(&version.game_versions, mc_version) {
		return false;
	}
	content_type != ContentType::Mod
		|| !loader.is_modded()
		|| version.loaders.is_empty()
		|| version.loaders.iter().any(|other| loader.compatible_with(*other))
}

type Lookup = Option<ResolvedDependency>;

struct Unreachable;

#[derive(Default)]
pub struct DependencyCache {
	entries: Mutex<HashMap<(ProviderId, String), Arc<OnceCell<Lookup>>>>,
}

fn dependency_key(dep: &VersionDependency) -> Option<String> {
	dep.version_id
		.as_ref()
		.map(|version_id| format!("version:{version_id}"))
		.or_else(|| dep.project_id.as_ref().map(|project_id| format!("project:{project_id}")))
}

impl DependencyCache {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	fn cell(&self, provider_id: ProviderId, key: String) -> Arc<OnceCell<Lookup>> {
		Arc::clone(
			self.entries
				.lock()
				.unwrap()
				.entry((provider_id, key))
				.or_default(),
		)
	}

	async fn lookup(
		&self,
		provider_id: ProviderId,
		dep: &VersionDependency,
		target: &ClusterRow,
		ctx: &ContentCtx,
	) -> Result<Lookup, Unreachable> {
		let Some(key) = dependency_key(dep) else {
			return Ok(None);
		};

		let cell = self.cell(provider_id, key);
		cell.get_or_try_init(|| async {
			let provider = match ctx.providers.get(provider_id) {
				Ok(provider) => provider,
				Err(_) => return Ok(None),
			};
			match resolve_one(provider, dep, target, ctx).await {
				Ok(resolved) => Ok(resolved),
				Err(err) if err.is_transient() => Err(Unreachable),
				Err(err) => {
					tracing::warn!(?dep, %err, "could not resolve a dependency for release migration");
					Ok(None)
				}
			}
		})
		.await
		.cloned()
	}

	async fn resolve(
		&self,
		provider_id: ProviderId,
		root: &VersionDetail,
		target: &ClusterRow,
		present: &HashSet<String>,
		ctx: &ContentCtx,
	) -> Result<Option<Vec<ResolvedDependency>>, Unreachable> {
		let mut seen: HashSet<String> = HashSet::new();
		seen.insert(root.project_id.clone());

		let mut queue: VecDeque<(VersionDependency, usize)> = root
			.dependencies
			.iter()
			.filter(|dep| dep.kind == DependencyKind::Required)
			.map(|dep| (dep.clone(), 1))
			.collect();

		let mut found = Vec::new();
		while let Some((dep, depth)) = queue.pop_front() {
			if let Some(project_id) = &dep.project_id
				&& (seen.contains(project_id) || present.contains(project_id))
			{
				continue;
			}

			let Some(resolved) = self.lookup(provider_id, &dep, target, ctx).await? else {
				return Ok(None);
			};

			let project_id = resolved.version.project_id.clone();
			if present.contains(&project_id) || !seen.insert(project_id) {
				continue;
			}

			if depth < DEPENDENCY_DEPTH {
				for next in resolved
					.version
					.dependencies
					.iter()
					.filter(|next| next.kind == DependencyKind::Required)
				{
					queue.push_back((next.clone(), depth + 1));
				}
			}

			found.push(resolved);
		}

		Ok(Some(found))
	}
}

#[tracing::instrument(level = "debug", skip(source_cluster_ids, ctx))]
pub async fn plan_release_migrations(
	source_cluster_ids: &[i64],
	target_cluster_id: i64,
	ctx: &ContentCtx,
) -> Vec<(i64, ContentResult<ReleaseMigrationPlan>)> {
	let cache = DependencyCache::new();

	futures_util::stream::iter(source_cluster_ids.iter().copied().map(|source_cluster_id| {
		let cache = &cache;
		async move {
			let plan =
				plan_release_migration_cached(source_cluster_id, target_cluster_id, cache, ctx).await;
			(source_cluster_id, plan)
		}
	}))
	.buffer_unordered(SOURCE_CONCURRENCY)
	.collect::<Vec<_>>()
	.await
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn plan_release_migration(
	source_cluster_id: i64,
	target_cluster_id: i64,
	ctx: &ContentCtx,
) -> ContentResult<ReleaseMigrationPlan> {
	plan_release_migration_cached(
		source_cluster_id,
		target_cluster_id,
		&DependencyCache::new(),
		ctx,
	)
	.await
}

#[tracing::instrument(level = "debug", skip(cache, ctx))]
async fn plan_release_migration_cached(
	source_cluster_id: i64,
	target_cluster_id: i64,
	cache: &DependencyCache,
	ctx: &ContentCtx,
) -> ContentResult<ReleaseMigrationPlan> {
	let target = PackageStore::get_cluster(target_cluster_id, ctx).await?;
	let loader = GameLoader::from_repr(target.mc_loader as u8).unwrap_or(GameLoader::Vanilla);
	let present = target_projects(target_cluster_id, ctx).await?;

	let candidates: Vec<Candidate> = migratable_candidates(source_cluster_id, ctx)
		.await?
		.into_iter()
		.filter(|candidate| !present.contains(&candidate.project_id))
		.collect();

	let mut plan = ReleaseMigrationPlan {
		source_cluster_id,
		target_cluster_id,
		..Default::default()
	};

	let mut by_provider: HashMap<ProviderId, Vec<Candidate>> = HashMap::new();
	for candidate in candidates {
		by_provider.entry(candidate.provider).or_default().push(candidate);
	}

	let mut offered: Vec<(ReleaseMigrationPackage, VersionDetail)> = Vec::new();

	for (provider_id, candidates) in by_provider {
		let installed: Vec<InstalledPackage> = candidates
			.iter()
			.map(|candidate| InstalledPackage {
				hash: candidate.hash.clone(),
				project_id: candidate.project_id.clone(),
				content_type: candidate.content_type,
			})
			.collect();

		let found = match ctx.providers.get(provider_id) {
			Ok(provider) => {
				provider
					.latest_for_game_version(&installed, &target.mc_version, loader, ctx)
					.await
			}
			Err(err) => Err(err),
		};

		let found = match found {
			Ok(found) => found,
			Err(err) if err.is_transient() => {
				tracing::warn!(provider = ?provider_id, error = %err, "release migration check could not reach the provider");
				plan.unreachable = true;
				continue;
			}
			Err(err) => {
				tracing::warn!(provider = ?provider_id, error = %err, "release migration check failed for the provider");
				plan.unavailable.extend(candidates.into_iter().map(|candidate| ReleaseMigrationSkip {
					provider: provider_id,
					project_id: candidate.project_id,
					content_type: candidate.content_type,
					display_name: candidate.display_name,
					reason: SkipReason::ProviderUnavailable,
				}));
				continue;
			}
		};

		for candidate in candidates {
			let version = found
				.get(&candidate.hash)
				.filter(|version| fits_target(version, candidate.content_type, &target.mc_version, loader));

			match version {
				Some(version) => offered.push((
					ReleaseMigrationPackage {
						source_hash: candidate.hash,
						provider: provider_id,
						project_id: candidate.project_id,
						content_type: candidate.content_type,
						display_name: candidate.display_name,
						version_id: version.version_id.clone(),
						version_name: version_label(version),
						size: version.primary_file().map_or(0, |file| file.size),
					},
					version.clone(),
				)),
				None => plan.unavailable.push(ReleaseMigrationSkip {
					provider: provider_id,
					project_id: candidate.project_id,
					content_type: candidate.content_type,
					display_name: candidate.display_name,
					reason: SkipReason::NoCompatibleVersion,
				}),
			}
		}
	}

	if plan.unreachable {
		return Ok(plan);
	}

	let resolved_packages = futures_util::stream::iter(offered.into_iter().map(|(package, version)| {
		let target = &target;
		let present = &present;
		async move {
			let resolved = if resolves_dependencies(package.content_type) {
				cache
					.resolve(package.provider, &version, target, present, ctx)
					.await
			} else {
				Ok(Some(Vec::new()))
			};
			(package, resolved)
		}
	}))
	.buffer_unordered(DEPENDENCY_CONCURRENCY)
	.collect::<Vec<_>>()
	.await;

	let mut dependencies: Vec<ReleaseMigrationDependency> = Vec::new();
	for (package, resolved) in resolved_packages {
		let resolved = match resolved {
			Ok(resolved) => resolved,
			Err(Unreachable) => {
				plan.unreachable = true;
				return Ok(plan);
			}
		};

		let Some(resolved) = resolved else {
			plan.unavailable.push(ReleaseMigrationSkip {
				provider: package.provider,
				project_id: package.project_id,
				content_type: package.content_type,
				display_name: package.display_name,
				reason: SkipReason::MissingDependency,
			});
			continue;
		};

		for dependency in resolved {
			match dependencies
				.iter_mut()
				.find(|existing| existing.provider == package.provider && existing.project.id == dependency.project.id)
			{
				Some(existing) => existing.required_by.push(package.source_hash.clone()),
				None => dependencies.push(ReleaseMigrationDependency {
					provider: package.provider,
					project: dependency.project,
					version: dependency.version,
					required_by: vec![package.source_hash.clone()],
				}),
			}
		}

		plan.packages.push(package);
	}

	let libraries: HashSet<String> = plan
		.packages
		.iter()
		.filter(|package| {
			dependencies
				.iter()
				.any(|dependency| dependency.provider == package.provider && dependency.project.id == package.project_id)
		})
		.map(|package| package.source_hash.clone())
		.collect();

	plan.packages
		.retain(|package| !libraries.contains(&package.source_hash));
	for dependency in &mut dependencies {
		dependency.required_by.retain(|hash| !libraries.contains(hash));
	}
	dependencies.retain(|dependency| !dependency.required_by.is_empty());

	plan.dependencies = dependencies;

	plan.packages
		.sort_by_key(|package| package.display_name.to_lowercase());
	plan.dependencies
		.sort_by_key(|dependency| dependency.project.name.to_lowercase());
	plan.unavailable
		.sort_by_key(|skip| skip.display_name.to_lowercase());

	Ok(plan)
}

#[tracing::instrument(level = "debug", skip(dependency, child, ctx), fields(project_id = %dependency.project.id))]
pub async fn apply_release_migration_dependency(
	target_cluster_id: i64,
	dependency: &ReleaseMigrationDependency,
	child: Option<&GroupedProgressChild>,
	ctx: &ContentCtx,
) -> ContentResult<String> {
	let (installed, _) = PackageStore::install_to_cluster(
		dependency.provider,
		&dependency.project,
		&dependency.version,
		target_cluster_id,
		true,
		false,
		child,
		ctx,
	)
	.await?;

	artifact_dao::set_seen_status(&ctx.db, target_cluster_id, &installed.hash, SeenStatus::New)
		.await?;

	Ok(installed.hash)
}

#[tracing::instrument(level = "debug", skip(package, child, ctx), fields(project_id = %package.project_id))]
pub async fn apply_release_migration_package(
	target_cluster_id: i64,
	package: &ReleaseMigrationPackage,
	child: Option<&GroupedProgressChild>,
	ctx: &ContentCtx,
) -> ContentResult<String> {
	let provider = ctx.providers.get(package.provider)?;
	let project = provider.get_project(&package.project_id, ctx).await?;
	let version = provider
		.get_version(&package.project_id, &package.version_id, ctx)
		.await?;

	let (installed, _) = PackageStore::install_to_cluster(
		package.provider,
		&project,
		&version,
		target_cluster_id,
		true,
		false,
		child,
		ctx,
	)
	.await?;

	artifact_dao::set_seen_status(&ctx.db, target_cluster_id, &installed.hash, SeenStatus::New)
		.await?;

	Ok(installed.hash)
}

#[cfg(test)]
mod tests {
	use chrono::Utc;

	use super::*;
	use crate::packages::types::VersionFile;

	fn version(game_versions: &[&str], loaders: Vec<GameLoader>) -> VersionDetail {
		VersionDetail {
			version_id: "v".into(),
			project_id: "p".into(),
			name: "name".into(),
			version_number: "1.0".into(),
			changelog: None,
			game_versions: game_versions.iter().map(|v| (*v).to_string()).collect(),
			loaders,
			published: Utc::now(),
			downloads: 0,
			files: vec![VersionFile {
				sha1: "a".into(),
				url: "u".into(),
				file_name: "f.jar".into(),
				primary: true,
				size: 1,
				fingerprint: None,
			}],
			dependencies: Vec::new(),
		}
	}

	#[test]
	fn a_mod_for_another_loader_is_not_offered() {
		let forge = version(&["26.3"], vec![GameLoader::Forge]);
		assert!(!fits_target(&forge, ContentType::Mod, "26.3", GameLoader::Fabric));
	}

	#[test]
	fn a_mod_for_an_older_game_version_is_not_offered() {
		let old = version(&["26.2"], vec![GameLoader::Fabric]);
		assert!(!fits_target(&old, ContentType::Mod, "26.3", GameLoader::Fabric));
	}

	#[test]
	fn resource_packs_ignore_the_cluster_loader() {
		let pack = version(&["26.3"], vec![GameLoader::Vanilla]);
		assert!(fits_target(&pack, ContentType::ResourcePack, "26.3", GameLoader::Fabric));
	}

	#[test]
	fn a_version_without_files_is_not_offered() {
		let mut empty = version(&["26.3"], vec![GameLoader::Fabric]);
		empty.files.clear();
		assert!(!fits_target(&empty, ContentType::Mod, "26.3", GameLoader::Fabric));
	}

	#[test]
	fn packages_keep_a_stable_order_however_the_checks_finish() {
		let mut packages = vec!["Zoomify", "blur", "Sodium"];
		packages.sort_by_key(|name| name.to_lowercase());
		assert_eq!(packages, vec!["blur", "Sodium", "Zoomify"]);
	}

	#[test]
	fn pinned_versions_and_projects_cache_separately() {
		let pinned = VersionDependency {
			project_id: Some("fabric-api".into()),
			version_id: Some("v1".into()),
			kind: DependencyKind::Required,
		};
		let loose = VersionDependency {
			project_id: Some("fabric-api".into()),
			version_id: None,
			kind: DependencyKind::Required,
		};
		assert_eq!(dependency_key(&pinned).as_deref(), Some("version:v1"));
		assert_eq!(dependency_key(&loose).as_deref(), Some("project:fabric-api"));
	}
}
