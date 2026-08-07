//! Bundle-tracked artifacts are skipped
//! [`crate::bundles::updates`] owns those files and the two flows would fight
//! over the same artifact
//!
//! The `browser_package_updates` cache is only narrowed by a check that reached
//! the provider so an offline run is a no-op rather than "all up to date"

use std::collections::HashSet;

use futures_util::StreamExt;

use oneclient_common::domain::ProviderId;
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::browser_package_update as update_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::models::{BrowserPackageUpdateRow, ClusterRow};
use oneclient_events::GroupedProgressChild;

use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};
use crate::packages::dependencies::pick_version;
use crate::packages::store::{PackageStore, evict_if_unused, try_unlink_materialized};
use crate::packages::types::LinkedArtifactInfo;

/// Matches the bundle installer's fan-out
/// the providers are the bottleneck
const UPDATE_CHECK_CONCURRENCY: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserPackageUpdate {
	pub cluster_id: i64,
	/// The artifact currently linked into the cluster
	pub hash: String,
	pub provider: ProviderId,
	pub project_id: String,
	pub installed_version_id: String,
	pub installed_version_name: String,
	pub latest_version_id: String,
	pub latest_version_name: String,
	/// Local fallback for a UI with no meta cache entry
	pub display_name: String,
	/// The user has already declined this exact newer version
	pub skipped: bool,
}

impl BrowserPackageUpdate {
	fn from_row(row: BrowserPackageUpdateRow) -> Self {
		Self {
			cluster_id: row.cluster_id,
			hash: row.hash,
			provider: ProviderId::from_repr(row.provider as u8).unwrap_or(ProviderId::Modrinth),
			project_id: row.project_id,
			installed_version_id: row.installed_version_id,
			installed_version_name: row.installed_version_name,
			latest_version_id: row.latest_version_id,
			latest_version_name: row.latest_version_name,
			display_name: row.display_name,
			skipped: row.skipped != 0,
		}
	}
}

#[derive(Debug, Clone, Default)]
pub struct BrowserUpdateCheck {
	pub cluster_id: i64,
	pub updates: Vec<BrowserPackageUpdate>,
	/// Unreachable this run
	/// their cached rows are preserved not cleared
	pub unchecked: Vec<String>,
}

impl BrowserUpdateCheck {
	#[must_use]
	pub fn pending(&self) -> Vec<BrowserPackageUpdate> {
		self.updates.iter().filter(|u| !u.skipped).cloned().collect()
	}
}

struct Candidate {
	hash: String,
	provider: ProviderId,
	project_id: String,
	version_id: String,
	display_name: String,
	display_version: String,
}

/// A missing bundle tracking row is the marker for "the user added this themselves"
fn browser_installed(
	linked: &[LinkedArtifactInfo],
	bundle_hashes: &HashSet<String>,
) -> Vec<Candidate> {
	linked
		.iter()
		.filter(|info| !bundle_hashes.contains(&info.hash))
		.filter_map(|info| {
			let provider = info.provider?;
			if provider == ProviderId::Local {
				return None;
			}
			Some(Candidate {
				hash: info.hash.clone(),
				provider,
				project_id: info.project_id.clone()?,
				version_id: info.version_id.clone()?,
				display_name: info
					.display_name
					.clone()
					.unwrap_or_else(|| info.file_name.clone()),
				display_version: info.display_version.clone().unwrap_or_default(),
			})
		})
		.collect()
}

