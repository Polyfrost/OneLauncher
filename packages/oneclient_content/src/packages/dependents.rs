//! Who in a cluster depends on a package.
//!
//! [`crate::packages::dependencies`] answers the forward question at install
//! time — "what does this need?" — by asking the provider. This module answers
//! the reverse one, "what needs this?", and it has to answer it for content
//! that is already installed, while the user waits on a toggle, possibly
//! offline. That rules the provider out, so the dependency lists the installer
//! already receives are persisted with the release (see
//! [`crate::packages::store::store_release_dependencies`]) and read back from
//! `provider_release_dependencies`.
//!
//! Only [`DependencyKind::Required`] propagates. A mod that lists another as
//! optional runs without it — switching it off unasked would be the launcher
//! deciding something the user did not — so those are reported and left alone.
//! `Incompatible` is the opposite relation entirely, and `Embedded` means the
//! dependency is inside the jar, so neither says anything about what has to go
//! down with the target.

use std::collections::{HashMap, HashSet, VecDeque};

use futures_util::StreamExt;

use oneclient_common::domain::ProviderId;
use oneclient_db::dao::artifact as artifact_dao;

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::metadata_cache::read_cached_package_meta;
use crate::packages::store::{PackageStore, store_release_dependencies};
use crate::packages::types::{DependencyKind, LinkedArtifactInfo};

/// How many releases the backfill asks about at once. Matches the update
/// check's fan-out; the providers are the bottleneck, not us.
const BACKFILL_CONCURRENCY: usize = 6;

/// The two ids a dependency can be written against, paired with the provider
/// that issued them. Modrinth pins a project, a version or both, and the two id
/// spaces are disjoint, so a package answers to every key it has.
type DependencyKey = (ProviderId, String);

/// One installed package that depends on the one being disabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependentPackage {
	pub hash: String,
	pub name: String,
}

/// What disabling a package would take with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisableImpact {
	/// Installed packages that require the target, transitively. Disabling the
	/// target disables all of these with it.
	pub required: Vec<DependentPackage>,
	/// Installed packages that name the target as an optional dependency. They
	/// are listed so the user knows, and then left enabled.
	pub optional: Vec<DependentPackage>,
}

impl DisableImpact {
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.required.is_empty() && self.optional.is_empty()
	}

	/// The artifacts to switch off alongside the target.
	#[must_use]
	pub fn required_hashes(&self) -> Vec<String> {
		self.required.iter().map(|dep| dep.hash.clone()).collect()
	}
}

/// The package a disable starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisableRoot {
	/// An artifact the cluster has installed, by hash.
	Artifact(String),
	/// A package a bundle provides that the cluster has not installed yet, so
	/// there is no artifact to key on — only the provider's project id.
	Package(ProviderId, String),
}

/// Everything the cluster has that would have to go down with `root`.
///
/// Reads only what is stored. Call [`ensure_cluster_dependencies`] first if the
/// cluster may hold packages installed before their dependency lists were kept.
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn disable_impact(
	cluster_id: i64,
	root: &DisableRoot,
	ctx: &ContentCtx,
) -> ContentResult<DisableImpact> {
	let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;
	let edges = artifact_dao::list_cluster_dependency_edges(&ctx.db, cluster_id).await?;

	let by_hash: HashMap<&str, &LinkedArtifactInfo> = linked
		.iter()
		.map(|info| (info.hash.as_str(), info))
		.collect();

	let mut keys_by_hash: HashMap<String, Vec<DependencyKey>> = HashMap::new();
	for info in &linked {
		keys_by_hash.insert(info.hash.clone(), dependency_keys(info));
	}

	// Only enabled packages can be taken down by this, and an edge whose
	// dependent is already off says nothing the user needs to hear.
	let mut dependents: HashMap<DependencyKey, Vec<(String, DependencyKind)>> = HashMap::new();
	for edge in edges {
		let Some(info) = by_hash.get(edge.hash.as_str()) else {
			continue;
		};
		if !info.enabled {
			continue;
		}
		let Some(provider) = ProviderId::from_repr(edge.provider as u8) else {
			continue;
		};
		let Some(kind) = DependencyKind::parse(&edge.kind) else {
			continue;
		};

		for id in [edge.dependency_project_id, edge.dependency_version_id] {
			if id.is_empty() {
				continue;
			}
			dependents
				.entry((provider, id))
				.or_default()
				.push((edge.hash.clone(), kind));
		}
	}

	let (roots, root_hashes) = root_keys(root, &linked, &keys_by_hash);
	let closure = walk(&roots, &root_hashes, &keys_by_hash, &dependents);

	Ok(DisableImpact {
		required: name_all(&closure.required, &by_hash, ctx).await,
		optional: name_all(&closure.optional, &by_hash, ctx).await,
	})
}

