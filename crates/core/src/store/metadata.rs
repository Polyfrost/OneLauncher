//! Handles fetching metadata via interpulse.

use interpulse::api::minecraft::{fetch_version_manifest, VersionManifest as MinecraftManifest};
use interpulse::api::modded::{
	fetch_manifest as fetch_modded_manifest, Manifest as ModdedManifest,
};
use serde::{Deserialize, Serialize};

use crate::utils::http::{read_json, write, IoSemaphore};
use crate::utils::io::copy;
use crate::State;

use super::Directories;

/// The metadata url used in production.
#[cfg(not(debug_assertions))]
const METADATA_URL: &str = "https://meta.polyfrost.org";

/// The metadata url used in development.
#[cfg(debug_assertions)]
const METADATA_URL: &str = "localhost:5543";

/// A structure of manifests and metadata fetching utilities.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metadata {
	/// The [`MinecraftManifest`] associated with core Minecraft versions.
	pub minecraft: Option<MinecraftManifest>,
	/// The [`ModdedManifest`] associated with core Fabric versions.
	pub fabric: Option<ModdedManifest>,
	/// The [`ModdedManifest`] associated with core Quilt versions.
	pub quilt: Option<ModdedManifest>,
	/// The [`ModdedManifest`] associated with core NeoForge versions.
	pub neoforge: Option<ModdedManifest>,
	/// The [`ModdedManifest`] associated with core Forge versions.
	pub forge: Option<ModdedManifest>,
	/// The [`ModdedManifest`] associated with core Legacy Fabric versions.
	pub legacy_fabric: Option<ModdedManifest>,
}

impl Metadata {
	/// Get the formatted manifest for a specific [`Metadata`] type.
	fn get_manifest(name: &str) -> String {
		format!("{METADATA_URL}/{name}/v0/manifest.json")
	}

	/// Fetch all available metadata types and get a new [`Metadata`] structure.
	pub async fn fetch() -> crate::Result<Self> {
		// let (minecraft, fabric, quilt, neoforge, forge, legacy_fabric) = tokio::try_join! {
		// 	async {
		// 		let url = Self::get_manifest("minecraft");
		// 		fetch_version_manifest(Some(&url)).await
		// 	},
		// 	async {
		// 		let url = Self::get_manifest("fabric");
		// 		fetch_modded_manifest(&url).await
		// 	},
		// 	async {
		// 		let url = Self::get_manifest("quilt");
		// 		fetch_modded_manifest(&url).await
		// 	},
		// 	async {
		// 		let url = Self::get_manifest("neo");
		// 		fetch_modded_manifest(&url).await
		// 	},
		// 	async {
		// 		let url = Self::get_manifest("forge");
		// 		fetch_modded_manifest(&url).await
		// 	},
		// 	async {
		// 		let url = Self::get_manifest("legacy-fabric");
		// 		fetch_modded_manifest(&url).await
		// 	},
		// }?;

		// Ok(Self {
		// 	minecraft,
		// 	fabric,
		// 	quilt,
		// 	neoforge,
		// 	forge,
		// 	legacy_fabric,
		// })
		Ok(Self {
			minecraft: None,
			fabric: None,
			quilt: None,
			neoforge: None,
			forge: None,
			legacy_fabric: None,
		})
	}

	/// Initialize the core Metadata manager.
	#[tracing::instrument(skip(io_semaphore))]
	#[onelauncher_debug::debugger]
	pub async fn initialize(
		dirs: &Directories,
		online: bool,
		io_semaphore: &IoSemaphore,
	) -> crate::Result<Self> {
		let mut metadata = None;
		let path = dirs.caches_dir().await.join("metadata.json");
		let backup = dirs.caches_dir().await.join("metadata.json.bak");

		if let Ok(metadata_json) = read_json::<Metadata>(&path, io_semaphore).await {
			metadata = Some(metadata_json);
		} else if online {
			let res = async {
				let fetch_data = Self::fetch().await?;

				write(
					&path,
					&serde_json::to_vec(&fetch_data).unwrap_or_default(),
					io_semaphore,
				)
				.await?;
				write(
					&backup,
					&serde_json::to_vec(&fetch_data).unwrap_or_default(),
					io_semaphore,
				)
				.await?;

				metadata = Some(fetch_data);
				Ok::<(), crate::Error>(())
			}
			.await;

			match res {
				Ok(()) => {}
				Err(err) => {
					tracing::warn!("failed to fetch metadata: {err}")
				}
			}
		} else if let Ok(metadata_json) = read_json::<Metadata>(&backup, io_semaphore).await {
			metadata = Some(metadata_json);
			copy(&backup, &path).await?;
		}

		if let Some(meta) = metadata {
			Ok(meta)
		} else {
			Err(anyhow::anyhow!("failed to fetch launcher metadata").into())
		}
	}

	/// Update and backup all available metadata.
	pub async fn update() {
		let res = async {
			let fetch_data = Metadata::fetch().await?;
			let state = State::get().await?;

			let path = state.directories.caches_dir().await.join("metadata.json");
			let backup = state
				.directories
				.caches_dir()
				.await
				.join("metadata.json.bak");

			if path.exists() {
				copy(&path, &backup).await?;
			}

			write(
				&path,
				&serde_json::to_vec(&fetch_data)?,
				&state.io_semaphore,
			)
			.await?;

			let mut old_metadata = state.metadata.write().await;
			*old_metadata = fetch_data;

			Ok::<(), crate::Error>(())
		}
		.await;

		match res {
			Ok(()) => {}
			Err(err) => {
				tracing::warn!("failed to update launcher metadata: {err}")
			}
		};
	}
}