/// An unreachable project lands in [`BrowserUpdateCheck::unchecked`]
/// only an unreadable cluster errors
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn check_browser_package_updates(
	cluster_id: i64,
	ctx: &ContentCtx,
) -> ContentResult<BrowserUpdateCheck> {
	let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
	let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;

	let bundle_hashes: HashSet<String> = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id)
		.await?
		.into_iter()
		.map(|row| row.hash)
		.collect();

	let candidates = browser_installed(&linked, &bundle_hashes);
	if candidates.is_empty() {
		return Ok(BrowserUpdateCheck {
			cluster_id,
			..Default::default()
		});
	}

	let skipped: HashSet<String> = update_dao::list_for_cluster(&ctx.db, cluster_id)
		.await?
		.into_iter()
		.filter(|row| row.skipped != 0)
		.map(|row| format!("{}:{}", row.hash, row.latest_version_id))
		.collect();

	let results = futures_util::stream::iter(candidates.into_iter().map(|candidate| {
		let cluster = &cluster;
		async move {
			let outcome = latest_for(&candidate, cluster, ctx).await;
			(candidate, outcome)
		}
	}))
	.buffer_unordered(UPDATE_CHECK_CONCURRENCY)
	.collect::<Vec<_>>()
	.await;

	let mut check = BrowserUpdateCheck {
		cluster_id,
		..Default::default()
	};

	for (candidate, outcome) in results {
		match outcome {
			Ok(Some((latest_version_id, latest_version_name))) => {
				let key = format!("{}:{latest_version_id}", candidate.hash);
				check.updates.push(BrowserPackageUpdate {
					cluster_id,
					hash: candidate.hash,
					provider: candidate.provider,
					project_id: candidate.project_id,
					installed_version_id: candidate.version_id,
					installed_version_name: candidate.display_version,
					latest_version_id,
					latest_version_name,
					display_name: candidate.display_name,
					skipped: skipped.contains(&key),
				});
			}
			Ok(None) => {}
			Err(err) => {
				// Offline is the common case and says nothing about the launcher
				if err.is_transient() {
					tracing::debug!(
						project_id = %candidate.project_id,
						error = %err,
						"update check skipped, provider unreachable"
					);
				} else {
					tracing::warn!(
						project_id = %candidate.project_id,
						error = %err,
						"update check failed for package"
					);
				}
				check.unchecked.push(candidate.hash);
			}
		}
	}

	Ok(check)
}

/// `None` when the installed version is already the newest compatible one
async fn latest_for(
	candidate: &Candidate,
	cluster: &ClusterRow,
	ctx: &ContentCtx,
) -> ContentResult<Option<(String, String)>> {
	let provider = ctx.providers.get(candidate.provider)?;
	let Some(latest) = pick_version(provider, &candidate.project_id, cluster, ctx).await? else {
		return Ok(None);
	};

	if latest.version_id == candidate.version_id {
		return Ok(None);
	}

	let name = if latest.version_number.is_empty() {
		latest.name
	} else {
		latest.version_number
	};

	Ok(Some((latest.version_id, name)))
}

/// Unreported rows are dropped except unreachable ones which keep their last
/// known state
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn refresh_browser_package_updates(
	cluster_id: i64,
	ctx: &ContentCtx,
) -> ContentResult<BrowserUpdateCheck> {
	let check = check_browser_package_updates(cluster_id, ctx).await?;

	let mut keep: Vec<String> = check.unchecked.clone();
	let mut stored = Vec::with_capacity(check.updates.len());

	for update in &check.updates {
		let row = update_dao::upsert(
			&ctx.db,
			cluster_id,
			&update.hash,
			update.provider as i64,
			&update.project_id,
			&update.installed_version_id,
			&update.installed_version_name,
			&update.latest_version_id,
			&update.latest_version_name,
			&update.display_name,
		)
		.await?;
		keep.push(row.hash.clone());
		// The row is the authority on `skipped` for this exact version
		stored.push(BrowserPackageUpdate::from_row(row));
	}

	update_dao::retain_hashes(&ctx.db, cluster_id, &keep).await?;

	Ok(BrowserUpdateCheck {
		cluster_id,
		updates: stored,
		unchecked: check.unchecked,
	})
}

/// Never touches the network
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn cached_browser_package_updates(
	cluster_id: i64,
	ctx: &ContentCtx,
) -> ContentResult<Vec<BrowserPackageUpdate>> {
	Ok(update_dao::list_for_cluster(&ctx.db, cluster_id)
		.await?
		.into_iter()
		.map(BrowserPackageUpdate::from_row)
		.collect())
}