/// Fetches and stores the dependency lists of any installed release that has
/// none recorded.
///
/// Everything installed before the launcher kept them lands here. Failures are
/// logged and skipped rather than raised: an unreachable provider means the
/// impact is computed from what is known, which is the same position the
/// launcher is in offline.
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn ensure_cluster_dependencies(cluster_id: i64, ctx: &ContentCtx) -> ContentResult<()> {
	let missing = artifact_dao::list_releases_missing_dependencies(&ctx.db, cluster_id).await?;
	if missing.is_empty() {
		return Ok(());
	}

	tracing::debug!(count = missing.len(), "backfilling package dependency lists");

	futures_util::stream::iter(missing)
		.map(|release| async move {
			let Some(provider) = ProviderId::from_repr(release.provider as u8) else {
				return;
			};
			if provider == ProviderId::Local {
				return;
			}

			// Straight to the provider, not through the version cache: the
			// cache is filled from the very rows this is here to write, so
			// asking it would store "no dependencies" and mark the release
			// done.
			let version = match fetch_version(provider, &release.project_id, &release.version_id, ctx)
				.await
			{
				Ok(version) => version,
				Err(err) => {
					tracing::debug!(
						project_id = %release.project_id,
						version_id = %release.version_id,
						%err,
						"could not read a version's dependency list"
					);
					return;
				}
			};

			if let Err(err) =
				store_release_dependencies(provider, &release.project_id, &version, ctx).await
			{
				tracing::warn!(
					project_id = %release.project_id,
					%err,
					"failed to store a version's dependency list"
				);
			}
		})
		.buffer_unordered(BACKFILL_CONCURRENCY)
		.collect::<Vec<()>>()
		.await;

	Ok(())
}

async fn fetch_version(
	provider: ProviderId,
	project_id: &str,
	version_id: &str,
	ctx: &ContentCtx,
) -> ContentResult<crate::packages::types::VersionDetail> {
	ctx.providers
		.get(provider)?
		.get_version(project_id, version_id, ctx)
		.await
}

/// Every id an installed package answers to.
fn dependency_keys(info: &LinkedArtifactInfo) -> Vec<DependencyKey> {
	let Some(provider) = info.provider else {
		return Vec::new();
	};

	[info.project_id.clone(), info.version_id.clone()]
		.into_iter()
		.flatten()
		.filter(|id| !id.is_empty())
		.map(|id| (provider, id))
		.collect()
}

/// The keys the walk starts from, and the artifacts that are the target itself
/// and so must never be reported as depending on it.
fn root_keys(
	root: &DisableRoot,
	linked: &[LinkedArtifactInfo],
	keys_by_hash: &HashMap<String, Vec<DependencyKey>>,
) -> (Vec<DependencyKey>, HashSet<String>) {
	match root {
		DisableRoot::Artifact(hash) => (
			keys_by_hash.get(hash).cloned().unwrap_or_default(),
			HashSet::from([hash.clone()]),
		),
		DisableRoot::Package(provider, project_id) => {
			// A cluster can hold more than one copy of a project; every one of
			// them is the target, not a dependent of it.
			let own = linked
				.iter()
				.filter(|info| info.provider == Some(*provider))
				.filter(|info| info.project_id.as_deref() == Some(project_id.as_str()))
				.map(|info| info.hash.clone())
				.collect();

			(vec![(*provider, project_id.clone())], own)
		}
	}
}

/// The transitive closure of the reverse graph, in discovery order.
struct Closure {
	required: Vec<String>,
	optional: Vec<String>,
}

