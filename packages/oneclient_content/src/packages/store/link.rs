use std::ffi::OsString;
use std::path::{Path, PathBuf};

use oneclient_db::models::{ArtifactRow, ClusterRow};

use oneclient_common::domain::ContentType;
use oneclient_common::paths;
use crate::error::ContentResult;

use super::manifest;
use super::paths::artifact_absolute_path;

const STAGING_SUFFIX: &str = ".oneclient-tmp";

/// What a live add actually did, so a caller can say so instead of promising
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveSync {
	/// The file is in the running game's folder now
	Applied,
	/// It could not be put there; it arrives at the next launch instead
	Deferred,
	/// Nothing to do: this content type never reloads, or no session owns the folder
	Skipped,
}

/// so a rerun that died mid-way leaves atleast one stale file, that next rerun clears anyways
fn staging_path(dest: &Path) -> PathBuf {
	let mut name = OsString::from(".");
	name.push(dest.file_name().unwrap_or_else(|| "entry".as_ref()));
	name.push(STAGING_SUFFIX);

	dest.with_file_name(name)
}

/// A launcher killed between the copy and the rename leaves one of these behind
/// and every other pass over these folders skips dotfiles
#[tracing::instrument(level = "debug")]
pub async fn sweep_staging_files(dir: &Path) {
	let Ok(mut entries) = polyio::read_dir(dir).await else {
		return;
	};

	while let Ok(Some(entry)) = entries.next_entry().await {
		let path = entry.path();
		let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
			continue;
		};

		if !name.starts_with('.') || !name.ends_with(STAGING_SUFFIX) {
			continue;
		}

		if let Err(err) = remove_entry(&path).await {
			tracing::debug!(file = name, error = %err, "failed to clear a stale staging file");
		}
	}
}

/// written to a staging name and renamed into place so the destination is never missing even for a moment
#[tracing::instrument(level = "debug")]
pub async fn link_or_copy(src: &Path, dest: &Path) -> ContentResult<()> {
	if let Some(parent) = dest.parent() {
		polyio::create_dir_all(parent).await?;
	}

	let staging = staging_path(dest);
	remove_entry(&staging).await?;

	if polyio::symlink_file(src, &staging).await.is_err() {
		polyio::copy(src, &staging).await?;
	}

	if let Err(err) = polyio::rename(&staging, dest).await {
		remove_entry(&staging).await.ok();
		return Err(err.into());
	}

	Ok(())
}

pub async fn remove_entry(path: &Path) -> ContentResult<()> {
	if polyio::symlink_metadata(path).await.is_err() {
		return Ok(());
	}

	polyio::remove_file(path).await?;
	Ok(())
}

#[tracing::instrument(level = "debug", skip(cluster), fields(cluster_id = cluster.id))]
pub async fn try_unlink_materialized(
	cluster: &ClusterRow,
	content_type: ContentType,
	file_name: &str,
) -> bool {
	let Ok(game_dir) = paths::cluster_game_dir(&cluster.folder_name) else {
		return false;
	};

	let _guard = manifest::lock().await;

	let Some(mut loaded) = manifest::load(&game_dir).await else {
		return false;
	};

	// The shared game dir belongs to whichever cluster played last
	// touching a file we did not put there would delete another cluster's or
	// the user's content
	let relative = manifest::entry_path(content_type.folder_name(), file_name);
	if !loaded.owns(cluster.id, &relative) {
		return false;
	}

	let path = game_dir.join(content_type.folder_name()).join(file_name);
	if let Err(err) = remove_entry(&path).await {
		tracing::debug!(
			file = file_name,
			error = %err,
			"could not drop package from the game folder now; it goes at the next launch"
		);
		return false;
	}

	loaded.entries.retain(|entry| entry.path != relative);
	manifest::save(&game_dir, &loaded).await;

	true
}

#[tracing::instrument(level = "debug", skip(cluster, artifact), fields(cluster_id = cluster.id, hash = %artifact.hash))]
pub async fn try_link_materialized(
	cluster: &ClusterRow,
	artifact: &ArtifactRow,
	file_name: &str,
) -> LiveSync {
	let Some(content_type) = ContentType::from_repr(artifact.content_type as u8) else {
		return LiveSync::Skipped;
	};

	if !content_type.reloads_in_game() {
		return LiveSync::Skipped;
	}

	let Ok(game_dir) = paths::cluster_game_dir(&cluster.folder_name) else {
		return LiveSync::Deferred;
	};

	let Ok(src) = artifact_absolute_path(&artifact.path) else {
		return LiveSync::Deferred;
	};

	if !polyio::try_exists(&src).await.unwrap_or(false) {
		tracing::warn!(hash = %artifact.hash, "cached artifact missing; leaving it to the next launch");
		return LiveSync::Deferred;
	}

	let _guard = manifest::lock().await;

	// No manifest means nothing is playing out of this folder right now
	let Some(mut loaded) = manifest::load(&game_dir).await else {
		return LiveSync::Skipped;
	};

	if loaded.cluster_id != cluster.id {
		return LiveSync::Deferred;
	}

	let relative = manifest::entry_path(content_type.folder_name(), file_name);
	let dest = game_dir.join(content_type.folder_name()).join(file_name);

	// Same rule as the unlink path: never write over a file we did not place
	if !loaded.owns(cluster.id, &relative) && polyio::symlink_metadata(&dest).await.is_ok() {
		tracing::debug!(
			file = file_name,
			"a file we do not own already sits in the game folder; leaving it to the next launch"
		);
		return LiveSync::Deferred;
	}

	if let Err(err) = link_or_copy(&src, &dest).await {
		tracing::debug!(
			file = file_name,
			error = %err,
			"could not add the pack to the running game; it goes in at the next launch"
		);
		return LiveSync::Deferred;
	}

	loaded.entries.retain(|entry| entry.path != relative);
	loaded.entries.push(manifest::ManifestEntry {
		path: relative,
		hash: artifact.hash.clone(),
	});
	manifest::save(&game_dir, &loaded).await;

	LiveSync::Applied
}