/// The row stays and the package still reads as out of date
/// only the modal stops asking until a newer version than the declined one
/// appears
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn skip_browser_package_update(
	cluster_id: i64,
	hash: &str,
	ctx: &ContentCtx,
) -> ContentResult<()> {
	update_dao::set_skipped(&ctx.db, cluster_id, hash, true).await?;
	Ok(())
}

/// Download first unlink second so a failed download leaves the cluster as it was
#[tracing::instrument(level = "debug", skip(update, child, ctx), fields(cluster_id = update.cluster_id, project_id = %update.project_id))]
pub async fn apply_browser_package_update(
	update: &BrowserPackageUpdate,
	child: Option<&GroupedProgressChild>,
	ctx: &ContentCtx,
) -> ContentResult<String> {
	// Never touch anything a bundle owns however this was called
	if bundle_dao::get_bundle_tracked(&ctx.db, update.cluster_id, &update.hash)
		.await?
		.is_some()
	{
		return Err(ContentError::InvalidData {
			reason: format!(
				"{} is provided by a bundle and is not updated by the browser flow",
				update.display_name
			),
		});
	}

	let provider = ctx.providers.get(update.provider)?;
	let project = provider.get_project(&update.project_id, ctx).await?;
	let version = provider
		.get_version(&update.project_id, &update.latest_version_id, ctx)
		.await?;

	// Compatibility is not re-checked
	// a provider disagreeing at install time would strand the user on a build
	// they cannot move off
	let installed = PackageStore::install_to_cluster(
		update.provider,
		&project,
		&version,
		update.cluster_id,
		true,
		false,
		child,
		ctx,
	)
	.await?;

	let enabled_before = artifact_dao::get_cluster_artifact(&ctx.db, update.cluster_id, &update.hash)
		.await?
		.map(|link| link.enabled != 0);

	// Everything else of this project goes not just the recorded hash which can
	// be stale if the user installed a version by hand since the check ran
	unlink_other_versions(
		update.cluster_id,
		update.provider,
		&update.project_id,
		&installed.hash,
		ctx,
	)
	.await?;

	// Provider handed back the already-linked file
	// leave the link untouched
	if installed.hash == update.hash {
		update_dao::delete(&ctx.db, update.cluster_id, &update.hash).await?;
		return Ok(installed.hash);
	}

	// A package switched off stays off
	// the fresh link starts enabled so this is a toggle rather than a set
	if enabled_before == Some(false) {
		PackageStore::set_artifact_enabled(update.cluster_id, &installed.hash, ctx).await?;
	}

	update_dao::delete(&ctx.db, update.cluster_id, &update.hash).await?;

	Ok(installed.hash)
}

/// Bundle-owned copies are stepped over
/// unlinking one here would have the next bundle sync put it straight back
#[tracing::instrument(level = "debug", skip(ctx))]
async fn unlink_other_versions(
	cluster_id: i64,
	provider: ProviderId,
	project_id: &str,
	keep_hash: &str,
	ctx: &ContentCtx,
) -> ContentResult<()> {
	let others = artifact_dao::list_cluster_artifacts_for_project(
		&ctx.db,
		cluster_id,
		provider as i64,
		project_id,
		keep_hash,
	)
	.await?;

	for link in others {
		if bundle_dao::get_bundle_tracked(&ctx.db, cluster_id, &link.hash)
			.await?
			.is_some()
		{
			tracing::debug!(
				hash = %link.hash,
				"leaving a bundle-owned version of the project in place"
			);
			continue;
		}

		tracing::info!(
			cluster_id,
			project_id,
			hash = %link.hash,
			"dropping a superseded version of the project"
		);
		unlink_superseded(cluster_id, &link.hash, ctx).await?;
	}

	Ok(())
}

