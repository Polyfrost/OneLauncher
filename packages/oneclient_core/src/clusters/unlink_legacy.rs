use std::path::Path;

use oneclient_db::dao::artifact as artifact_dao;

use crate::LauncherResult;
use crate::state::LauncherState;
use oneclient_common::domain::ContentType;
use oneclient_content::packages::store::manifest::{
	self, ManifestEntry, MaterializedManifest,
};
use oneclient_content::packages::store::artifact_absolute_path;

const SWEPT_TYPES: [ContentType; 2] = [ContentType::Mod, ContentType::DataPack];
const SWEPT_TYPES_REDIRECTED: [ContentType; 1] = [ContentType::DataPack];

async fn swept_types(cluster_root: &Path) -> &'static [ContentType] {
	if manifest::mods_live_in_cluster(cluster_root).await {
		&SWEPT_TYPES_REDIRECTED
	} else {
		&SWEPT_TYPES
	}
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SweepReport {
	pub removed: usize,
	pub adopted: usize,
	pub skipped: usize,
}

#[tracing::instrument(skip(state))]
pub async fn unlink_legacy_cluster_content(state: &LauncherState) -> LauncherResult<SweepReport> {
	let mut report = SweepReport::default();

	for cluster in state.clusters.list().await? {
		let dedicated = cluster.uses_dedicated_dir();
		let Ok(cluster_root) = cluster.dir() else {
			continue;
		};

		let swept = swept_types(&cluster_root).await;

		if dedicated {
			adopt_dedicated(state, &cluster, &cluster_root, swept, &mut report).await;
		} else {
			sweep_shared(state, &cluster_root, swept, &mut report).await;
		}
	}

	if report.removed > 0 || report.adopted > 0 {
		tracing::info!(
			removed = report.removed,
			adopted = report.adopted,
			skipped = report.skipped,
			"cleaned up legacy cluster content links"
		);
	}

	Ok(report)
}

async fn sweep_shared(
	state: &LauncherState,
	cluster_root: &Path,
	swept: &[ContentType],
	report: &mut SweepReport,
) {
	for content_type in swept {
		let dir = cluster_root.join(content_type.folder_name());
		let Ok(mut entries) = polyio::read_dir(&dir).await else {
			continue;
		};

		while let Ok(Some(entry)) = entries.next_entry().await {
			let path = entry.path();
			let Ok(file_type) = entry.file_type().await else {
				continue;
			};

			// A symlink here is unambiguously ours nothing else writes links here
			if file_type.is_symlink() {
				polyio::remove_file(&path).await.ok();
				report.removed += 1;
				continue;
			}

			if !file_type.is_file() {
				continue;
			}

			match cached_artifact_for(state, &path).await {
				Some(CacheMatch::Elsewhere) => {
					if polyio::remove_file(&path).await.is_ok() {
						report.removed += 1;
					}
				}
				// Cache row points at this very file (older recovery pass stored it
				// in place) deleting it would destroy the only copy
				Some(CacheMatch::IsTheArtifact) => {
					tracing::debug!(
						path = %path.display(),
						"leaving cluster file in place; it is the artifact's only copy"
					);
					report.skipped += 1;
				}
				None => report.skipped += 1,
			}
		}
	}
}

/// Writes a manifest so later launches can tell adopted content from user files
async fn adopt_dedicated(
	state: &LauncherState,
	cluster: &crate::clusters::Cluster,
	cluster_root: &Path,
	swept: &[ContentType],
	report: &mut SweepReport,
) {
	if manifest::load(cluster_root, manifest::MANIFEST_NAME)
		.await
		.is_some()
	{
		return;
	}

	let linked = match oneclient_content::packages::PackageStore::list_linked_artifacts(
		cluster.id,
		&state.services.content(),
	)
	.await
	{
		Ok(linked) => linked,
		Err(err) => {
			tracing::warn!(cluster_id = cluster.id, error = %err, "cannot adopt dedicated content");
			return;
		}
	};

	let mut entries = Vec::new();
	for link in linked {
		if !link.enabled || !swept.contains(&link.content_type) {
			continue;
		}

		let relative =
			manifest::entry_path(link.content_type.folder_name(), &link.cluster_file_name);
		if polyio::symlink_metadata(cluster_root.join(&relative))
			.await
			.is_err()
		{
			continue;
		}

		entries.push(ManifestEntry {
			path: relative,
			hash: link.hash,
		});
	}

	report.adopted += entries.len();
	manifest::save(
		cluster_root,
		manifest::MANIFEST_NAME,
		&MaterializedManifest::new(cluster.id, entries),
	)
	.await;
}

enum CacheMatch {
	/// Cache holds this content at its own path the cluster copy is a link
	Elsewhere,
	/// The artifact row points at this exact file
	IsTheArtifact,
}

async fn cached_artifact_for(state: &LauncherState, path: &Path) -> Option<CacheMatch> {
	let hash = polyio::sha1_file(path).await.ok()?;
	let row = artifact_dao::get_artifact_by_hash(&state.services.db, &polyio::normalize_hash(&hash))
		.await
		.ok()
		.flatten()?;

	let canonical = artifact_absolute_path(&row.path).ok()?;
	if same_file(&canonical, path) {
		Some(CacheMatch::IsTheArtifact)
	} else {
		Some(CacheMatch::Elsewhere)
	}
}

fn same_file(a: &Path, b: &Path) -> bool {
	match (polyio::canonicalize(a), polyio::canonicalize(b)) {
		(Ok(a), Ok(b)) => a == b,
		_ => a == b,
	}
}