#[cfg(test)]
mod tests {
	use super::*;

	fn cluster() -> ClusterRow {
		ClusterRow {
			id: 1,
			name: "Test".into(),
			folder_name: "test".into(),
			setting_profile_name: None,
			mc_version: "1.21.1".into(),
			mc_loader: 0,
			stage: 0,
			mc_loader_version: None,
			created_at: None,
			last_played: None,
			overall_played: None,
			linked_modpack_hash: None,
		}
	}

	fn artifact(content_type: ContentType) -> ArtifactRow {
		ArtifactRow {
			hash: "abc".into(),
			content_type: content_type as i64,
			path: "packages/whatever".into(),
			file_name: "thing".into(),
			size_bytes: None,
		}
	}

	#[tokio::test]
	async fn a_mod_is_never_added_to_a_running_game() {
		assert_eq!(
			try_link_materialized(&cluster(), &artifact(ContentType::Mod), "sodium.jar").await,
			LiveSync::Skipped
		);
		assert_eq!(
			try_link_materialized(&cluster(), &artifact(ContentType::World), "world.zip").await,
			LiveSync::Skipped
		);
	}

	#[tokio::test]
	async fn a_stale_staging_file_is_swept() {
		let root = polyio::testing::ScratchDir::new("sweep_staging");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();

		let orphan = staging_path(&dir.join("pack.zip"));
		polyio::write(&orphan, b"half a copy".as_slice()).await.unwrap();
		polyio::write(dir.join("keep.zip"), b"pack".as_slice())
			.await
			.unwrap();

		sweep_staging_files(dir).await;

		assert!(polyio::symlink_metadata(&orphan).await.is_err());
		assert!(polyio::symlink_metadata(dir.join("keep.zip")).await.is_ok());

		std::fs::remove_dir_all(root.path()).ok();
	}

	#[tokio::test]
	async fn remove_entry_clears_a_dangling_link() {
		let root = polyio::testing::ScratchDir::new("dangling_link");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();

		let target = dir.join("target.jar");
		let link = dir.join("link.jar");
		polyio::write(&target, b"jar".as_slice()).await.unwrap();
		polyio::symlink_file(&target, &link).await.unwrap();
		polyio::remove_file(&target).await.unwrap();

		assert!(!link.exists(), "the link resolves to nothing");
		assert!(
			polyio::symlink_metadata(&link).await.is_ok(),
			"but the link itself is still there"
		);

		remove_entry(&link).await.unwrap();
		assert!(polyio::symlink_metadata(&link).await.is_err());

		std::fs::remove_dir_all(root.path()).ok();
	}

	#[tokio::test]
	async fn replacing_a_pack_leaves_only_the_pack() {
		let root = polyio::testing::ScratchDir::new("atomic_replace");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();

		let old = dir.join("old.zip");
		let new = dir.join("new.zip");
		polyio::write(&old, b"old".as_slice()).await.unwrap();
		polyio::write(&new, b"new".as_slice()).await.unwrap();

		let packs = dir.join("resourcepacks");
		let dest = packs.join("pack.zip");

		link_or_copy(&old, &dest).await.unwrap();
		link_or_copy(&new, &dest).await.unwrap();

		assert_eq!(polyio::read_to_string(&dest).await.unwrap(), "new");

		let mut names = Vec::new();
		let mut entries = polyio::read_dir(&packs).await.unwrap();
		while let Ok(Some(entry)) = entries.next_entry().await {
			names.push(entry.file_name().to_string_lossy().into_owned());
		}

		assert_eq!(names, vec!["pack.zip".to_string()]);

		std::fs::remove_dir_all(root.path()).ok();
	}

	/// A crash between the write and the rename must not wedge the next attempt
	#[tokio::test]
	async fn a_stale_staging_file_is_cleared() {
		let root = polyio::testing::ScratchDir::new("stale_staging");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();

		let src = dir.join("src.zip");
		let dest = dir.join("pack.zip");
		polyio::write(&src, b"real".as_slice()).await.unwrap();
		polyio::write(staging_path(&dest), b"junk".as_slice())
			.await
			.unwrap();

		link_or_copy(&src, &dest).await.unwrap();

		assert_eq!(polyio::read_to_string(&dest).await.unwrap(), "real");
		assert!(
			polyio::symlink_metadata(staging_path(&dest)).await.is_err(),
			"the staging file is consumed by the rename"
		);

		std::fs::remove_dir_all(root.path()).ok();
	}

	#[tokio::test]
	async fn remove_entry_is_fine_with_nothing_there() {
		let root = polyio::testing::ScratchDir::new("remove_missing");
		polyio::create_dir_all(root.path()).await.unwrap();

		remove_entry(&root.join("never_existed.jar")).await.unwrap();

		std::fs::remove_dir_all(root.path()).ok();
	}
}
