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

/// `Path::exists` resolves symlinks so a link to an evicted artifact reads as absent and survives every unlink
pub async fn remove_entry(path: &Path) -> ContentResult<()> {
	if polyio::symlink_metadata(path).await.is_err() {
		return Ok(());
	}

	polyio::remove_file(path).await?;
	Ok(())
}

fn materialized_root(
	cluster: &ClusterRow,
	content_type: ContentType,
) -> Option<(std::path::PathBuf, &'static str)> {
	if content_type.is_global() {
		return paths::shared_minecraft_dir()
			.ok()
			.map(|dir| (dir, manifest::GLOBAL_MANIFEST_NAME));
	}

	if content_type == ContentType::Mod {
		return paths::cluster_dir(&cluster.folder_name)
			.ok()
			.map(|dir| (dir, manifest::MODS_MANIFEST_NAME));
	}

	paths::cluster_game_dir(&cluster.folder_name)
		.ok()
		.map(|dir| (dir, manifest::MANIFEST_NAME))
}

#[tracing::instrument(level = "debug", skip(cluster), fields(cluster_id = cluster.id))]
pub async fn try_unlink_materialized(
	cluster: &ClusterRow,
	content_type: ContentType,
	file_name: &str,
) -> bool {
	let Some((root, manifest_name)) = materialized_root(cluster, content_type) else {
		return false;
	};

	let Some(mut loaded) = manifest::load(&root, manifest_name).await else {
		return false;
	};

	let relative = manifest::entry_path(content_type.folder_name(), file_name);
	let ours = if content_type.is_global() {
		loaded.contains(&relative)
	} else {
		loaded.owns(cluster.id, &relative)
	};

	if !ours {
		return false;
	}

	let path = root.join(&relative);
	if let Err(err) = remove_entry(&path).await {
		tracing::debug!(
			file = file_name,
			error = %err,
			"could not drop package from the game folder now; it goes at the next launch"
		);
		return false;
	}

	loaded.entries.retain(|entry| entry.path != relative);
	manifest::save(&root, manifest_name, &loaded).await;

	true
}

#[cfg(test)]
mod tests {
	use super::*;

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
