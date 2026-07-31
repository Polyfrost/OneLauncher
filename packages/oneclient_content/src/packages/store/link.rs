use std::path::Path;

use oneclient_db::models::ClusterRow;

use oneclient_common::domain::ContentType;
use oneclient_common::paths;
use crate::error::ContentResult;

use super::manifest;

#[tracing::instrument(level = "debug")]
pub async fn link_or_copy(src: &Path, dest: &Path) -> ContentResult<()> {
	if let Some(parent) = dest.parent() {
		polyio::create_dir_all(parent).await?;
	}

	remove_entry(dest).await?;

	if polyio::symlink_file(src, dest).await.is_ok() {
		return Ok(());
	}

	polyio::copy(src, dest).await?;
	Ok(())
}

/// Deletes `path` if anything is there, symlink or not.
///
/// `Path::exists` resolves symlinks, so a link whose artifact was evicted from
/// the cache reads as absent and the dead link survives every unlink; the
/// package then keeps showing up in the folder it was removed from.
pub async fn remove_entry(path: &Path) -> ContentResult<()> {
	if polyio::symlink_metadata(path).await.is_err() {
		return Ok(());
	}

	polyio::remove_file(path).await?;
	Ok(())
}

/// Drops a package out of the game folder right now, if that is possible.
///
/// Purely an optimisation for what the user sees: removal is a database
/// operation, and the folder is brought in line at the next launch regardless.
/// A running game holds its jars open — on Windows that blocks deleting *any*
/// hard link to them — so failure here is expected and is not an error.
///
/// Returns whether anything was actually deleted.
#[tracing::instrument(level = "debug", skip(cluster), fields(cluster_id = cluster.id))]
pub async fn try_unlink_materialized(
	cluster: &ClusterRow,
	content_type: ContentType,
	file_name: &str,
) -> bool {
	let Ok(game_dir) = paths::cluster_game_dir(&cluster.folder_name) else {
		return false;
	};

	let Some(mut loaded) = manifest::load(&game_dir).await else {
		return false;
	};

	// The shared game dir belongs to whichever cluster played last. Touching a
	// file we did not put there would delete another cluster's content, or a
	// file the user placed by hand.
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

#[cfg(test)]
mod tests {
	use super::*;

	/// A dead link — one whose target was deleted underneath it — has to go.
	/// `Path::exists` follows the link and reports it absent, which is what let
	/// these survive every unlink.
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
	async fn remove_entry_is_fine_with_nothing_there() {
		let root = polyio::testing::ScratchDir::new("remove_missing");
		polyio::create_dir_all(root.path()).await.unwrap();

		remove_entry(&root.join("never_existed.jar")).await.unwrap();

		std::fs::remove_dir_all(root.path()).ok();
	}
}
