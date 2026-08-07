use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use oneclient_db::dao::artifact as artifact_dao;

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use oneclient_common::paths;

use super::paths::{artifact_absolute_path, relative_cache_path};

/// A download lands in the cache before its row is inserted so an in-flight fetch looks exactly like an orphan
const RECENT_GRACE: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Default, Clone, Copy)]
pub struct GcReport {
	pub artifacts_removed: usize,
	pub files_removed: usize,
	pub bytes_freed: u64,
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn evict_if_unused(hash: &str, ctx: &ContentCtx) -> ContentResult<bool> {
	// Read the row first once it is deleted there is no way back to the file
	let row = artifact_dao::get_artifact_by_hash(&ctx.db, hash).await?;

	if !artifact_dao::delete_artifact_if_unused(&ctx.db, hash).await? {
		return Ok(false);
	}

	if let Some(row) = row
		&& let Ok(path) = artifact_absolute_path(&row.path)
	{
		remove_cached_file(&path).await;
	}

	Ok(true)
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn collect_unused_artifacts(ctx: &ContentCtx) -> ContentResult<GcReport> {
	let mut report = GcReport::default();

	for row in artifact_dao::list_unused_artifacts(&ctx.db).await? {
		let path = artifact_absolute_path(&row.path).ok();

		// An install inserts the row then links it
		// in between it looks unused
		if let Some(path) = &path
			&& is_recent(path).await
		{
			continue;
		}

		if !artifact_dao::delete_artifact_if_unused(&ctx.db, &row.hash).await? {
			continue;
		}
		report.artifacts_removed += 1;

		if let Some(path) = path {
			report.bytes_freed += file_len(&path).await;
			remove_cached_file(&path).await;
		}
	}

	if report.artifacts_removed > 0 {
		tracing::info!(
			artifacts = report.artifacts_removed,
			mb_freed = report.bytes_freed / 1_000_000,
			"reclaimed unused packages"
		);
	}

	Ok(report)
}

pub async fn find_unreferenced_files(ctx: &ContentCtx) -> ContentResult<Vec<(PathBuf, u64)>> {
	let cache_root = paths::packages_cache_dir()?;
	if !polyio::try_exists(&cache_root).await.unwrap_or(false) {
		return Ok(Vec::new());
	}

	let known: HashSet<String> = artifact_dao::list_artifact_paths(&ctx.db)
		.await?
		.into_iter()
		.collect();

	// An empty artifacts table means a lost index not a cache full of garbage
	// the other reading would offer to wipe every package the user has
	if known.is_empty() {
		tracing::debug!("no artifacts indexed; treating the package cache as fully referenced");
		return Ok(Vec::new());
	}

	let mut found = Vec::new();
	for file in walk_files(&cache_root).await {
		let Ok(relative) = relative_cache_path(&file) else {
			continue;
		};
		if known.contains(&relative) || is_recent(&file).await {
			continue;
		}

		let size = file_len(&file).await;
		found.push((file, size));
	}

	Ok(found)
}

/// Reasons from the filesystem not the database so it can misread an install whose database was reset
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn remove_unreferenced_files(ctx: &ContentCtx) -> ContentResult<GcReport> {
	let cache_root = paths::packages_cache_dir()?;
	let mut report = GcReport::default();

	for (file, size) in find_unreferenced_files(ctx).await? {
		if polyio::remove_file(&file).await.is_ok() {
			report.files_removed += 1;
			report.bytes_freed += size;
			prune_empty_parents(&file, &cache_root).await;
		}
	}

	if report.files_removed > 0 {
		tracing::info!(
			files = report.files_removed,
			mb_freed = report.bytes_freed / 1_000_000,
			"removed unreferenced cache files"
		);
	}

	Ok(report)
}

async fn remove_cached_file(path: &Path) {
	if polyio::remove_file(path).await.is_err() {
		return;
	}

	if let Ok(root) = paths::packages_cache_dir() {
		prune_empty_parents(path, &root).await;
	}
}

async fn prune_empty_parents(file: &Path, root: &Path) {
	let mut dir = file.parent().map(Path::to_path_buf);

	while let Some(current) = dir {
		if current == root || !current.starts_with(root) {
			return;
		}

		// `remove_dir` refuses a non-empty directory which is exactly the stop condition
		if polyio::remove_dir(&current).await.is_err() {
			return;
		}

		dir = current.parent().map(Path::to_path_buf);
	}
}

async fn is_recent(path: &Path) -> bool {
	let Ok(meta) = polyio::stat(path).await else {
		// Cannot tell how old it is so leave it be
		return true;
	};

	meta.modified()
		.ok()
		.and_then(|modified| SystemTime::now().duration_since(modified).ok())
		.is_none_or(|age| age < RECENT_GRACE)
}

async fn file_len(path: &Path) -> u64 {
	polyio::stat(path).await.map(|m| m.len()).unwrap_or(0)
}

async fn walk_files(root: &Path) -> Vec<PathBuf> {
	let mut files = Vec::new();
	let mut stack = vec![root.to_path_buf()];

	while let Some(dir) = stack.pop() {
		let Ok(mut entries) = polyio::read_dir(&dir).await else {
			continue;
		};

		while let Ok(Some(entry)) = entries.next_entry().await {
			let Ok(file_type) = entry.file_type().await else {
				continue;
			};

			if file_type.is_dir() {
				stack.push(entry.path());
			} else if file_type.is_file() {
				files.push(entry.path());
			}
		}
	}

	files
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn empty_parents_go_but_the_root_stays() {
		let root = polyio::testing::ScratchDir::new("gc_prune");
		let cache = root.join("packages");
		let nested = cache.join("mod").join("modrinth").join("proj").join("v1");
		polyio::create_dir_all(&nested).await.unwrap();

		let file = nested.join("a.jar");
		polyio::write(&file, b"jar".as_slice()).await.unwrap();
		polyio::remove_file(&file).await.unwrap();

		prune_empty_parents(&file, &cache).await;

		assert!(!cache.join("mod").exists(), "empty chain should be gone");
		assert!(cache.exists(), "the cache root itself always stays");

		std::fs::remove_dir_all(root.path()).ok();
	}

	#[tokio::test]
	async fn a_sibling_stops_the_prune() {
		let root = polyio::testing::ScratchDir::new("gc_prune_sibling");
		let cache = root.join("packages");
		let version = cache.join("mod").join("modrinth").join("proj").join("v1");
		polyio::create_dir_all(&version).await.unwrap();

		let gone = version.join("a.jar");
		polyio::write(&gone, b"jar".as_slice()).await.unwrap();
		polyio::write(version.join("b.jar"), b"jar".as_slice())
			.await
			.unwrap();
		polyio::remove_file(&gone).await.unwrap();

		prune_empty_parents(&gone, &cache).await;

		assert!(version.exists(), "a directory with another file survives");

		std::fs::remove_dir_all(root.path()).ok();
	}

	#[tokio::test]
	async fn fresh_files_are_spared() {
		let root = polyio::testing::ScratchDir::new("gc_recent");
		polyio::create_dir_all(root.path()).await.unwrap();

		let file = root.join("downloading.jar");
		polyio::write(&file, b"partial".as_slice()).await.unwrap();

		assert!(is_recent(&file).await, "a just-written file is not garbage");

		std::fs::remove_dir_all(root.path()).ok();
	}

	#[tokio::test]
	async fn walk_finds_files_at_every_depth() {
		let root = polyio::testing::ScratchDir::new("gc_walk");
		polyio::create_dir_all(root.join("a").join("b")).await.unwrap();
		polyio::write(root.join("top.jar"), b"1".as_slice()).await.unwrap();
		polyio::write(root.join("a").join("mid.jar"), b"2".as_slice()).await.unwrap();
		polyio::write(root.join("a").join("b").join("deep.jar"), b"3".as_slice())
			.await
			.unwrap();

		let found = walk_files(root.path()).await;

		assert_eq!(found.len(), 3, "found: {found:?}");

		std::fs::remove_dir_all(root.path()).ok();
	}
}