/// Breadth-first over the reverse edges, following required ones only.
///
/// Both sides of the walk are visit-guarded — a package is expanded once, a key
/// is queued once — so a graph with a cycle in it terminates rather than
/// looping. Providers do publish those: two mods that each declare the other,
/// or a chain that closes on itself a few hops out.
fn walk(
	roots: &[DependencyKey],
	root_hashes: &HashSet<String>,
	keys_by_hash: &HashMap<String, Vec<DependencyKey>>,
	dependents: &HashMap<DependencyKey, Vec<(String, DependencyKind)>>,
) -> Closure {
	let mut visited: HashSet<String> = root_hashes.clone();
	let mut queued: HashSet<DependencyKey> = roots.iter().cloned().collect();
	let mut queue: VecDeque<DependencyKey> = roots.iter().cloned().collect();

	let mut required = Vec::new();
	let mut optional = Vec::new();
	let mut optional_seen: HashSet<String> = HashSet::new();

	while let Some(key) = queue.pop_front() {
		for (hash, kind) in dependents.get(&key).into_iter().flatten() {
			match kind {
				DependencyKind::Required => {
					if !visited.insert(hash.clone()) {
						continue;
					}
					required.push(hash.clone());

					// Whatever requires this one now goes too.
					for next in keys_by_hash.get(hash).into_iter().flatten() {
						if queued.insert(next.clone()) {
							queue.push_back(next.clone());
						}
					}
				}
				// Listed, never followed: a package that only optionally needs
				// the target keeps running, so nothing behind it is affected.
				DependencyKind::Optional => {
					if optional_seen.insert(hash.clone()) {
						optional.push(hash.clone());
					}
				}
				DependencyKind::Incompatible | DependencyKind::Embedded => {}
			}
		}
	}

	// A package reachable both ways is being disabled, so it belongs in one
	// list only — the one that says what happens to it.
	optional.retain(|hash| !visited.contains(hash));

	Closure { required, optional }
}