/// Not `bundles::remove_artifact_from_cluster`
/// `packages` must not depend on `bundles` and its override reconciliation
/// does not apply here
#[tracing::instrument(level = "debug", skip(ctx))]
async fn unlink_superseded(cluster_id: i64, hash: &str, ctx: &ContentCtx) -> ContentResult<()> {
	let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
	let content_type = artifact_dao::get_artifact_by_hash(&ctx.db, hash)
		.await?
		.and_then(|artifact| {
			oneclient_common::domain::ContentType::from_repr(artifact.content_type as u8)
		});
	let link = artifact_dao::get_cluster_artifact(&ctx.db, cluster_id, hash).await?;

	artifact_dao::unlink_cluster_artifact(&ctx.db, cluster_id, hash).await?;

	if let (Some(content_type), Some(link)) = (content_type, link) {
		try_unlink_materialized(&cluster, content_type, &link.cluster_file_name).await;
	}

	if let Err(err) = evict_if_unused(hash, ctx).await {
		tracing::warn!(hash, error = %err, "failed to evict the superseded artifact");
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use oneclient_common::domain::ContentType;

	fn linked(
		hash: &str,
		provider: Option<ProviderId>,
		project_id: Option<&str>,
		version_id: Option<&str>,
	) -> LinkedArtifactInfo {
		LinkedArtifactInfo {
			hash: hash.into(),
			cluster_file_name: format!("{hash}.jar"),
			enabled: true,
			content_type: ContentType::Mod,
			file_name: format!("{hash}.jar"),
			project_id: project_id.map(Into::into),
			version_id: version_id.map(Into::into),
			display_name: None,
			display_version: None,
			provider,
			published_at: None,
		}
	}

	#[test]
	fn bundle_provided_packages_are_never_candidates() {
		let linked = vec![linked(
			"a",
			Some(ProviderId::Modrinth),
			Some("sodium"),
			Some("v1"),
		)];
		let bundles: HashSet<String> = ["a".to_string()].into_iter().collect();

		assert!(
			browser_installed(&linked, &bundles).is_empty(),
			"a bundle-tracked artifact must stay out of the browser update flow"
		);
	}

	#[test]
	fn browser_installed_remote_packages_are_candidates() {
		let linked = vec![linked(
			"a",
			Some(ProviderId::Modrinth),
			Some("sodium"),
			Some("v1"),
		)];

		let found = browser_installed(&linked, &HashSet::new());
		assert_eq!(found.len(), 1);
		assert_eq!(found[0].project_id, "sodium");
		assert_eq!(found[0].version_id, "v1");
	}

	#[test]
	fn local_imports_are_not_candidates() {
		let linked = vec![
			linked("a", Some(ProviderId::Local), Some("x"), Some("v1")),
			linked("b", None, None, None),
		];

		assert!(
			browser_installed(&linked, &HashSet::new()).is_empty(),
			"a file with no provider release has no version to compare"
		);
	}

	#[test]
	fn a_remote_package_with_no_recorded_version_is_not_a_candidate() {
		let linked = vec![linked("a", Some(ProviderId::Modrinth), Some("sodium"), None)];

		assert!(
			browser_installed(&linked, &HashSet::new()).is_empty(),
			"without an installed version id there is nothing to compare against"
		);
	}

	#[test]
	fn pending_hides_versions_the_user_declined() {
		let make = |hash: &str, skipped: bool| BrowserPackageUpdate {
			cluster_id: 1,
			hash: hash.into(),
			provider: ProviderId::Modrinth,
			project_id: "sodium".into(),
			installed_version_id: "v1".into(),
			installed_version_name: "1.0".into(),
			latest_version_id: "v2".into(),
			latest_version_name: "2.0".into(),
			display_name: "Sodium".into(),
			skipped,
		};

		let check = BrowserUpdateCheck {
			cluster_id: 1,
			updates: vec![make("a", true), make("b", false)],
			unchecked: Vec::new(),
		};

		let pending = check.pending();
		assert_eq!(pending.len(), 1);
		assert_eq!(pending[0].hash, "b");
	}
}
