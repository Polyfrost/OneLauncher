use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MANIFEST_NAME: &str = ".oneclient-materialized.json";
pub const MODS_MANIFEST_NAME: &str = ".oneclient-mods.json";
pub const GLOBAL_MANIFEST_NAME: &str = ".oneclient-global.json";

const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
	/// Slash-separated relative to the game directory e.g. `mods/sodium.jar`
	pub path: String,
	pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterializedManifest {
	pub version: u32,
	/// The shared `.minecraft` is used by every non-dedicated cluster in turn so this often is not us
	pub cluster_id: i64,
	pub entries: Vec<ManifestEntry>,
}

impl MaterializedManifest {
	#[must_use]
	pub fn new(cluster_id: i64, entries: Vec<ManifestEntry>) -> Self {
		Self {
			version: MANIFEST_VERSION,
			cluster_id,
			entries,
		}
	}

	#[must_use]
	pub fn paths(&self) -> HashSet<&str> {
		self.entries.iter().map(|e| e.path.as_str()).collect()
	}

	#[must_use]
	pub fn contains(&self, relative_path: &str) -> bool {
		self.entries.iter().any(|e| e.path == relative_path)
	}

	/// The cluster check is what makes deleting out of the shared game directory safe
	#[must_use]
	pub fn owns(&self, cluster_id: i64, relative_path: &str) -> bool {
		self.cluster_id == cluster_id && self.contains(relative_path)
	}
}

#[must_use]
pub fn manifest_path(dir: &Path, name: &str) -> PathBuf {
	dir.join(name)
}

/// An unparseable manifest reads as `None` not an error
/// stashing links as user content is recoverable refusing to launch is not
pub async fn load(dir: &Path, name: &str) -> Option<MaterializedManifest> {
	let raw = polyio::read_to_string(manifest_path(dir, name)).await.ok()?;

	match serde_json::from_str::<MaterializedManifest>(&raw) {
		Ok(manifest) if manifest.version == MANIFEST_VERSION => Some(manifest),
		Ok(manifest) => {
			tracing::warn!(
				version = manifest.version,
				"ignoring materialized manifest written by a different launcher version"
			);
			None
		}
		Err(err) => {
			tracing::warn!(error = %err, "materialized manifest is unreadable; ignoring");
			None
		}
	}
}

pub async fn save(dir: &Path, name: &str, manifest: &MaterializedManifest) {
	let body = match serde_json::to_vec_pretty(manifest) {
		Ok(body) => body,
		Err(err) => {
			tracing::warn!(error = %err, "failed to encode materialized manifest");
			return;
		}
	};

	if let Err(err) = polyio::write(manifest_path(dir, name), body).await {
		tracing::warn!(error = %err, "failed to write materialized manifest");
	}
}

pub async fn clear(dir: &Path, name: &str) {
	polyio::remove_file(manifest_path(dir, name)).await.ok();
}

#[must_use]
pub fn entry_path(content_folder: &str, file_name: &str) -> String {
	format!("{content_folder}/{file_name}")
}

// whether this cluster keeps its mods in its own folder rather than in the game directory
pub async fn mods_live_in_cluster(cluster_dir: &Path) -> bool {
	load(cluster_dir, MODS_MANIFEST_NAME).await.is_some()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn manifest() -> MaterializedManifest {
		MaterializedManifest::new(
			7,
			vec![ManifestEntry {
				path: "mods/sodium.jar".into(),
				hash: "abc".into(),
			}],
		)
	}

	#[test]
	fn ownership_is_scoped_to_the_cluster_that_wrote_it() {
		let manifest = manifest();

		assert!(manifest.owns(7, "mods/sodium.jar"));
		assert!(!manifest.owns(8, "mods/sodium.jar"));
		assert!(!manifest.owns(7, "mods/iris.jar"));
	}

	#[tokio::test]
	async fn round_trips_through_the_game_dir() {
		let root = polyio::testing::ScratchDir::new("manifest_round_trip");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();

		assert!(load(dir, MANIFEST_NAME).await.is_none(), "no manifest yet");

		save(dir, MANIFEST_NAME, &manifest()).await;
		let loaded = load(dir, MANIFEST_NAME).await.expect("manifest should load");
		assert_eq!(loaded.cluster_id, 7);
		assert!(loaded.contains("mods/sodium.jar"));

		clear(dir, MANIFEST_NAME).await;
		assert!(
			load(dir, MANIFEST_NAME).await.is_none(),
			"cleared manifest should be gone"
		);

		std::fs::remove_dir_all(root.path()).ok();
	}

	#[tokio::test]
	async fn the_two_manifests_do_not_collide() {
		let root = polyio::testing::ScratchDir::new("manifest_two_names");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();

		save(dir, MODS_MANIFEST_NAME, &manifest()).await;

		assert!(
			load(dir, MODS_MANIFEST_NAME).await.is_some(),
			"the mods manifest is there"
		);
		assert!(
			load(dir, MANIFEST_NAME).await.is_none(),
			"and it is not mistaken for the game-dir one"
		);

		clear(dir, MODS_MANIFEST_NAME).await;

		std::fs::remove_dir_all(root.path()).ok();
	}

	#[tokio::test]
	async fn garbage_manifest_reads_as_absent() {
		let root = polyio::testing::ScratchDir::new("manifest_garbage");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();
		polyio::write(manifest_path(dir, MANIFEST_NAME), b"not json".as_slice())
			.await
			.unwrap();

		assert!(load(dir, MANIFEST_NAME).await.is_none());

		std::fs::remove_dir_all(root.path()).ok();
	}
}