/// Turns hashes into something worth showing in a modal.
///
/// The metadata cache holds the project name the package manager displays; the
/// release's own display name is the version ("Sodium 0.6.0"), which is why it
/// is only the fallback.
async fn name_all(
	hashes: &[String],
	by_hash: &HashMap<&str, &LinkedArtifactInfo>,
	ctx: &ContentCtx,
) -> Vec<DependentPackage> {
	let mut ids_by_provider: HashMap<ProviderId, Vec<String>> = HashMap::new();
	for info in hashes.iter().filter_map(|hash| by_hash.get(hash.as_str())) {
		if let (Some(provider), Some(project_id)) = (info.provider, info.project_id.clone()) {
			ids_by_provider.entry(provider).or_default().push(project_id);
		}
	}

	let mut names: HashMap<DependencyKey, String> = HashMap::new();
	for (provider, ids) in ids_by_provider {
		for (project_id, meta) in read_cached_package_meta(ctx, provider, &ids).await {
			if !meta.name.is_empty() {
				names.insert((provider, project_id), meta.name);
			}
		}
	}

	let mut out: Vec<DependentPackage> = hashes
		.iter()
		.filter_map(|hash| by_hash.get(hash.as_str()).map(|info| (hash, *info)))
		.map(|(hash, info)| {
			let cached = info
				.provider
				.zip(info.project_id.clone())
				.and_then(|key| names.get(&key).cloned());

			DependentPackage {
				hash: hash.clone(),
				name: cached
					.or_else(|| info.display_name.clone())
					.unwrap_or_else(|| info.file_name.clone()),
			}
		})
		.collect();

	// Discovery order is breadth-first from the target, which means nothing to
	// someone reading a list.
	out.sort_by_key(|dep| dep.name.to_lowercase());
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn key(id: &str) -> DependencyKey {
		(ProviderId::Modrinth, id.to_string())
	}

	/// `(dependent, target, kind)` triples, as they come out of the database.
	fn graph(
		edges: &[(&str, &str, DependencyKind)],
	) -> (
		HashMap<String, Vec<DependencyKey>>,
		HashMap<DependencyKey, Vec<(String, DependencyKind)>>,
	) {
		let mut keys_by_hash: HashMap<String, Vec<DependencyKey>> = HashMap::new();
		let mut dependents: HashMap<DependencyKey, Vec<(String, DependencyKind)>> = HashMap::new();

		for (dependent, target, kind) in edges {
			keys_by_hash
				.entry((*dependent).to_string())
				.or_insert_with(|| vec![key(dependent)]);
			keys_by_hash
				.entry((*target).to_string())
				.or_insert_with(|| vec![key(target)]);
			dependents
				.entry(key(target))
				.or_default()
				.push(((*dependent).to_string(), *kind));
		}

		(keys_by_hash, dependents)
	}

	fn closure(root: &str, edges: &[(&str, &str, DependencyKind)]) -> (Vec<String>, Vec<String>) {
		let (keys_by_hash, dependents) = graph(edges);
		let result = walk(
			&[key(root)],
			&HashSet::from([root.to_string()]),
			&keys_by_hash,
			&dependents,
		);

		(result.required, result.optional)
	}

	#[test]
	fn a_required_dependency_takes_its_dependents_with_it() {
		let (required, optional) = closure("api", &[("sodium", "api", DependencyKind::Required)]);

		assert_eq!(required, ["sodium"]);
		assert!(optional.is_empty());
	}

	#[test]
	fn the_closure_is_transitive() {
		// c needs b needs a: disabling a has to reach c, not stop at b.
		let (required, _) = closure(
			"a",
			&[
				("b", "a", DependencyKind::Required),
				("c", "b", DependencyKind::Required),
			],
		);

		assert_eq!(required, ["b", "c"]);
	}

	#[test]
	fn a_cycle_terminates() {
		// Two mods that each require the other, plus a third hanging off them.
		let (required, _) = closure(
			"a",
			&[
				("b", "a", DependencyKind::Required),
				("a", "b", DependencyKind::Required),
				("c", "b", DependencyKind::Required),
			],
		);

		assert_eq!(required, ["b", "c"], "the target itself is never a dependent");
	}

	#[test]
	fn an_optional_dependent_is_reported_but_not_followed() {
		let (required, optional) = closure(
			"a",
			&[
				("b", "a", DependencyKind::Optional),
				("c", "b", DependencyKind::Required),
			],
		);

		assert!(required.is_empty(), "an optional dependency is not a reason to disable");
		assert_eq!(optional, ["b"]);
		// c only breaks if b goes, and b is staying.
	}

	#[test]
	fn a_package_needed_both_ways_is_only_disabled() {
		let (required, optional) = closure(
			"a",
			&[
				("b", "a", DependencyKind::Required),
				("c", "a", DependencyKind::Optional),
				("c", "b", DependencyKind::Required),
			],
		);

		assert_eq!(required, ["b", "c"]);
		assert!(
			optional.is_empty(),
			"a package that is going down must not also be listed as unaffected"
		);
	}

	#[test]
	fn incompatible_and_embedded_relations_are_ignored() {
		let (required, optional) = closure(
			"a",
			&[
				("b", "a", DependencyKind::Incompatible),
				("c", "a", DependencyKind::Embedded),
			],
		);

		assert!(required.is_empty());
		assert!(optional.is_empty());
	}

	#[test]
	fn a_package_reached_by_two_paths_is_listed_once() {
		let (required, _) = closure(
			"a",
			&[
				("b", "a", DependencyKind::Required),
				("c", "a", DependencyKind::Required),
				("d", "b", DependencyKind::Required),
				("d", "c", DependencyKind::Required),
			],
		);

		assert_eq!(required, ["b", "c", "d"]);
	}

	#[test]
	fn a_version_pinned_dependency_still_matches() {
		// Modrinth may name only the version; the package answers to both ids,
		// so the edge has to find it either way.
		let mut keys_by_hash = HashMap::new();
		keys_by_hash.insert(
			"lib".to_string(),
			vec![key("lib-project"), key("lib-version")],
		);
		keys_by_hash.insert("mod".to_string(), vec![key("mod-project")]);

		let mut dependents = HashMap::new();
		dependents.insert(
			key("lib-version"),
			vec![("mod".to_string(), DependencyKind::Required)],
		);

		let result = walk(
			&[key("lib-project"), key("lib-version")],
			&HashSet::from(["lib".to_string()]),
			&keys_by_hash,
			&dependents,
		);

		assert_eq!(result.required, ["mod"]);
	}
}
