//! Handles fetching metadata via interpulse.

use interpulse::api::minecraft::VersionManifest as MinecraftManifest;
use interpulse::api::modded::Manifest as ModdedManifest;
use serde::{Deserialize, Serialize};

use crate::utils::http::{fetch, read_json, write, FetchSemaphore, IoSemaphore};
use crate::utils::io::copy;
use crate::State;

use super::Directories;

/// The metadata url.
const METADATA_URL: &str = "https://meta.polyfrost.org";

/// A structure of manifests and metadata fetching utilities.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metadata {
	/// The [`MinecraftManifest`] associated with core Minecraft versions.
	pub minecraft: MinecraftManifest,
	/// The [`ModdedManifest`] associated with core Fabric versions.
	pub fabric: ModdedManifest,
	/// The [`ModdedManifest`] associated with core Quilt versions.
	pub quilt: ModdedManifest,
	/// The [`ModdedManifest`] associated with core NeoForge versions.
	pub neoforge: ModdedManifest,
	/// The [`ModdedManifest`] associated with core Forge versions.
	pub forge: ModdedManifest,
	// /// The [`ModdedManifest`] associated with core Legacy Fabric versions.
	// pub legacy_fabric: Option<ModdedManifest>,
}

impl Metadata {
	/// Get the formatted manifest for a specific [`Metadata`] type.
	fn get_manifest(name: &str, version: usize) -> String {
		format!("{METADATA_URL}/{name}/{version}/manifest.json")
	}

	/// Fetch all available metadata types and get a new [`Metadata`] structure.
	pub async fn fetch() -> crate::Result<Self> {
		let semaphore = &State::get().await?.fetch_semaphore;
		let (minecraft, fabric, quilt, neoforge, forge) = tokio::try_join! {
			async {
				let url = Self::get_manifest("minecraft", interpulse::api::minecraft::CURRENT_FORMAT_VERSION);
				fetch_version_manifest(Some(&url), semaphore).await
			},
			async {
				let url = Self::get_manifest("fabric", interpulse::api::modded::CURRENT_FABRIC_FORMAT_VERSION);
				fetch_modded_manifest(&url, semaphore).await
			},
			async {
				let url = Self::get_manifest("quilt", interpulse::api::modded::CURRENT_QUILT_FORMAT_VERSION);
				fetch_modded_manifest(&url, semaphore).await
			},
			async {
				let url = Self::get_manifest("neo", interpulse::api::modded::CURRENT_NEOFORGE_FORMAT_VERSION);
				fetch_modded_manifest(&url, semaphore).await
			},
			async {
				let url = Self::get_manifest("forge", interpulse::api::modded::CURRENT_FORGE_FORMAT_VERSION);
				fetch_modded_manifest(&url, semaphore).await
			},
		}?;

		Ok(Self {
			minecraft,
			fabric,
			quilt,
			neoforge,
			forge,
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

async fn fetch_version_manifest(
	url: Option<&str>,
	semaphore: &FetchSemaphore,
) -> crate::Result<MinecraftManifest> {
	Ok(serde_json::from_slice(
		&fetch(
			url.unwrap_or(interpulse::api::minecraft::VERSION_MANIFEST_URL),
			None,
			semaphore,
		)
		.await?,
	)?)
}

async fn fetch_modded_manifest(
	url: &str,
	semaphore: &FetchSemaphore,
) -> crate::Result<ModdedManifest> {
	Ok(serde_json::from_slice(&fetch(url, None, semaphore).await?)?)
}
